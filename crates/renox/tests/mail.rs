use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::Kernel;
use renox::auth::{Channel, Notification};
use renox::mail::{Mail, MailConfig};
use renox::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tower::ServiceExt;

fn config(dir: &std::path::Path) -> Config {
    Config {
        env: Environment::Testing,
        key: Some(renox::generate_key()),
        views_path: dir.to_path_buf(),
        name: "Toko Kopi".into(),
        ..Config::default()
    }
}

async fn kernel_with(config: Config) -> Kernel {
    let kernel = App::with_config(config)
        .module(Auth::new())
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    kernel
}

fn views() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("mail")).unwrap();
    std::fs::write(
        dir.path().join("mail/receipt.html"),
        r#"{% extends "renox/mail/layout.html" %}{% from "renox/mail/button.html" import button %}
{% block content %}<h1>Terima kasih, {{ name }}</h1><table><tr><td>Total</td><td>Rp {{ total }}</td></tr></table>{{ button("https://toko.id/o/1", "Lihat pesanan") }}{% endblock %}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("mail/plain.html"),
        "<p>Hanya HTML untuk {{ name }}</p>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("mail/plain.txt"),
        "Versi teks untuk {{ name }} dari {{ app.name }}",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn mail_views_render_html_and_text() {
    let dir = views();
    let kernel = kernel_with(config(dir.path())).await;
    let state = kernel.state();

    let mail = state
        .mail_view(
            "budi@example.com",
            "Struk",
            "mail/receipt",
            context! { name => "Budi", total => "25.000" },
        )
        .unwrap();
    let html = mail.html.as_deref().unwrap();
    assert!(
        html.contains("<h1>Terima kasih, Budi</h1>") && html.contains("Toko Kopi"),
        "layout and app name"
    );
    assert!(html.contains(r#"href="https://toko.id/o/1""#));
    assert!(mail.text.contains("Terima kasih, Budi"), "{}", mail.text);
    assert!(mail.text.contains("Total Rp 25.000"), "{}", mail.text);
    assert!(
        mail.text.contains("Lihat pesanan (https://toko.id/o/1)"),
        "{}",
        mail.text
    );
    assert!(!mail.text.contains('<'), "{}", mail.text);

    let plain = state
        .mail_view("a@b.id", "x", "mail/plain", context! { name => "Ani" })
        .unwrap();
    assert_eq!(
        plain.text, "Versi teks untuk Ani dari Toko Kopi",
        "a .txt template wins"
    );
}

#[tokio::test]
async fn queued_mail_is_sent_by_a_worker() {
    let dir = views();
    let kernel = kernel_with(config(dir.path())).await;
    kernel
        .state()
        .queue_mail(Mail::new("a@b.id", "Nanti", "isi"))
        .await
        .unwrap();
    assert!(kernel.mailer().sent().is_empty());
    kernel.run_jobs().await.unwrap();
    assert_eq!(kernel.mailer().sent()[0].subject, "Nanti");
}

async fn get(kernel: &Kernel, uri: &str) -> (StatusCode, String) {
    let res = kernel
        .router()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn sent_mail_can_be_previewed_in_debug_only() {
    let dir = views();
    let kernel = kernel_with(Config {
        debug: true,
        ..config(dir.path())
    })
    .await;
    let mail = Mail::new("budi@example.com", "Halo <Budi>", "teks").html("<p>\"html\" & more</p>");
    kernel.mailer().send(mail).await.unwrap();

    let (status, list) = get(&kernel, "/_renox/mail").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list.contains("Halo &lt;Budi&gt;") && list.contains(r#"href="/_renox/mail/1""#),
        "{list}"
    );
    let (_, page) = get(&kernel, "/_renox/mail/1").await;
    assert!(
        page.contains(
            r#"<iframe sandbox srcdoc="&lt;p&gt;&quot;html&quot; &amp; more&lt;/p&gt;">"#
        ),
        "{page}"
    );
    assert!(page.contains("<pre>teks</pre>"));
    assert_eq!(
        get(&kernel, "/_renox/mail/9").await.0,
        StatusCode::NOT_FOUND
    );

    let production = kernel_with(Config {
        debug: false,
        ..config(dir.path())
    })
    .await;
    assert_eq!(
        get(&production, "/_renox/mail").await.0,
        StatusCode::NOT_FOUND
    );
}

/// Accepts one message, speaking just enough SMTP, and returns what was sent.
async fn fake_smtp() -> (u16, Arc<Mutex<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(String::new()));
    let log = received.clone();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (read, mut write) = socket.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"220 fake ESMTP\r\n").await.unwrap();
        let mut in_data = false;
        while let Ok(Some(line)) = lines.next_line().await {
            log.lock().unwrap().push_str(&format!("{line}\n"));
            let reply: &[u8] = if in_data {
                if line == "." {
                    in_data = false;
                    b"250 queued\r\n"
                } else {
                    continue;
                }
            } else if line.starts_with("EHLO") {
                b"250-fake\r\n250 8BITMIME\r\n"
            } else if line.starts_with("DATA") {
                in_data = true;
                b"354 go ahead\r\n"
            } else if line.starts_with("QUIT") {
                write.write_all(b"221 bye\r\n").await.unwrap();
                break;
            } else {
                b"250 ok\r\n"
            };
            write.write_all(reply).await.unwrap();
        }
    });
    (port, received)
}

#[tokio::test]
async fn smtp_sends_multipart_mail() {
    let dir = views();
    let (port, received) = fake_smtp().await;
    let smtp = MailConfig {
        mailer: "smtp".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        encryption: "none".into(),
        from_address: "toko@example.com".into(),
        ..MailConfig::default()
    };
    let kernel = kernel_with(Config {
        mail: smtp,
        ..config(dir.path())
    })
    .await;
    let mail = kernel
        .state()
        .mail_view(
            "budi@example.com",
            "Struk pesanan",
            "mail/receipt",
            context! { name => "Budi", total => "1" },
        )
        .unwrap();
    kernel.mailer().send(mail).await.unwrap();

    let data = received.lock().unwrap().clone();
    assert!(data.contains("MAIL FROM:<toko@example.com>"), "{data}");
    assert!(data.contains("RCPT TO:<budi@example.com>"));
    assert!(
        data.contains("From: \"Toko Kopi\" <toko@example.com>")
            || data.contains("From: Toko Kopi <toko@example.com>"),
        "{data}"
    );
    assert!(data.contains("Subject: Struk pesanan"));
    assert!(data.contains("multipart/alternative"));
    assert!(data.contains("text/plain") && data.contains("text/html"));
}

#[tokio::test]
async fn bad_mail_settings_fail_at_boot() {
    let dir = views();
    for (mail, expected) in [
        (
            MailConfig {
                mailer: "sendgrid".into(),
                ..MailConfig::default()
            },
            "MAIL_MAILER",
        ),
        (
            MailConfig {
                mailer: "smtp".into(),
                encryption: "ssl3".into(),
                ..MailConfig::default()
            },
            "MAIL_ENCRYPTION",
        ),
        (
            MailConfig {
                mailer: "smtp".into(),
                from_address: "not an address".into(),
                ..MailConfig::default()
            },
            "MAIL_FROM_ADDRESS",
        ),
    ] {
        let err = App::with_config(Config {
            mail,
            ..config(dir.path())
        })
        .boot()
        .await
        .err()
        .unwrap();
        assert!(format!("{err:?}").contains(expected), "{err:?}");
    }
}

struct OrderShipped {
    order_id: i64,
}

impl Notification for OrderShipped {
    fn kind(&self) -> &'static str {
        "order-shipped"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database]
    }

    fn to_mail(&self, user: &User, _: &AppState) -> Result<Mail> {
        Ok(Mail::new(
            &user.email,
            format!("Pesanan #{} dikirim", self.order_id),
            "Sedang di jalan.",
        ))
    }

    fn to_database(&self, _: &User) -> serde_json::Value {
        serde_json::json!({ "order_id": self.order_id })
    }
}

struct DatabaseOnly;

impl Notification for DatabaseOnly {
    fn kind(&self) -> &'static str {
        "promo"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail]
    }
}

#[tokio::test]
async fn notifications_go_to_mail_and_the_database() {
    let dir = views();
    let kernel = kernel_with(config(dir.path())).await;
    let (state, db) = (kernel.state(), kernel.db());
    let budi = User::register(db, "Budi", "budi@example.com", "rahasia123")
        .await
        .unwrap();
    let ani = User::register(db, "Ani", "ani@example.com", "rahasia123")
        .await
        .unwrap();

    state
        .notify(&budi, &OrderShipped { order_id: 7 })
        .await
        .unwrap();
    state
        .notify(&budi, &OrderShipped { order_id: 8 })
        .await
        .unwrap();
    assert_eq!(kernel.mailer().sent()[1].subject, "Pesanan #8 dikirim");

    let all = budi.notifications(db, 10).await.unwrap();
    assert_eq!((all.len(), all[0].kind.as_str()), (2, "order-shipped"));
    assert_eq!(all[0].data["order_id"], 8, "newest first");
    assert_eq!(budi.unread_notification_count(db).await.unwrap(), 2);

    assert!(
        !ani.mark_notification_read(db, all[0].id).await.unwrap(),
        "not Ani's"
    );
    assert!(budi.mark_notification_read(db, all[0].id).await.unwrap());
    assert_eq!(
        budi.unread_notifications(db).await.unwrap()[0].data["order_id"],
        7
    );
    assert_eq!(budi.mark_all_notifications_read(db).await.unwrap(), 1);
    assert_eq!(budi.unread_notification_count(db).await.unwrap(), 0);

    let err = state.notify(&ani, &DatabaseOnly).await.unwrap_err();
    assert!(format!("{err:?}").contains("notification `promo` has no mail version"));
}

#[tokio::test]
async fn auth_mails_are_html_with_a_text_version() {
    let dir = views();
    let kernel = kernel_with(config(dir.path())).await;
    User::register(kernel.db(), "Budi", "budi@example.com", "rahasia123")
        .await
        .unwrap();
    let router = kernel.router();
    let res = router
        .clone()
        .oneshot(
            Request::get("/forgot-password")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = res.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let page =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let token = page
        .split(r#"name="_token" value=""#)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    router
        .oneshot(
            Request::post("/forgot-password")
                .header("cookie", cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("_token={token}&email=budi@example.com")))
                .unwrap(),
        )
        .await
        .unwrap();

    let mail = kernel.mailer().sent().pop().unwrap();
    assert!(
        mail.html
            .as_deref()
            .unwrap()
            .contains("Reset your password</a>")
    );
    assert!(
        mail.text.starts_with("You asked to reset your password."),
        "{}",
        mail.text
    );
    assert!(
        mail.text
            .lines()
            .any(|l| l.starts_with("http://127.0.0.1:3000/reset-password/"))
    );
}

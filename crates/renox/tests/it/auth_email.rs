use std::collections::HashMap;

use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use renox::Kernel;
use renox::prelude::*;
use tower::ServiceExt;

struct Area;

impl Module for Area {
    fn name(&self) -> &'static str {
        "area"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { "home" })
            .name("home")
            .get("/token", |session: Session| async move { session.token() })
            .merge(
                Routes::new()
                    .get("/members", |auth: AuthUser| async move {
                        format!("member {}", auth.name)
                    })
                    .require_verified(),
            )
            .merge(
                Routes::new()
                    .get("/api/me", |auth: AuthUser| async move {
                        Json(serde_json::json!({ "name": auth.name }))
                    })
                    .post("/api/notes", |auth: AuthUser| async move {
                        format!("saved by {}", auth.name)
                    })
                    .require_auth(),
            )
    }
}

async fn kernel(auth: Auth) -> Kernel {
    let config = Config {
        env: Environment::Testing,
        key: Some(renox::generate_key()),
        views_path: std::env::temp_dir().join("renox-no-views"),
        ..Config::default()
    };
    let kernel = App::with_config(config)
        .module(auth)
        .module(Area)
        .boot()
        .await
        .unwrap();
    kernel.migrate().await.unwrap();
    kernel
}

struct Client {
    router: axum::Router,
    cookie: Option<String>,
}

struct Reply {
    status: StatusCode,
    headers: HashMap<String, String>,
    body: String,
}

impl Reply {
    fn location(&self) -> Option<&str> {
        self.headers.get("location").map(String::as_str)
    }
}

impl Client {
    fn new(kernel: &Kernel) -> Self {
        Self {
            router: kernel.router(),
            cookie: None,
        }
    }

    async fn send(&mut self, mut req: Request<Body>) -> Reply {
        if let Some(cookie) = &self.cookie {
            req.headers_mut().insert(COOKIE, cookie.parse().unwrap());
        }
        let res = self.router.clone().oneshot(req).await.unwrap();
        let headers: HashMap<_, _> = res
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_owned()))
            .collect();
        if let Some(set) = headers.get(SET_COOKIE.as_str()) {
            self.cookie = Some(set.split(';').next().unwrap().to_owned());
        }
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: String::from_utf8(body.to_vec()).unwrap(),
        }
    }

    async fn get(&mut self, uri: &str) -> Reply {
        self.send(Request::get(uri).body(Body::empty()).unwrap())
            .await
    }

    async fn post(&mut self, uri: &str, body: &str) -> Reply {
        let token = self.get("/token").await.body;
        self.send(
            Request::post(uri)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header("referer", uri)
                .body(Body::from(format!("_token={token}&{body}")))
                .unwrap(),
        )
        .await
    }
}

/// The path and query of the first link in the last mail.
fn link(kernel: &Kernel) -> String {
    let mail = kernel.mailer().sent().pop().expect("a mail was sent");
    let url = mail
        .text
        .split_whitespace()
        .find(|w| w.starts_with("http"))
        .unwrap()
        .to_owned();
    url.trim_start_matches("http://127.0.0.1:3000").to_owned()
}

#[tokio::test]
async fn forgotten_passwords_are_reset_by_email() {
    let kernel = kernel(Auth::new()).await;
    User::register(kernel.db(), "Arif", "arif@example.com", "lama-rahasia")
        .await
        .unwrap();
    let mut laptop = Client::new(&kernel);
    laptop
        .post("/login", "email=arif@example.com&password=lama-rahasia")
        .await;

    let mut client = Client::new(&kernel);
    assert!(
        client
            .get("/forgot-password")
            .await
            .body
            .contains(r#"name="email""#)
    );

    // Unknown emails get the same answer and no mail.
    let reply = client
        .post("/forgot-password", "email=siapa@example.com")
        .await;
    assert_eq!(reply.location(), Some("/forgot-password"));
    assert!(kernel.mailer().sent().is_empty());

    client
        .post("/forgot-password", "email=ARIF@example.com")
        .await;
    let page = client.get("/forgot-password").await.body;
    assert!(page.contains("If that email has an account, a reset link is on its way."));
    let mails = kernel.mailer().sent();
    assert_eq!(
        (mails.len(), mails[0].to.as_str(), mails[0].subject.as_str()),
        (1, "arif@example.com", "Reset your password")
    );

    // A second request within a minute sends nothing new.
    client
        .post("/forgot-password", "email=arif@example.com")
        .await;
    assert_eq!(kernel.mailer().sent().len(), 1);

    let reset = link(&kernel);
    assert!(
        reset.starts_with("/reset-password/") && reset.ends_with("?email=arif%40example.com"),
        "{reset}"
    );
    let page = client.get(&reset).await.body;
    assert!(page.contains(r#"value="arif@example.com""#));
    let token = reset
        .trim_start_matches("/reset-password/")
        .split('?')
        .next()
        .unwrap()
        .to_owned();

    let reply = client
        .post("/reset-password", "token=salah&email=arif@example.com&password=baru-rahasia&password_confirmation=baru-rahasia")
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    assert!(
        client
            .get(&reset)
            .await
            .body
            .contains("This password reset link is invalid or has expired.")
    );

    let reply = client
        .post(
            "/reset-password",
            &format!("token={token}&email=arif@example.com&password=baru-rahasia&password_confirmation=baru-rahasia"),
        )
        .await;
    assert_eq!(reply.location(), Some("/login"));
    assert!(
        client
            .get("/login")
            .await
            .body
            .contains("Your password has been reset.")
    );

    let user = User::find_by_email(kernel.db(), "arif@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(user.check_password("baru-rahasia").await);
    assert_eq!(
        laptop.get("/api/me").await.status,
        StatusCode::SEE_OTHER,
        "old sessions end"
    );

    let again = client
        .post(
            "/reset-password",
            &format!("token={token}&email=arif@example.com&password=ketiga-rahasia&password_confirmation=ketiga-rahasia"),
        )
        .await;
    assert_eq!(again.location(), Some("/reset-password"), "links work once");
}

#[tokio::test]
async fn reset_links_expire() {
    let kernel = kernel(Auth::new()).await;
    User::register(kernel.db(), "Arif", "arif@example.com", "lama-rahasia")
        .await
        .unwrap();
    let mut client = Client::new(&kernel);
    client
        .post("/forgot-password", "email=arif@example.com")
        .await;
    let token = link(&kernel)
        .trim_start_matches("/reset-password/")
        .split('?')
        .next()
        .unwrap()
        .to_owned();

    let two_hours_ago = renox::db::now() - renox::chrono::TimeDelta::hours(2);
    renox::sqlx::query("UPDATE password_reset_tokens SET created_at = ?")
        .bind(two_hours_ago)
        .execute(kernel.db())
        .await
        .unwrap();
    client
        .post(
            "/reset-password",
            &format!("token={token}&email=arif@example.com&password=baru-rahasia&password_confirmation=baru-rahasia"),
        )
        .await;
    let user = User::find_by_email(kernel.db(), "arif@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(user.check_password("lama-rahasia").await);
}

#[tokio::test]
async fn new_users_verify_their_email_with_a_signed_link() {
    let kernel = kernel(Auth::new().verify_email()).await;
    let mut client = Client::new(&kernel);
    client
        .post(
            "/register",
            "name=Arif&email=arif@example.com&password=rahasia123&password_confirmation=rahasia123",
        )
        .await;
    assert_eq!(
        kernel.mailer().sent()[0].subject,
        "Verify your email address"
    );
    assert_eq!(
        client.get("/members").await.location(),
        Some("/verify-email")
    );
    assert!(
        client
            .get("/verify-email")
            .await
            .body
            .contains("Send another link")
    );

    let verify = link(&kernel);
    assert!(
        verify.contains("?expires=") && verify.contains("&signature="),
        "{verify}"
    );

    let tampered = verify.replace("expires=", "expires=9");
    assert_eq!(client.get(&tampered).await.status, StatusCode::FORBIDDEN);
    let unsigned = verify.split("&signature=").next().unwrap().to_owned();
    assert_eq!(client.get(&unsigned).await.status, StatusCode::FORBIDDEN);

    let reply = client.get(&verify).await;
    assert_eq!(reply.location(), Some("/"));
    assert_eq!(client.get("/members").await.body, "member Arif");
    assert_eq!(
        client.get("/verify-email").await.location(),
        Some("/"),
        "nothing left to verify"
    );

    // Another user can't use Arif's link.
    User::register(kernel.db(), "Budi", "budi@example.com", "rahasia123")
        .await
        .unwrap();
    let mut other = Client::new(&kernel);
    other
        .post("/login", "email=budi@example.com&password=rahasia123")
        .await;
    assert_eq!(other.get(&verify).await.status, StatusCode::FORBIDDEN);

    other.post("/email/verification-notification", "").await;
    assert_eq!(
        kernel.mailer().sent().last().unwrap().to,
        "budi@example.com"
    );
    assert!(
        other
            .get("/verify-email")
            .await
            .body
            .contains("A new verification link has been sent.")
    );
}

#[tokio::test]
async fn api_tokens_authenticate_without_cookies_or_csrf() {
    let kernel = kernel(Auth::new()).await;
    let user = User::register(kernel.db(), "Arif", "arif@example.com", "rahasia123")
        .await
        .unwrap();
    let token = user
        .create_token(kernel.db(), "mobile", None)
        .await
        .unwrap();
    assert!(token.plain.starts_with(&format!("{}|", token.token.id)));

    let mut api = Client::new(&kernel);
    let bearer = |req: axum::http::request::Builder, t: &str| {
        req.header("authorization", format!("Bearer {t}"))
    };
    let me = api
        .send(
            bearer(Request::get("/api/me"), &token.plain)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        (me.status, me.body.as_str()),
        (StatusCode::OK, r#"{"name":"Arif"}"#)
    );
    let post = api
        .send(
            bearer(Request::post("/api/notes"), &token.plain)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(post.body, "saved by Arif", "no CSRF token needed");
    assert!(
        user.tokens(kernel.db()).await.unwrap()[0]
            .last_used_at
            .is_some()
    );

    let wrong = api
        .send(
            bearer(
                Request::get("/api/me"),
                &format!("{}|salah", token.token.id),
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await;
    assert_eq!(
        (wrong.status, wrong.body.as_str()),
        (
            StatusCode::UNAUTHORIZED,
            r#"{"message":"Unauthenticated."}"#
        )
    );
    // A bad token is a guest, and guarded routes answer 401 rather than a login redirect.
    let forged = api
        .send(
            bearer(Request::post("/api/notes"), "1|salah")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(forged.status, StatusCode::UNAUTHORIZED);

    assert!(
        user.revoke_token(kernel.db(), token.token.id)
            .await
            .unwrap()
    );
    let revoked = api
        .send(
            bearer(Request::get("/api/me"), &token.plain)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(revoked.status, StatusCode::UNAUTHORIZED);

    let expired = user
        .create_token(
            kernel.db(),
            "old",
            Some(renox::db::now() - renox::chrono::TimeDelta::minutes(1)),
        )
        .await
        .unwrap();
    let reply = api
        .send(
            bearer(Request::get("/api/me"), &expired.plain)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

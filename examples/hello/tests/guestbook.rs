use renox::prelude::*;
use renox::testing::TestApp;
use std::time::Duration;

const DAY: u64 = 24 * 60 * 60;

/// Tests don't read `.env`; the guestbook's default language is Indonesian.
async fn app() -> TestApp {
    TestApp::with_config(hello::app(), |config| config.locale = "id".into()).await
}

#[renox::test]
async fn the_guestbook_lists_entries() {
    let app = app().await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("Buku Tamu")
        .assert_see("Belum ada pesan.");
}

#[renox::test]
async fn signing_the_guestbook_thanks_the_owner_by_mail() {
    let app = app().await;
    app.post("/entries", &[("name", "Arif"), ("message", "Kopinya enak")])
        .await
        .assert_status(303);
    app.assert_database_has(
        "entries",
        &[("name", &"Arif"), ("message", &"Kopinya enak")],
    )
    .await;
    app.get("/").await.assert_see("Kopinya enak");

    assert_eq!(app.queued_jobs().await, ["thank-guest"]);
    app.run_jobs().await;
    app.assert_mail_sent("owner@example.com", "Arif menulis di buku tamu");
}

#[renox::test]
async fn empty_entries_are_rejected() {
    let app = app().await;
    app.htmx()
        .post("/entries", &[("name", ""), ("message", "x")])
        .await
        .assert_invalid("name")
        .assert_invalid("message");
    app.assert_database_count("entries", 0).await;
}

#[renox::test]
async fn the_prune_command_deletes_old_entries() {
    let app = app().await;
    app.post("/entries", &[("name", "Arif"), ("message", "Kopinya enak")])
        .await;
    // Forty days later, another entry; then the command, on the moved clock.
    app.travel(Duration::from_secs(40 * DAY));
    app.post("/entries", &[("name", "Budi"), ("message", "Tehnya juga")])
        .await;
    let prune = |args: &'static [&'static str], answer: &'static str| {
        app.at_travelled_time(renox::prompt::answering(
            [answer],
            app.kernel().call("entries:prune", args.iter().copied()),
        ))
    };
    // "Delete 1 entries older than 30 days?" → no.
    prune(&[], "no").await.unwrap();
    app.assert_database_count("entries", 2).await;
    prune(&["--days", "30"], "yes").await.unwrap();
    app.assert_database_count("entries", 1).await;
    app.assert_database_has("entries", &[("name", &"Budi")])
        .await;
    // clap checks the arguments.
    assert!(prune(&["--days", "x"], "yes").await.is_err());
}

#[renox::test]
async fn a_logged_in_guest_can_open_their_account() {
    let app = app().await;
    app.get("/account").await.assert_redirect("/login");

    let user = User::register(app.db(), "Budi", "budi@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
    // The layout links the name to the account page.
    app.get("/")
        .await
        .assert_ok()
        .assert_see(r#"<a href="/account">Budi</a>"#);
    app.get("/account")
        .await
        .assert_ok()
        .assert_see("Akun kamu")
        .assert_see("budi@example.com");
}

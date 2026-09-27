use renox::testing::TestApp;

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

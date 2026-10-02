use renox::prelude::*;
use renox::testing::TestApp;
use uploads::Document;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR not a real image, but it starts like one";
const PDF: &[u8] = b"%PDF-1.7\n% an invoice\n";

#[renox::test]
async fn photos_are_checked_by_content_and_served_publicly() {
    let app = TestApp::new(uploads::app()).await;
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Fake")],
            &[
                ("photos", "kopi.png", PNG),
                ("photos", "fake.png", b"just text"),
            ],
        )
        .await
        .assert_invalid("photos.1");

    app.post_multipart(
        "/photos",
        &[("title", "Kopi")],
        &[("photos", "kopi.png", PNG)],
    )
    .await
    .assert_redirect("/");
    // Several at once, from htmx: each becomes a document.
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Menu")],
            &[("photos", "a.png", PNG), ("photos", "b.png", PNG)],
        )
        .await
        .assert_hx_redirect("/");
    assert_eq!(
        Document::query()
            .where_like("title", "Menu (%")
            .count(app.db())
            .await
            .unwrap(),
        2
    );
    let photo = Document::where_eq("title", "Kopi")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    let url = app.state().storage.url(&photo.file_key);
    assert!(url.starts_with("/storage/photos/"), "{url}");
    app.get(&url).await.assert_ok();
    app.get("/").await.assert_see(&url);
}

#[renox::test]
async fn invoices_are_private_behind_expiring_links() {
    let app = TestApp::new(uploads::app()).await;
    app.htmx()
        .post_multipart(
            "/invoices",
            &[("title", "Not a PDF")],
            &[("invoice", "x.pdf", PNG)],
        )
        .await
        .assert_invalid("invoice");
    app.post_multipart(
        "/invoices",
        &[("title", "September")],
        &[("invoice", "sept.pdf", PDF)],
    )
    .await
    .assert_redirect("/");

    let invoice = Document::where_eq("title", "September")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    // Not reachable by a guessable URL…
    app.get(&format!("/storage/{}", invoice.file_key))
        .await
        .assert_not_found();
    // …only through the signed link the download route hands out.
    let res = app.get(&format!("/invoices/{}/download", invoice.id)).await;
    res.assert_status(303);
    let link = res.header("location").unwrap().to_owned();
    assert!(link.contains("signature="), "{link}");
    assert_eq!(
        app.get(&link).await.assert_ok().text(),
        String::from_utf8_lossy(PDF)
    );
    app.get(&link.replace("signature=", "signature=x"))
        .await
        .assert_forbidden();
    // Five minutes later the link is dead.
    app.travel(std::time::Duration::from_secs(301));
    app.get(&link).await.assert_forbidden();
    app.travel_back();
    // Or sent by the app, shown in the browser under its original name.
    let res = app.get(&format!("/invoices/{}", invoice.id)).await;
    res.assert_ok()
        .assert_header("content-type", "application/pdf");
    assert!(
        res.header("content-disposition")
            .unwrap()
            .starts_with("inline; filename=\"sept.pdf\"")
    );
}

#[renox::test]
async fn photo_uploads_have_limits() {
    let app = TestApp::new(uploads::app()).await;
    // More than ten files at once.
    let many: Vec<(&str, &str, &[u8])> = (0..11).map(|_| ("photos", "a.png", PNG)).collect();
    app.htmx()
        .post_multipart("/photos", &[("title", "Too many")], &many)
        .await
        .assert_invalid("photos");
    // A photo over 2 MB.
    let mut big = PNG.to_vec();
    big.resize(2049 * 1024, 0);
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Too big")],
            &[("photos", "big.png", &big)],
        )
        .await
        .assert_invalid("photos.0");
    app.assert_database_count("documents", 0).await;
}

#[renox::test]
async fn uploads_say_so_on_the_next_page() {
    let app = TestApp::new(uploads::app()).await;
    app.get("/").await.assert_see("Nothing uploaded yet");

    app.post_multipart(
        "/photos",
        &[("title", "Kopi")],
        &[("photos", "kopi.png", PNG)],
    )
    .await
    .assert_redirect("/");
    app.get("/")
        .await
        .assert_see("Photo uploaded.")
        .assert_see(r#"<table class="rx-table">"#)
        .assert_dont_see("Nothing uploaded yet");
    // Once only.
    app.get("/").await.assert_dont_see("Photo uploaded.");

    // From htmx: `HX-Redirect` makes the browser load the page anew, so the
    // toast waits in the session for it too.
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Menu")],
            &[("photos", "a.png", PNG), ("photos", "b.png", PNG)],
        )
        .await
        .assert_hx_redirect("/");
    app.get("/").await.assert_see("2 photos uploaded.");

    app.post_multipart(
        "/invoices",
        &[("title", "September")],
        &[("invoice", "sept.pdf", PDF)],
    )
    .await
    .assert_redirect("/");
    let invoice = Document::where_eq("title", "September")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    app.get("/")
        .await
        .assert_see("Invoice uploaded. It stays private.")
        .assert_see(&format!(r#"href="/invoices/{}/download""#, invoice.id))
        .assert_see(r#"<span class="rx-badge rx-badge--warning">Private</span>"#);
}

#[renox::test]
async fn deleting_a_document_removes_its_file() {
    let app = TestApp::new(uploads::app()).await;
    app.post_multipart(
        "/photos",
        &[("title", "Kopi")],
        &[("photos", "kopi.png", PNG)],
    )
    .await;
    app.post_multipart(
        "/invoices",
        &[("title", "September")],
        &[("invoice", "sept.pdf", PDF)],
    )
    .await;
    let storage = app.state().storage.clone();

    for title in ["Kopi", "September"] {
        let doc = Document::where_eq("title", title)
            .first(app.db())
            .await
            .unwrap()
            .unwrap();
        assert!(storage.exists(&doc.file_key).await.unwrap());
        app.get("/")
            .await
            .assert_see(&format!(r#"action="/documents/{}""#, doc.id));

        app.delete(&format!("/documents/{}", doc.id))
            .await
            .assert_redirect("/");
        assert!(Document::find(app.db(), doc.id).await.unwrap().is_none());
        assert!(!storage.exists(&doc.file_key).await.unwrap());
        app.get("/")
            .await
            .assert_see(&format!("“{title}” deleted."));
    }
    app.assert_database_count("documents", 0).await;
    app.get("/").await.assert_see("Nothing uploaded yet");
    // Gone: a second delete is a 404.
    app.delete("/documents/1").await.assert_status(404);
}

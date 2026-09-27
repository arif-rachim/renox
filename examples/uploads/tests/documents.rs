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
            &[("photo", "fake.png", b"just text")],
        )
        .await
        .assert_invalid("photo");

    app.post_multipart(
        "/photos",
        &[("title", "Kopi")],
        &[("photo", "kopi.png", PNG)],
    )
    .await
    .assert_redirect("/");
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
}

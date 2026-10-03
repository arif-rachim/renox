use renox::prelude::*;
use renox::testing::TestApp;
use uploads::Document;

/// Not a real image, but it starts like one: a PNG header saying 800 × 600.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x03\x20\0\0\x02\x58 and no pixels";
/// The same, 8000 pixels wide.
const WIDE_PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x1f\x40\0\0\x02\x58 and no pixels";
const PDF: &[u8] = b"%PDF-1.7\n% an invoice\n";

#[renox::test]
async fn photos_are_checked_by_content_and_served_publicly() {
    let app = TestApp::new(uploads::app()).await;
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Fake")],
            &[
                ("photos", "coffee.png", PNG),
                ("photos", "fake.png", b"just text"),
            ],
        )
        .await
        .assert_invalid("photos.1");

    app.post_multipart(
        "/photos",
        &[("title", "Coffee")],
        &[("photos", "coffee.png", PNG)],
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
    let photo = Document::where_eq("title", "Coffee")
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
    // Wider than 6000 pixels: the header says so, whatever the file size.
    app.htmx()
        .post_multipart(
            "/photos",
            &[("title", "Too wide")],
            &[("photos", "wide.png", WIDE_PNG)],
        )
        .await
        .assert_invalid("photos.0")
        .assert_see("invalid image dimensions");
    app.assert_database_count("documents", 0).await;
}

#[renox::test]
async fn uploads_say_so_on_the_next_page() {
    let app = TestApp::new(uploads::app()).await;
    app.get("/").await.assert_see("Nothing uploaded yet");

    app.post_multipart(
        "/photos",
        &[("title", "Coffee")],
        &[("photos", "coffee.png", PNG)],
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
        &[("title", "Coffee")],
        &[("photos", "coffee.png", PNG)],
    )
    .await;
    app.post_multipart(
        "/invoices",
        &[("title", "September")],
        &[("invoice", "sept.pdf", PDF)],
    )
    .await;
    let storage = app.state().storage.clone();

    for title in ["Coffee", "September"] {
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

/// The same app with its files on S3: a real S3-compatible server when
/// `TEST_S3_ENDPOINT` is set (CI's `s3` job runs SeaweedFS; the commands are
/// at the top of crates/renox/tests/it/s3.rs), else nothing to do:
///
/// ```text
/// TEST_S3_ENDPOINT=http://127.0.0.1:8333 TEST_S3_BUCKET=renox-test \
///     TEST_S3_ACCESS_KEY_ID=renox TEST_S3_SECRET_ACCESS_KEY=renox-secret \
///     cargo test -p uploads --features s3
/// ```
#[cfg(feature = "s3")]
mod on_s3 {
    use super::{PDF, PNG};
    use renox::prelude::*;
    use renox::testing::TestApp;
    use uploads::Document;

    async fn s3_app() -> Option<TestApp> {
        let endpoint = std::env::var("TEST_S3_ENDPOINT").ok()?;
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"));
        let (bucket, id, secret) = (
            var("TEST_S3_BUCKET"),
            var("TEST_S3_ACCESS_KEY_ID"),
            var("TEST_S3_SECRET_ACCESS_KEY"),
        );
        Some(
            TestApp::with_config(uploads::app(), |c| {
                c.storage.disk = renox::storage::DiskDriver::S3;
                c.storage.endpoint = Some(endpoint.clone());
                c.storage.bucket = Some(bucket.clone());
                c.storage.region = Some("us-east-1".into());
                c.storage.access_key_id = Some(id);
                c.storage.secret_access_key = Some(secret);
                // Public files are read straight from the bucket.
                c.storage.url = Some(format!("{endpoint}/{bucket}"));
            })
            .await,
        )
    }

    /// GET over plain HTTP (the test server speaks http): status and body.
    async fn http_get(url: &str) -> (u16, Vec<u8>) {
        use renox::tokio::io::{AsyncReadExt, AsyncWriteExt};
        let rest = url.strip_prefix("http://").expect("an http:// URL");
        let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let mut stream = renox::tokio::net::TcpStream::connect(host).await.unwrap();
        let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&response[..split]);
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, response[split + 4..].to_vec())
    }

    #[renox::test]
    async fn photos_and_invoices_live_on_s3() {
        let Some(app) = s3_app().await else {
            return;
        };
        app.post_multipart(
            "/photos",
            &[("title", "Coffee")],
            &[("photos", "coffee.png", PNG)],
        )
        .await
        .assert_redirect("/");
        app.post_multipart(
            "/invoices",
            &[("title", "March")],
            &[("invoice", "march.pdf", PDF)],
        )
        .await
        .assert_redirect("/");
        let photo = Document::where_eq("kind", "photo")
            .first(app.db())
            .await
            .unwrap()
            .unwrap();
        let invoice = Document::where_eq("kind", "invoice")
            .first(app.db())
            .await
            .unwrap()
            .unwrap();
        let storage = &app.state().storage;
        assert_eq!(
            storage.get(&invoice.file_key).await.unwrap().as_deref(),
            Some(PDF)
        );

        // The page links the photo at its address on the store.
        let url = storage.url(&photo.file_key);
        assert!(url.starts_with("http://"), "{url}");
        app.get("/").await.assert_see(&url);

        // An invoice's link is presigned by S3, and works without the app.
        let res = app.get(&format!("/invoices/{}/download", invoice.id)).await;
        let link = res.header("location").expect("a redirect").to_owned();
        assert!(link.contains("X-Amz-Signature"), "{link}");
        assert_eq!(http_get(&link).await, (200, PDF.to_vec()));
        // The app can send it too, with its original name.
        let file = app.get(&format!("/invoices/{}", invoice.id)).await;
        file.assert_ok();
        assert!(
            file.header("content-disposition")
                .unwrap_or_default()
                .contains("march.pdf")
        );

        // Deleting the document deletes the object.
        app.delete(&format!("/documents/{}", invoice.id)).await;
        assert!(!storage.exists(&invoice.file_key).await.unwrap());
        app.delete(&format!("/documents/{}", photo.id)).await;
        assert!(!storage.exists(&photo.file_key).await.unwrap());
    }
}

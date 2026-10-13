//! The videos: served from the disk with Range support, and the `learn:video`
//! directive points at `LEARN_MEDIA_URL`.

use renox::testing::TestApp;
use renox_learn::content;

async fn with_media_url(value: Option<&str>) -> TestApp {
    let value = value.map(str::to_owned);
    TestApp::with_config(renox_learn::app(), move |c| {
        c.url = "https://learn.renox.rs".into();
        if let Some(v) = &value {
            c.vars.insert("LEARN_MEDIA_URL".into(), v.clone());
        }
    })
    .await
}

/// The path of the page that has the stage-5 video.
fn forms_page() -> String {
    let chapter = content::chapters()
        .iter()
        .find(|c| c.number == 5)
        .expect("stage 5 is written");
    format!("/stages/{}", chapter.slug)
}

#[renox::test]
async fn a_range_request_on_storage_media_is_206() {
    let app = with_media_url(None).await;
    app.state()
        .storage
        .put("public/media/x.webm", vec![0u8; 1000].into())
        .await
        .unwrap();

    let whole = app.get("/storage/media/x.webm").await;
    whole
        .assert_status(200)
        .assert_header("accept-ranges", "bytes");
    assert_eq!(whole.body.len(), 1000);

    let part = app
        .request()
        .header("range", "bytes=0-99")
        .get("/storage/media/x.webm")
        .await;
    part.assert_status(206)
        .assert_header("content-range", "bytes 0-99/1000");
    assert_eq!(part.body.len(), 100);
}

#[renox::test]
async fn the_video_directive_uses_the_default_storage_media_path() {
    let app = with_media_url(None).await;
    let page = app.get(&forms_page()).await;
    page.assert_ok()
        .assert_see("<figure class=\"learn-video\"><video controls preload=\"none\" playsinline")
        .assert_see("poster=\"/storage/media/s05-add-item.webp?v=")
        .assert_see("<source src=\"/storage/media/s05-add-item.webm?v=")
        .assert_see("type=\"video/webm\">")
        .assert_see("<source src=\"/storage/media/s05-add-item.mp4?v=")
        .assert_see("Download the video</a>")
        .assert_dont_see("__LEARN_MEDIA__")
        .assert_dont_see("autoplay");
}

#[renox::test]
async fn the_video_directive_uses_learn_media_url() {
    let app = with_media_url(Some("/media/")).await;
    let page = app.get(&forms_page()).await;
    page.assert_ok()
        .assert_see("src=\"/media/s05-add-item.webm?v=")
        .assert_see("src=\"/media/s05-add-item.mp4?v=")
        .assert_dont_see("/storage/media/")
        .assert_dont_see("__LEARN_MEDIA__");
}

#[renox::test]
async fn an_absolute_media_url_falls_back_to_storage_media() {
    for value in ["https://cdn.example.com/media", "//cdn.example.com/media"] {
        let app = with_media_url(Some(value)).await;
        let page = app.get(&forms_page()).await;
        page.assert_ok()
            .assert_see("src=\"/storage/media/s05-add-item.webm?v=")
            .assert_dont_see("cdn.example.com");
    }
}

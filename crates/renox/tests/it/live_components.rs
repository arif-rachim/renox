//! Registering live components.

use renox::live_component::LiveContext;
use renox::prelude::*;
use renox::testing::TestApp;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Counter {
    count: i64,
}

impl LiveComponent for Counter {
    const NAME: &'static str = "counter";
    const VIEW: &'static str = "live/counter.html";

    async fn data(&self, _ctx: &LiveContext) -> Result<serde_json::Value> {
        Ok(json!({ "label": "clicks" }))
    }

    async fn call(
        &mut self,
        action: &str,
        _args: Vec<serde_json::Value>,
        _ctx: &mut LiveContext,
    ) -> Result {
        match action {
            "increment" => {
                self.count += 1;
                Ok(())
            }
            _ => Err(Error::NotFound),
        }
    }
}

#[renox::test]
async fn a_component_registers_through_the_app() {
    let app = App::with_config(Config::default()).live_component::<Counter>();
    assert!(app.boot().await.is_ok());
}

#[renox::test]
async fn a_duplicate_component_is_a_boot_error() {
    let err = App::with_config(Config::default())
        .live_component::<Counter>()
        .live_component::<Counter>()
        .boot()
        .await
        .err()
        .unwrap();
    assert!(
        format!("{err:?}").contains("live component `counter` is registered twice"),
        "{err:?}"
    );
}

async fn counter(ctx: LiveContext) -> Result<View> {
    let list = ctx.mount(Counter { count: 2 }).await?;
    Ok(view("page.html", context! { list }))
}

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/counter", counter)
    }
}

#[renox::test]
async fn a_page_mounts_a_component() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("live")).unwrap();
    std::fs::write(
        dir.path().join("page.html"),
        r#"{% set component = list %}{% include "renox/live.html" %}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("live/counter.html"),
        "<b>{{ state.count }}</b>{{ data.label }}{{ csrf_field() }}",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(
        App::new()
            .module(Auth::new())
            .module(Pages)
            .live_component::<Counter>(),
        move |c| c.views_path = path,
    )
    .await;
    let page = app.get("/counter").await;
    page.assert_ok()
        .assert_see(r#"data-rx-live="counter""#)
        .assert_see("<b>2</b>clicks")
        .assert_see(r#"name="_token""#)
        .assert_see("data-rx-snapshot=");
}

#[renox::test]
async fn a_page_with_two_components_loads_idiomorph_once() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("live")).unwrap();
    std::fs::write(
        dir.path().join("page.html"),
        r#"{% set component = list %}{% include "renox/live.html" %}{% set component = list %}{% include "renox/live.html" %}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("live/counter.html"),
        "<b>{{ state.count }}</b>",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(
        App::new()
            .module(Auth::new())
            .module(Pages)
            .live_component::<Counter>(),
        move |c| c.views_path = path,
    )
    .await;
    let body = app.get("/counter").await.text();
    assert_eq!(body.matches(r#"data-rx-live="counter""#).count(), 2);
    assert_eq!(body.matches("idiomorph-0.7.3.min.js").count(), 1);
    assert_eq!(body.matches("/_renox/live-").count(), 1);
}

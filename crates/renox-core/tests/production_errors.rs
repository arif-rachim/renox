// Separate test binary: the debug flag is process-wide.
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use renox_core::{App, AppState, Config, Module, Result};
use tower::ServiceExt;

struct Demo;

impl Module for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn routes(&self) -> Router<AppState> {
        Router::new().route("/fail", get(fail))
    }
}

async fn fail() -> Result<String> {
    let n: u8 = "not a number".parse()?;
    Ok(n.to_string())
}

#[tokio::test]
async fn internal_error_hides_detail_without_debug() {
    let config = Config {
        debug: false,
        ..Config::default()
    };
    let router = App::with_config(config).module(Demo).into_router().unwrap();
    let res = router
        .oneshot(Request::get("/fail").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("500 · Internal Server Error"));
    assert!(!body.contains("invalid digit"));
}

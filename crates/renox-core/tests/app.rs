use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Router, response::Response};
use http_body_util::BodyExt;
use renox_core::{App, AppState, Config, Module, Result};
use tower::ServiceExt;

struct Demo;

impl Module for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn routes(&self) -> Router<AppState> {
        Router::new()
            .route(
                "/",
                get(|State(s): State<AppState>| async move { s.config.name.clone() }),
            )
            .route("/fail", get(fail))
    }
}

async fn fail() -> Result<String> {
    let n: u8 = "not a number".parse()?;
    Ok(n.to_string())
}

async fn call(debug: bool, uri: &str) -> (StatusCode, String) {
    let config = Config {
        name: "Test App".into(),
        debug,
        ..Config::default()
    };
    let router = App::with_config(config).module(Demo).into_router().unwrap();
    let res: Response = router
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn module_routes_receive_app_state() {
    let (status, body) = call(true, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Test App");
}

#[tokio::test]
async fn unknown_route_renders_404_page() {
    let (status, body) = call(true, "/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("404 · Not Found"));
}

#[tokio::test]
async fn internal_error_shows_detail_in_debug() {
    let (status, body) = call(true, "/fail").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body.contains("invalid digit"));
}

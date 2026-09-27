// Separate test binary: the debug flag is process-wide.
mod support;

use axum::http::StatusCode;
use renox_core::{Module, Result, Routes};
use support::TestApp;

struct Demo;

impl Module for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/fail", fail)
    }
}

async fn fail() -> Result<String> {
    let n: u8 = "not a number".parse()?;
    Ok(n.to_string())
}

#[tokio::test]
async fn internal_error_hides_detail_without_debug() {
    let mut app = TestApp::new(false, &[], |app| app.module(Demo)).await;
    let res = app.get("/fail").await;
    assert_eq!(res.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(res.body.contains("500 · Internal Server Error"));
    assert!(!res.body.contains("invalid digit"));
}

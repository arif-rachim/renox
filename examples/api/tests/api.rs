use renox::prelude::*;
use renox::testing::TestApp;

async fn app() -> TestApp {
    let app = TestApp::new(api::app()).await;
    User::register(app.db(), "Arif", "arif@example.com", "password123")
        .await
        .unwrap();
    app
}

/// Logs in through the API and returns `Bearer <token>`.
async fn bearer(app: &TestApp) -> String {
    let res = app
        .request()
        .without_csrf()
        .post_json(
            "/api/tokens",
            &json!({ "email": "arif@example.com", "password": "password123", "device": "test" }),
        )
        .await;
    res.assert_ok();
    let body: serde_json::Value = res.json();
    format!("Bearer {}", body["token"].as_str().unwrap())
}

#[renox::test]
async fn tokens_are_issued_for_the_right_password_only() {
    let app = app().await;
    let wrong = json!({ "email": "arif@example.com", "password": "nope", "device": "test" });
    let unknown = json!({ "email": "who@example.com", "password": "nope", "device": "test" });
    for body in [wrong, unknown] {
        app.request()
            .without_csrf()
            .json()
            .post_json("/api/tokens", &body)
            .await
            .assert_status(401);
    }
    let missing = app
        .request()
        .without_csrf()
        .post_json("/api/tokens", &json!({ "email": "not-an-email" }))
        .await;
    missing.assert_status(422);
    let errors: serde_json::Value = missing.json();
    assert!(errors["errors"]["email"].is_array());
    assert!(errors["errors"]["password"].is_array());
    assert!(bearer(&app).await.starts_with("Bearer "));
}

#[renox::test]
async fn the_api_needs_a_token() {
    let app = app().await;
    app.request()
        .json()
        .get("/api/products")
        .await
        .assert_status(401);
    app.request()
        .json()
        .header("authorization", "Bearer 1|forged")
        .get("/api/products")
        .await
        .assert_status(401);

    let token = bearer(&app).await;
    let res = app
        .request()
        .header("authorization", &token)
        .get("/api/products")
        .await;
    res.assert_ok();
    let page: serde_json::Value = res.json();
    assert_eq!(page["total"], 0);
}

#[renox::test]
async fn products_are_created_with_json_and_validated() {
    let app = app().await;
    let token = bearer(&app).await;
    // No CSRF token: requests with a Bearer token don't need one.
    let post = |body: serde_json::Value| {
        let (app, token) = (&app, token.clone());
        async move {
            app.request()
                .without_csrf()
                .header("authorization", &token)
                .post_json("/api/products", &body)
                .await
        }
    };
    let created = post(json!({ "name": "Kopi", "price": 18000 })).await;
    created.assert_status(201);
    let product: serde_json::Value = created.json();
    assert_eq!(product["name"], "Kopi");

    let invalid = post(json!({ "name": "Kopi", "price": -1 })).await;
    invalid.assert_status(422);
    let body: serde_json::Value = invalid.json();
    assert!(
        body["errors"]["name"][0]
            .as_str()
            .unwrap()
            .contains("taken")
    );
    assert!(body["errors"]["price"].is_array());

    let id = product["id"].as_i64().unwrap();
    let res = app
        .request()
        .header("authorization", &token)
        .get(&format!("/api/products/{id}"))
        .await;
    res.assert_ok();
    let missing = app
        .request()
        .json()
        .header("authorization", &token)
        .get("/api/products/999")
        .await;
    missing.assert_status(404);
    println!("404 body: {}", missing.text());
}

#[renox::test]
async fn revoked_tokens_stop_working() {
    let app = app().await;
    let phone = bearer(&app).await;
    let laptop = bearer(&app).await;
    let status = |token: String| {
        let app = &app;
        async move {
            app.request()
                .json()
                .header("authorization", &token)
                .get("/api/products")
                .await
                .status
                .as_u16()
        }
    };
    // Logging out on the phone leaves the laptop logged in.
    app.request()
        .without_csrf()
        .header("authorization", &phone)
        .delete("/api/tokens/current")
        .await
        .assert_status(204);
    assert_eq!(status(phone.clone()).await, 401);
    assert_eq!(status(laptop.clone()).await, 200);

    let tablet = bearer(&app).await;
    app.request()
        .without_csrf()
        .header("authorization", &laptop)
        .delete("/api/tokens")
        .await
        .assert_status(204);
    assert_eq!(status(laptop).await, 401);
    assert_eq!(status(tablet).await, 401);
}

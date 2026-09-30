use renox::prelude::*;
use renox::testing::TestApp;
use std::time::Duration;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

async fn app() -> TestApp {
    let app = TestApp::new(api::app()).await;
    User::register(app.db(), "Arif", "arif@example.com", "password123")
        .await
        .unwrap();
    app
}

/// Logs in through the API and returns `Bearer <token>` (read and write).
async fn bearer(app: &TestApp) -> String {
    login(app, false).await
}

async fn login(app: &TestApp, read_only: bool) -> String {
    let res = app
        .request()
        .without_csrf()
        .post_json(
            "/api/tokens",
            &json!({
                "email": "arif@example.com",
                "password": "password123",
                "device": "test",
                "read_only": read_only,
            }),
        )
        .await;
    res.assert_ok();
    let body: serde_json::Value = res.json();
    assert!(body["expires_at"].is_string());
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
    assert!(missing.json_path("errors.email").is_array());
    assert!(missing.json_path("errors.password").is_array());
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
    res.assert_ok()
        .assert_json_path("items", json!([]))
        .assert_json_path("next_cursor", json!(null));
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
    created
        .assert_status(201)
        .assert_json(json!({ "name": "Kopi", "price": 18000 }));

    let invalid = post(json!({ "name": "Kopi", "price": -1 })).await;
    invalid.assert_status(422);
    assert!(
        invalid
            .json_path("errors.name.0")
            .as_str()
            .unwrap()
            .contains("taken")
    );
    assert!(invalid.json_path("errors.price").is_array());

    let id = created.json_path("id").as_str().unwrap().to_owned();
    assert_eq!(id.len(), 26, "a ULID");
    app.request()
        .header("authorization", &token)
        .get(&format!("/api/products/{id}"))
        .await
        .assert_ok()
        .assert_json_path("name", "Kopi");
    let missing = app
        .request()
        .json()
        .header("authorization", &token)
        .get("/api/products/01J9Z3ABCDEFGHJKMNPQRSTVWX")
        .await;
    missing.assert_status(404);
    // A malformed id is a 404 too, not a 400.
    app.request()
        .json()
        .header("authorization", &token)
        .get("/api/products/42")
        .await
        .assert_status(404);
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

#[renox::test]
async fn read_only_tokens_cannot_write() {
    let app = app().await;
    let reader = login(&app, true).await;
    let writer = bearer(&app).await;
    let create = |token: String| {
        let app = &app;
        async move {
            app.request()
                .without_csrf()
                .json()
                .header("authorization", &token)
                .post_json("/api/products", &json!({ "name": "Kopi", "price": 18000 }))
                .await
        }
    };
    create(reader.clone()).await.assert_status(403);
    let created = create(writer.clone()).await;
    created.assert_status(201);
    let id = created.json_path("id").as_str().unwrap().to_owned();

    // The read-only token still reads, but can't delete.
    app.request()
        .header("authorization", &reader)
        .get(&format!("/api/products/{id}"))
        .await
        .assert_ok();
    let delete = |token: String| {
        let (app, id) = (&app, id.clone());
        async move {
            app.request()
                .without_csrf()
                .json()
                .header("authorization", &token)
                .delete(&format!("/api/products/{id}"))
                .await
                .status
                .as_u16()
        }
    };
    assert_eq!(delete(reader).await, 403);
    assert_eq!(delete(writer.clone()).await, 204);
    assert_eq!(delete(writer).await, 404);
}

/// The status of `GET /api/products` with `token`.
async fn list_status(app: &TestApp, token: &str) -> u16 {
    app.request()
        .json()
        .header("authorization", token)
        .get("/api/products")
        .await
        .status
        .as_u16()
}

#[renox::test]
async fn tokens_expire_after_thirty_days() {
    let app = app().await;
    let token = bearer(&app).await;
    app.travel(29 * DAY);
    assert_eq!(list_status(&app, &token).await, 200);
    app.travel(2 * DAY);
    assert_eq!(list_status(&app, &token).await, 401);
}

#[renox::test]
async fn expired_tokens_stop_working() {
    let app = app().await;
    let user = User::find_by_email(app.db(), "arif@example.com")
        .await
        .unwrap()
        .unwrap();
    let yesterday = renox::db::now() - renox::chrono::TimeDelta::days(1);
    let token = user
        .create_token_with(app.db(), "old", &["products:read"], Some(yesterday))
        .await
        .unwrap();
    app.request()
        .json()
        .header("authorization", &format!("Bearer {}", token.plain))
        .get("/api/products")
        .await
        .assert_status(401);
}

#[renox::test]
async fn products_are_listed_with_a_cursor() {
    let app = app().await;
    for i in 1..=25 {
        let product = api::Product {
            name: format!("Product {i:02}"),
            price: i,
            ..Default::default()
        };
        api::Product::create(app.db(), product).await.unwrap();
    }
    let token = bearer(&app).await;
    let get = |url: String| {
        let (app, token) = (&app, token.clone());
        async move {
            let res = app
                .request()
                .header("authorization", &token)
                .get(&url)
                .await;
            res.assert_ok();
            res.json::<serde_json::Value>()
        }
    };
    // Newest first, 20 at a time.
    let first = get("/api/products".into()).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 20);
    assert_eq!(first["items"][0]["name"], "Product 25");
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();

    let second = get(format!("/api/products?cursor={cursor}")).await;
    let names: Vec<&str> = second["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Product 05",
            "Product 04",
            "Product 03",
            "Product 02",
            "Product 01"
        ]
    );
    assert!(second["next_cursor"].is_null());

    app.request()
        .json()
        .header("authorization", &token)
        .get("/api/products?cursor=nope")
        .await
        .assert_status(400);
}

#[renox::test]
async fn guests_are_limited_per_ip_and_users_per_account() {
    let app = app().await;
    let token = bearer(&app).await; // one guest request
    // Invalid input (422), so the login lock doesn't count these.
    let wrong = json!({ "email": "not-an-email" });
    for _ in 0..9 {
        app.request()
            .without_csrf()
            .post_json("/api/tokens", &wrong)
            .await
            .assert_status(422);
    }
    let res = app
        .request()
        .without_csrf()
        .post_json("/api/tokens", &wrong)
        .await;
    res.assert_status(429);
    assert!(res.header("retry-after").is_some());

    // A logged-in user counts on their own, with a higher limit.
    let res = app
        .request()
        .header("authorization", &token)
        .get("/api/products")
        .await;
    res.assert_ok().assert_header("x-ratelimit-limit", "120");

    // A minute later the guest may try again.
    app.travel(Duration::from_secs(61));
    app.request()
        .without_csrf()
        .post_json("/api/tokens", &wrong)
        .await
        .assert_status(422);
}

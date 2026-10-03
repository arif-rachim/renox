//! M14b: template filters and hooks, shared view data, provided values,
//! app layers, extending users, registration hooks, async gates and the
//! development error page.

use std::time::Duration;

use renox::Provided;
use renox::axum::extract::Request;
use renox::axum::middleware::{Next, from_fn};
use renox::db::Migration;
use renox::prelude::*;
use renox::testing::TestApp;
use renox::view::ViewContext;

const ADD_COLUMNS: Migration = Migration::new(
    "20300101000000_add_role_and_phone_to_users",
    "ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member';
     ALTER TABLE users ADD COLUMN phone TEXT;",
    None,
);

#[derive(Clone)]
struct Shop {
    name: String,
}

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { view("home.html", context! {}) })
            .name("home")
            .get("/prices", || async {
                view(
                    "prices.html",
                    context! { price => 75_000, at => "2026-10-01T17:30:00Z", day => "2026-10-01" },
                )
            })
            .get("/menu", || async { view("menu.html", context! {}) })
            .get("/menu-override", || async {
                view("menu.html", context! { cart_count => 99 })
            })
            .get(
                "/shop",
                |shop: Provided<Shop>| async move { shop.name.clone() },
            )
            .get("/missing", |_: Provided<String>| async { "never" })
            .get("/typo", || async {
                view(
                    "typo.html",
                    context! { product => context! { name => "Coffee" } },
                )
            })
            .get("/flash", || async { view("flash.html", context! {}) })
            .get("/me", || async { view("me.html", context! {}) })
            .get("/me.json", |user: AuthUser| async move {
                Json(user.user().clone())
            })
            .get("/admin", |user: AuthUser| async move {
                user.gate("admin").map(|()| "admin area")
            })
            .get("/billing", |user: AuthUser| async move {
                user.gate_async("billing").await.map(|()| "billing")
            })
            .get("/billing-sync", |user: AuthUser| async move {
                if user.allows("billing") { "yes" } else { "no" }
            })
    }
}

async fn stamp(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let who = if user.is_some() { "member" } else { "guest" };
    res.headers_mut().insert("x-visitor", who.parse().unwrap());
    res
}

async fn order_a(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let seen = res
        .headers()
        .get("x-order")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    res.headers_mut()
        .insert("x-order", format!("a{seen}").parse().unwrap());
    res
}

async fn order_b(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    res.headers_mut().insert("x-order", "b".parse().unwrap());
    res
}

fn views() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| std::fs::write(dir.path().join(name), body).unwrap();
    write("home.html", "home");
    write(
        "prices.html",
        "{{ price | number }}|{{ 1234.5 | number(2) }}|{{ price | rupiah }}|{{ at | date('%d/%m/%Y %H:%M') }}|{{ day | date }}",
    );
    write("menu.html", "{{ cart_count }} items for {{ who }}");
    write("typo.html", "<h1>{{ prodcut }}</h1>");
    write("flash.html", "[{{ flash.status }}]");
    write("me.html", "{{ auth.user.name }} is {{ auth.user.role }}");
    dir
}

fn app() -> App {
    App::new()
        .migrations(&[ADD_COLUMNS])
        .module(Auth::new())
        .module(Pages)
        .templates(|env| {
            env.add_filter("rupiah", |n: i64| {
                format!("Rp {}", renox::format_number(n as f64, 0, "de"))
            });
        })
        .share("cart_count", |ctx: ViewContext| async move {
            Ok(if ctx.user.is_some() { 3 } else { 0 })
        })
        .share("who", |ctx: ViewContext| async move {
            Ok(ctx.user.map_or("guest".to_owned(), |u| u.name.clone()))
        })
        .provide(Shop {
            name: "Coffee Shop".into(),
        })
        .layer(from_fn(stamp))
        .layer(from_fn(order_a))
        .layer(from_fn(order_b))
        .gate("admin", |user| {
            user.get::<String>("role").as_deref() == Some("admin")
        })
        .gate_async("billing", |user, state| async move {
            let phone: Option<String> = renox::db::sql("SELECT phone FROM users WHERE id = ?")
                .bind(user.id)
                .scalar(&state.db)
                .await?;
            Ok(phone.is_some())
        })
}

async fn test_app(dir: &tempfile::TempDir, locale: &str, debug: bool) -> TestApp {
    let (views, locale) = (dir.path().to_path_buf(), locale.to_owned());
    TestApp::with_config(app(), move |c| {
        c.views_path = views;
        c.locale = locale;
        c.debug = debug;
        c.timezone = "+07:00".parse().unwrap();
    })
    .await
}

#[renox::test]
async fn filters_and_template_hooks() {
    let dir = views();
    let app = test_app(&dir, "de", true).await;
    app.get("/prices")
        .await
        .assert_ok()
        .assert_see("75.000|1.234,50|Rp 75.000|02/10/2026 00:30|2026-10-01");
    let app = test_app(&dir, "en", true).await;
    app.get("/prices")
        .await
        .assert_see("75,000|1,234.50|Rp 75.000|");
}

#[renox::test]
async fn shared_view_data_follows_the_user_and_yields_to_the_handler() {
    let dir = views();
    let app = test_app(&dir, "en", true).await;
    app.get("/menu").await.assert_see("0 items for guest");
    let user = User::register(app.db(), "Alex", "alex@example.com", "letmein123")
        .await
        .unwrap();
    app.acting_as(&user);
    app.get("/menu").await.assert_see("3 items for Alex");
    app.get("/menu-override")
        .await
        .assert_see("99 items for Alex");
}

#[renox::test]
async fn provided_values_and_app_layers() {
    let dir = views();
    let app = test_app(&dir, "en", true).await;
    app.get("/shop").await.assert_see("Coffee Shop");
    assert_eq!(
        app.state().provided::<Shop>().map(|s| s.name.clone()),
        Some("Coffee Shop".to_owned())
    );
    let missing = app.get("/missing").await;
    missing.assert_status(500);
    assert!(
        missing
            .text()
            .contains("no alloc::string::String was provided"),
        "{}",
        missing.text()
    );

    let res = app.get("/shop").await;
    res.assert_header("x-visitor", "guest");
    // The first layer added sees the response last: `a` wraps `b`.
    res.assert_header("x-order", "ab");
    // Framework routes aren't wrapped.
    assert!(app.get("/health").await.header("x-visitor").is_none());
}

#[renox::test]
async fn users_keep_the_apps_own_columns() {
    let dir = views();
    let app = test_app(&dir, "en", true).await;
    let mut user = User::register(app.db(), "Alex", "alex@example.com", "letmein123")
        .await
        .unwrap();
    assert_eq!(user.get::<String>("role"), Some("member".into()));
    user.set(app.db(), "role", "admin").await.unwrap();
    assert_eq!(user.get::<String>("role"), Some("admin".into()));
    assert!(user.set(app.db(), "password", "x").await.is_err());
    assert!(
        user.set(app.db(), "role; DROP TABLE users", "x")
            .await
            .is_err()
    );

    let admins = User::where_eq("role", "admin").get(app.db()).await.unwrap();
    assert_eq!(admins.len(), 1);

    app.acting_as(&user);
    app.get("/me").await.assert_see("Alex is admin");
    app.get("/admin").await.assert_see("admin area");
    let json: renox::serde_json::Value = app.get("/me.json").await.json();
    assert_eq!(json["role"], "admin");
    assert!(json.get("password").is_none() && json.get("sessions_revoked_at").is_none());
}

#[renox::test]
async fn async_gates_can_query_the_database() {
    let dir = views();
    let app = test_app(&dir, "en", true).await;
    let mut user = User::register(app.db(), "Alex", "alex@example.com", "letmein123")
        .await
        .unwrap();
    app.acting_as(&user);
    app.get("/billing").await.assert_forbidden();
    user.set(app.db(), "phone", "0812").await.unwrap();
    app.get("/billing").await.assert_ok().assert_see("billing");
    // A plain check can't wait for an async gate: it denies.
    app.get("/billing-sync").await.assert_see("no");
}

#[renox::test]
async fn registration_hooks_validate_and_save_extra_fields() {
    let dir = views();
    let (views, _) = (dir.path().to_path_buf(), ());
    let app = TestApp::with_config(
        App::new()
            .migrations(&[ADD_COLUMNS])
            .module(
                Auth::new()
                    .registration_rules(|form, v| {
                        v.field("phone", &form.get("phone")).required().max(20);
                    })
                    .on_registered(|mut user, form, state| async move {
                        if form.get("phone") == "000" {
                            return Err(Error::BadRequest("that number is blocked".into()));
                        }
                        user.set(&state.db, "phone", form.get("phone")).await?;
                        let first = User::query().count(&state.db).await? == 1;
                        user.set(&state.db, "role", if first { "admin" } else { "member" })
                            .await
                    }),
            )
            .module(Pages),
        move |c| c.views_path = views,
    )
    .await;
    let form = |email: &'static str, phone: &'static str| {
        vec![
            ("name", "Alex"),
            ("email", email),
            ("password", "letmein123"),
            ("password_confirmation", "letmein123"),
            ("phone", phone),
        ]
    };
    // Built-in and extra errors come together.
    let res = app
        .htmx()
        .post(
            "/register",
            &[("name", ""), ("email", "x"), ("password", "letmein123")],
        )
        .await;
    res.assert_invalid("phone").assert_invalid("email");

    app.post("/register", &form("alex@example.com", "0812"))
        .await;
    app.assert_database_has(
        "users",
        &[
            ("email", &"alex@example.com"),
            ("phone", &"0812"),
            ("role", &"admin"),
        ],
    )
    .await;

    app.logout();
    let res = app.post("/register", &form("ben@example.com", "000")).await;
    res.assert_status(400);
    app.assert_database_missing("users", &[("email", &"ben@example.com")])
        .await;
}

#[renox::test]
async fn misspelled_variables_fail_while_developing() {
    let dir = views();
    let app = test_app(&dir, "en", true).await;
    let res = app.get("/typo").await;
    res.assert_status(500);
    let page = res.text();
    assert!(page.contains("GET /typo"), "{page}");
    assert!(page.contains("typo.html"), "the template and line: {page}");
    // A flashed value that isn't there is just empty.
    app.get("/flash").await.assert_ok().assert_see("[]");

    let live = test_app(&dir, "en", false).await;
    live.get("/typo").await.assert_ok().assert_see("<h1></h1>");
    let _ = Duration::ZERO;
}

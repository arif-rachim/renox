//! M21g: sessions kept in the database (`SESSION_DRIVER=database`).

use renox::prelude::*;
use renox::testing::TestApp;

struct Probe;

impl Module for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get(
                "/put/{v}",
                |session: Session, Path(v): Path<String>| async move {
                    session.put("v", v)?;
                    Ok::<_, Error>("ok")
                },
            )
            .get("/big", |session: Session| async move {
                session.put("big", "x".repeat(20_000))?;
                Ok::<_, Error>("ok")
            })
            .get("/get", |session: Session| async move {
                session.get::<String>("v").unwrap_or_else(|| "none".into())
            })
            .get(
                "/login/{id}",
                |State(db): State<Db>, session: Session, Path(id): Path<i64>| async move {
                    let user = User::find_or_404(&db, id).await?;
                    renox::auth::login(&session, &user, None)?;
                    Ok::<_, Error>("in")
                },
            )
            .get("/logout", |session: Session| async move {
                session.flush();
                "out"
            })
            .get("/whoami", |user: Option<AuthUser>| async move {
                user.map_or_else(|| "guest".to_owned(), |u| u.name.clone())
            })
    }
}

fn app() -> App {
    App::new().module(Auth::new()).module(Probe)
}

async fn rows(app: &TestApp) -> i64 {
    renox::db::sql("SELECT COUNT(*) FROM sessions")
        .scalar(app.db())
        .await
        .unwrap()
}

#[renox::test]
async fn the_database_holds_the_session_and_the_cookie_only_its_id() {
    // Outside the testing environment, so the real table is used.
    let app = TestApp::with_config(app(), |c| {
        c.session_driver = "database".into();
        c.env = renox::Environment::Local;
    })
    .await;
    let user = User::register(app.db(), "Ana", "ana@example.com", "password123")
        .await
        .unwrap();

    app.get("/put/coffee").await.assert_see("ok");
    let cookie = app.session_cookie().unwrap();
    assert!(cookie.len() < 200, "only an id: {cookie}");
    assert_eq!(rows(&app).await, 1);
    app.get("/get").await.assert_see("coffee");
    // No 4 KB limit.
    app.get("/big").await.assert_ok();
    assert!(app.session_cookie().unwrap().len() < 200);
    app.get("/get").await.assert_see("coffee");
    // The table has the id's hash, not the id.
    let stored: String = renox::db::sql("SELECT id FROM sessions")
        .scalar(app.db())
        .await
        .unwrap();
    assert!(!cookie.contains(&stored));

    // Logging in gives the session a new id and deletes the old row, so a
    // copy of the old cookie is worthless.
    let before = app.session_cookie();
    app.get(&format!("/login/{}", user.id))
        .await
        .assert_see("in");
    let after = app.session_cookie();
    assert_ne!(before, after);
    app.get("/whoami").await.assert_see("Ana");
    app.get("/get").await.assert_see("coffee"); // the data came along
    let owner: Option<i64> = renox::db::sql("SELECT user_id FROM sessions")
        .scalar(app.db())
        .await
        .unwrap();
    assert_eq!(owner, Some(user.id));
    assert_eq!(rows(&app).await, 1);
    app.use_session_cookie(before);
    app.get("/whoami").await.assert_see("guest");
    app.get("/get").await.assert_see("none");

    // Logging out does the same.
    app.use_session_cookie(after.clone());
    app.get("/logout").await.assert_see("out");
    app.use_session_cookie(after);
    app.get("/whoami").await.assert_see("guest");
}

#[renox::test]
async fn cookie_sessions_carry_over_and_expired_rows_are_pruned() {
    let app = TestApp::with_config(app(), |c| {
        c.session_driver = "database".into();
        c.env = renox::Environment::Local;
    })
    .await;
    let user = User::register(app.db(), "Ben", "ben@example.com", "password123")
        .await
        .unwrap();
    // A whole-session cookie (as the cookie driver wrote it) still works:
    // switching SESSION_DRIVER logs nobody out.
    app.acting_as(&user);
    app.get("/whoami").await.assert_see("Ben");
    assert!(app.session_cookie().unwrap().len() < 200, "now an id");
    app.get("/whoami").await.assert_see("Ben");

    renox::db::sql(
        "INSERT INTO sessions (id, user_id, payload, expires_at, last_activity) VALUES ('old', NULL, '{}', 1, 1)",
    )
    .execute(app.db())
    .await
    .unwrap();
    // A request's background prune (1 in 50) may have beaten us to it.
    let pruned = Session::prune_expired(app.db()).await.unwrap();
    assert!(pruned <= 1, "{pruned}");
    assert_eq!(rows(&app).await, 1, "only the live session is left");
    app.get("/whoami").await.assert_see("Ben");
}

#[renox::test]
async fn test_helpers_work_with_database_sessions() {
    let app = TestApp::with_config(app(), |c| c.session_driver = "database".into()).await;
    let user = User::register(app.db(), "Cindy", "cindy@example.com", "password123")
        .await
        .unwrap();
    app.acting_as(&user);
    app.get("/put/tea").await.assert_ok();
    assert_eq!(app.session_get::<String>("v").as_deref(), Some("tea"));
    app.assert_authenticated(Some(&user))
        .assert_session_has("v");
    app.get("/whoami").await.assert_see("Cindy");
    // Forms still get their CSRF token from the session.
    app.post("/logout", &[]).await.assert_redirect("/");
    app.assert_guest();
}

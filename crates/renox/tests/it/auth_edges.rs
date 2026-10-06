//! #253: auth paths no test had reached: the `AuthUser` methods, the auth
//! middleware when its lookups fail, guards for guests, the session of an
//! older login, registration fields, and the helpers for other login ways.

use renox::auth::permissions::{self, Permissions, Scope};
use renox::auth::{DatabaseMessage, complete_login, pending_login};
use renox::prelude::*;
use renox::testing::TestApp;

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/home", || async { "home" })
            .get("/me", |user: AuthUser| async move {
                format!(
                    "{}|{:?}|{}|{}",
                    user.has_permission("orders.view"),
                    user.token_id(),
                    user.has_role_in("manager", &Scope::new("stores", 7)),
                    user.has_role_in("manager", &Scope::new("stores", 8)),
                )
            })
            .get("/who", |user: Option<AuthUser>| async move {
                user.map_or_else(|| "guest".to_owned(), |u| u.name.clone())
            })
            .get("/beta", |user: AuthUser| async move {
                Ok::<_, Error>(format!(
                    "{}|{}",
                    user.allows_async("beta").await?,
                    user.allows_async("no-such-gate").await?
                ))
            })
            .get("/forget-session-id", |session: Session| async move {
                // As a login from before sessions had ids.
                session.remove("_auth_session_id");
                "ok"
            })
            .post(
                "/logout-now",
                |State(state): State<AppState>, session: Session| async move {
                    renox::auth::logout(&state.db, &session).await?;
                    Ok::<_, Error>("out")
                },
            )
            .get(
                "/pending",
                |State(state): State<AppState>, session: Session| async move {
                    let Some(pending) = pending_login(&session) else {
                        return Ok::<_, Error>("none".to_owned());
                    };
                    Ok(format!(
                        "{:?}",
                        complete_login(&state, &session, &pending, None).await?
                    ))
                },
            )
            .merge(
                Routes::new()
                    .get("/verified-only", || async { "verified" })
                    .require_verified(),
            )
    }
}

async fn app() -> TestApp {
    TestApp::new(
        App::new()
            .module(Auth::new())
            .module(Permissions)
            .module(Pages)
            .gate_async("beta", |_user, _state| async { Ok(false) }),
    )
    .await
}

async fn ann(app: &TestApp) -> User {
    User::register(app.db(), "Ann", "ann@example.com", "password123")
        .await
        .unwrap()
}

#[renox::test]
async fn auth_user_answers_for_permissions_tokens_and_scoped_roles() {
    let app = app().await;
    let db = app.db();
    permissions::define_role(db, "clerk", &["orders.view"])
        .await
        .unwrap();
    permissions::define_role(db, "manager", &[]).await.unwrap();
    let ann = ann(&app).await;
    ann.assign_role(db, "clerk").await.unwrap();
    ann.assign_role_in(db, "manager", &Scope::new("stores", 7))
        .await
        .unwrap();
    app.acting_as(&ann);
    app.get("/me").await.assert_see("true|None|true|false");
    // With a token, the request knows which one.
    let token = ann.create_token(db, "cli", None).await.unwrap();
    let res = app
        .request()
        .header("authorization", &format!("Bearer {}", token.plain))
        .get("/me")
        .await;
    let text = res.text();
    assert!(text.starts_with("true|Some("), "{text}");
}

#[renox::test]
async fn async_gates_and_unknown_gates_refuse() {
    let app = app().await;
    app.acting_as(&ann(&app).await);
    app.get("/beta").await.assert_see("false|false");
}

#[renox::test]
async fn a_guest_asking_for_json_gets_a_401() {
    let app = app().await;
    let res = app.request().json().get("/me").await;
    res.assert_unauthorized();
    assert!(res.json_path("message").is_string());
}

#[renox::test]
async fn require_verified_sends_guests_to_log_in() {
    let app = app().await;
    app.get("/verified-only").await.assert_redirect("/login");
    app.request()
        .json()
        .get("/verified-only")
        .await
        .assert_unauthorized();
}

#[renox::test]
async fn logging_out_an_older_session_ends_all_of_the_users_sessions() {
    let app = app().await;
    let ann = ann(&app).await;
    app.post(
        "/login",
        &[("email", "ann@example.com"), ("password", "password123")],
    )
    .await
    .assert_redirect("/");
    let other_device = app.session_cookie();
    app.get("/forget-session-id").await.assert_ok();
    app.post("/logout-now", &[]).await.assert_see("out");
    // The other device's session (same user) is over too.
    app.use_session_cookie(other_device);
    app.get("/who").await.assert_see("guest");
    let revoked: i64 = renox::db::sql(
        "SELECT COUNT(*) FROM users WHERE id = ? AND sessions_revoked_at IS NOT NULL",
    )
    .bind(ann.id)
    .scalar(app.db())
    .await
    .unwrap();
    assert_eq!(revoked, 1);
}

#[renox::test]
async fn a_broken_token_table_makes_a_guest_not_an_error() {
    let app = app().await;
    let ann = ann(&app).await;
    let token = ann.create_token(app.db(), "cli", None).await.unwrap();
    renox::db::sql("DROP TABLE personal_access_tokens")
        .execute(app.db())
        .await
        .unwrap();
    app.request()
        .header("authorization", &format!("Bearer {}", token.plain))
        .get("/who")
        .await
        .assert_ok()
        .assert_see("guest");
}

#[renox::test]
async fn a_broken_roles_table_leaves_the_user_without_roles() {
    let app = app().await;
    let db = app.db();
    permissions::define_role(db, "clerk", &["orders.view"])
        .await
        .unwrap();
    let ann = ann(&app).await;
    ann.assign_role(db, "clerk").await.unwrap();
    renox::db::sql("DROP TABLE role_user")
        .execute(db)
        .await
        .unwrap();
    app.acting_as(&ann);
    app.get("/me")
        .await
        .assert_ok()
        .assert_see("false|None|false|false");
}

#[renox::test]
async fn a_broken_users_table_makes_a_guest() {
    let app = app().await;
    app.acting_as(&ann(&app).await);
    app.get("/who").await.assert_see("Ann");
    renox::db::sql("ALTER TABLE users RENAME TO people")
        .execute(app.db())
        .await
        .unwrap();
    app.get("/who").await.assert_ok().assert_see("guest");
}

/// A second login step that every user must pass, for the pending-login test.
struct Step;

impl Module for Step {
    fn name(&self) -> &'static str {
        "step"
    }

    fn register(&self, app: &mut Registry) {
        app.second_factor("step.challenge", |_user, _state| async { Ok(true) });
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/step", || async { "step" })
            .name("step.challenge")
            .get("/", || async { "home" })
            .name("home")
    }
}

#[renox::test]
async fn completing_a_login_for_a_user_who_is_gone_ends_the_wait() {
    let app = TestApp::new(App::new().module(Auth::new()).module(Step).module(Pages)).await;
    app.get("/pending").await.assert_see("none");
    let ann = ann(&app).await;
    app.post(
        "/login",
        &[("email", "ann@example.com"), ("password", "password123")],
    )
    .await
    .assert_redirect("/step");
    let mut ann = ann;
    ann.delete(app.db()).await.unwrap();
    // The wait is over and nobody is logged in.
    app.get("/pending").await.assert_see("None");
    app.get("/pending").await.assert_see("none");
    app.get("/who").await.assert_see("guest");
}

#[renox::test]
async fn database_messages_have_a_kind_for_each_level() {
    for (message, kind) in [
        (DatabaseMessage::info("Synced"), "info"),
        (DatabaseMessage::warning("Low disk"), "warning"),
        (DatabaseMessage::error("Failed"), "error"),
    ] {
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["status"], kind, "{value}");
    }
}

struct Extra;

impl Module for Extra {
    fn name(&self) -> &'static str {
        "extra"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { "home" })
            .name("home")
            .post(
                "/join-by-invite",
                |State(state): State<AppState>| async move {
                    renox::auth::register_verified(&state, "Bob", "bob@example.com", &[]).await?;
                    Ok::<_, Error>("joined")
                },
            )
            .merge(
                Routes::new()
                    .post("/danger", || async { "done" })
                    .require_password_confirmed()
                    .require_auth(),
            )
    }
}

/// Registration fields sent several times (a group of checkboxes) reach
/// `registration_rules` and `on_registered` whole.
#[renox::test]
async fn registration_fields_sent_twice_are_lists() {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = seen.clone();
    let app = TestApp::new(
        App::new()
            .module(
                Auth::new()
                    .registration_rules(|reg, v| {
                        v.field("topics", &reg.all("topics")).required();
                    })
                    .on_registered(move |user, reg, _state| {
                        let kept = kept.clone();
                        async move {
                            kept.lock().unwrap().push(format!(
                                "{}:{}:{}",
                                user.name,
                                reg.get("topics"),
                                reg.all("topics").join("+")
                            ));
                            Ok(())
                        }
                    }),
            )
            .module(Extra),
    )
    .await;
    let form = [
        ("name", "Cara"),
        ("email", "cara@example.com"),
        ("password", "a long password 12"),
        ("password_confirmation", "a long password 12"),
    ];
    let mut with_topics = form.to_vec();
    with_topics.extend([("topics", "news"), ("topics", "deals")]);
    app.post("/register", &with_topics)
        .await
        .assert_redirect("/");
    assert_eq!(*seen.lock().unwrap(), ["Cara:news:news+deals"]);
    app.logout();
    let mut without = form.to_vec();
    without[1] = ("email", "dan@example.com");
    app.htmx()
        .post("/register", &without)
        .await
        .assert_invalid("topics");
}

#[renox::test]
async fn register_verified_is_refused_when_registration_is_closed() {
    let app = TestApp::new(
        App::new()
            .module(Auth::new().without_registration())
            .module(Extra),
    )
    .await;
    app.post("/join-by-invite", &[]).await.assert_forbidden();
    app.assert_database_missing("users", &[("email", &"bob@example.com")])
        .await;
}

#[renox::test]
async fn an_htmx_request_is_sent_to_confirm_the_password_with_hx_redirect() {
    let app = TestApp::new(App::new().module(Auth::new()).module(Extra)).await;
    app.acting_as(&ann(&app).await);
    app.htmx()
        .post("/danger", &[])
        .await
        .assert_hx_redirect("/confirm-password");
    // Once confirmed, it goes through.
    app.confirm_password();
    app.htmx().post("/danger", &[]).await.assert_see("done");
}

/// A notification that only names itself: mail by default, and no mail
/// version, so sending it says what's missing.
struct Bare;

impl renox::auth::Notification for Bare {
    fn kind(&self) -> &'static str {
        "bare"
    }
}

/// Database only, with the default `null` data; and a channel with no
/// version for it.
struct Quiet;

impl renox::auth::Notification for Quiet {
    fn kind(&self) -> &'static str {
        "quiet"
    }

    fn channels(&self, _to: &renox::auth::Recipient) -> Vec<renox::auth::Channel> {
        vec![
            renox::auth::Channel::Database,
            renox::auth::Channel::Custom("sms"),
        ]
    }
}

#[renox::test]
async fn notifications_use_the_trait_defaults() {
    let app = TestApp::new(
        App::new()
            .module(Auth::new())
            .channel("sms", |_to, _message, _state| async { Ok(()) }),
    )
    .await;
    let ann = ann(&app).await;
    let err = app.state().notify(&ann, &Bare).await.unwrap_err();
    assert!(
        format!("{err:?}").contains("notification `bare` has no mail version"),
        "{err:?}"
    );
    let err = app.state().notify(&ann, &Quiet).await.unwrap_err();
    assert!(
        format!("{err:?}").contains("has no version for the `sms` channel"),
        "{err:?}"
    );
    // The database row was written first, with `null` data.
    let data: Option<String> = renox::db::sql("SELECT data FROM notifications WHERE kind = ?")
        .bind("quiet")
        .scalar_optional(app.db())
        .await
        .unwrap();
    assert_eq!(data.as_deref(), Some("null"));
}

#[renox::test]
async fn notify_later_is_recorded_by_the_fake() {
    let app = TestApp::new(App::new().module(Auth::new())).await;
    app.fake_notifications();
    let ann = ann(&app).await;
    app.state().notify_later(&ann, &Bare).await.unwrap();
    app.assert_notified(&ann, "bare");
    assert!(app.queued_jobs().await.is_empty());
}

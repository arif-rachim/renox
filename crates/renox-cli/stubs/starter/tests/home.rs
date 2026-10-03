use {{crate_name}}::roles::{self, ADMIN, MEMBER};
use renox::prelude::*;
use renox::testing::TestApp;
use std::time::Duration;

/// A verified user with `role`.
async fn person(app: &TestApp, email: &str, role: &str) -> User {
    let mut user = User::register(app.db(), "Anna", email, "secret123")
        .await
        .unwrap();
    user.email_verified_at = Some(renox::db::now());
    user.save(app.db()).await.unwrap();
    roles::define(app.db()).await.unwrap();
    user.assign_role(app.db(), role).await.unwrap();
    user
}

#[renox::test]
async fn the_home_page_works() {
    let app = TestApp::new({{crate_name}}::app()).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_view("home/index.html")
        .assert_see("Create an account");
}

#[renox::test]
async fn sign_ups_are_members_who_verify_their_email_first() {
    let app = TestApp::new({{crate_name}}::app()).await;
    app.post(
        "/register",
        &[
            ("name", "Anna"),
            ("email", "anna@example.com"),
            ("password", "secret123"),
            ("password_confirmation", "secret123"),
        ],
    )
    .await
    .assert_redirect("/dashboard");
    let anna = User::find_by_email(app.db(), "anna@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(anna.roles(app.db()).await.unwrap(), ["member"]);
    // The verification link went by mail; until it's used, the dashboard waits.
    app.assert_mail_sent("anna@example.com", "Verify your email");
    app.get("/dashboard").await.assert_redirect("/verify-email");
}

#[renox::test]
async fn the_dashboard_is_for_verified_users() {
    let app = TestApp::new({{crate_name}}::app()).await;
    app.get("/dashboard").await.assert_redirect("/login");
    let anna = person(&app, "anna@example.com", MEMBER).await;
    app.acting_as(&anna);
    app.get("/dashboard")
        .await
        .assert_ok()
        .assert_see("Welcome back, Anna.");
}

#[renox::test]
async fn only_admins_manage_users_and_read_the_activity_log() {
    let app = TestApp::new({{crate_name}}::app()).await;
    let member = person(&app, "member@example.com", MEMBER).await;
    app.acting_as(&member);
    app.get("/users").await.assert_forbidden();
    app.get("/activity").await.assert_forbidden();

    let admin = person(&app, "admin@example.com", ADMIN).await;
    app.acting_as(&admin);
    app.get("/users")
        .await
        .assert_ok()
        .assert_see("member@example.com");
    app.htmx()
        .put(
            &format!("/users/{}/roles", member.id),
            &[("roles", "admin"), ("roles", "member")],
        )
        .await
        .assert_ok();
    assert_eq!(member.roles(app.db()).await.unwrap(), ["admin", "member"]);
    // Recorded, and shown on the activity page.
    app.get("/activity")
        .await
        .assert_ok()
        .assert_see("user.roles_changed");
}

#[renox::test]
async fn the_first_admin_is_made_with_a_command() {
    let app = TestApp::new({{crate_name}}::app()).await;
    let anna = User::register(app.db(), "Anna", "anna@example.com", "secret123")
        .await
        .unwrap();
    app.kernel()
        .call("users:admin", ["anna@example.com"])
        .await
        .unwrap();
    assert_eq!(anna.roles(app.db()).await.unwrap(), [ADMIN]);
    assert!(
        app.kernel()
            .call("users:admin", ["nobody@example.com"])
            .await
            .is_err()
    );
}

#[renox::test]
async fn admins_keep_their_own_admin_role() {
    let app = TestApp::new({{crate_name}}::app()).await;
    let admin = person(&app, "admin@example.com", ADMIN).await;
    app.acting_as(&admin);
    app.htmx()
        .put(
            &format!("/users/{}/roles", admin.id),
            &[("roles", "member")],
        )
        .await
        .assert_invalid("roles");
}

#[renox::test]
async fn sessions_end_after_their_lifetime() {
    let app = TestApp::new({{crate_name}}::app()).await;
    let anna = person(&app, "anna@example.com", MEMBER).await;
    app.acting_as(&anna);
    app.get("/account").await.assert_ok();
    // Time moves for the app (renox::db::now(), sessions, the queue), not for the test.
    app.travel(Duration::from_secs(3 * 60 * 60)); // past SESSION_LIFETIME's 120 minutes
    app.get("/account").await.assert_redirect("/login");
}

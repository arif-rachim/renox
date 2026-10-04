//! Turning two-factor authentication on and off from the account page, the
//! login challenge and recovery codes (#170–#173).

use std::time::Duration;

use renox::audit::Audit;
use renox::prelude::*;
use renox::testing::TestApp;
use renox_2fa::{RecoveryCodeUsed, TwoFactor, TwoFactorCredential, TwoFactorEnabled, totp};

const PASSWORD: &str = "a long password 12";

async fn app() -> (TestApp, User) {
    let app = TestApp::new(
        App::new()
            .module(Auth::new().account())
            .module(Audit)
            .module(TwoFactor::new()),
    )
    .await;
    let ana = User::register(app.db(), "Ana", "ana@example.com", PASSWORD)
        .await
        .unwrap();
    (app, ana)
}

/// The secret of `user`'s (unconfirmed or confirmed) setup.
async fn secret(app: &TestApp, user: &User) -> String {
    TwoFactorCredential::of(app.db(), user.id)
        .await
        .unwrap()
        .unwrap()
        .secret
        .to_string()
}

/// The code an authenticator app would show `seconds` from now.
fn code(secret: &str, seconds: i64) -> String {
    let step = totp::step_at(renox::db::now().timestamp() + seconds);
    totp::code_at(secret, step).unwrap()
}

/// Turns it on for the logged-in `user`; returns the secret and the
/// recovery codes the page showed.
async fn turn_on(app: &TestApp, user: &User) -> (String, Vec<String>) {
    app.acting_as(user);
    app.confirm_password();
    app.post("/two-factor/enable", &[])
        .await
        .assert_redirect("/two-factor/setup");
    let secret = secret(app, user).await;
    app.post(
        "/two-factor/confirm",
        &[("code", code(&secret, 0).as_str())],
    )
    .await
    .assert_redirect("/two-factor/recovery-codes");
    let page = app
        .get("/two-factor/recovery-codes")
        .await
        .assert_ok()
        .text();
    let codes: Vec<String> = page
        .split("<pre class=\"rx-2fa__codes\">")
        .nth(1)
        .and_then(|rest| rest.split("</pre>").next())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(codes.len(), 8, "{page}");
    (secret, codes)
}

/// Logs out, then types the right password: the login waits for the code.
async fn password(app: &TestApp, user: &User) {
    app.post("/logout", &[]).await;
    app.post(
        "/login",
        &[("email", user.email.as_str()), ("password", PASSWORD)],
    )
    .await
    .assert_redirect("/two-factor/challenge");
}

#[renox::test]
async fn the_account_page_offers_it_and_turning_it_on_needs_the_password() {
    let (app, ana) = app().await;
    app.acting_as(&ana);
    app.get("/account")
        .await
        .assert_see("Two-factor authentication")
        .assert_see("action=\"/two-factor/enable\"");
    // Without a fresh password confirmation, it asks for the password first.
    app.post("/two-factor/enable", &[])
        .await
        .assert_redirect("/confirm-password");
    assert!(
        TwoFactorCredential::of(app.db(), ana.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[renox::test]
async fn enrolling_shows_a_qr_code_and_needs_a_right_code() {
    let (app, ana) = app().await;
    app.fake_events();
    app.acting_as(&ana);
    app.confirm_password();
    app.post("/two-factor/enable", &[])
        .await
        .assert_redirect("/two-factor/setup");
    let secret = secret(&app, &ana).await;
    let page = app.get("/two-factor/setup").await.assert_ok().text();
    assert!(page.contains("<svg"), "a QR code");
    assert!(page.contains(&secret[..4]), "the key to type in");
    // Not on until a code is confirmed: logging in doesn't ask yet.
    assert!(
        !TwoFactorCredential::enabled(app.db(), ana.id)
            .await
            .unwrap()
    );

    app.htmx()
        .post("/two-factor/confirm", &[("code", "000000")])
        .await
        .assert_invalid("code");
    assert!(
        !TwoFactorCredential::enabled(app.db(), ana.id)
            .await
            .unwrap()
    );

    app.post(
        "/two-factor/confirm",
        &[("code", code(&secret, 0).as_str())],
    )
    .await
    .assert_redirect("/two-factor/recovery-codes");
    assert!(
        TwoFactorCredential::enabled(app.db(), ana.id)
            .await
            .unwrap()
    );
    assert_eq!(app.emitted::<TwoFactorEnabled>().len(), 1);
    // The secret isn't shown again, and the codes only once.
    app.get("/two-factor/recovery-codes").await.assert_ok();
    app.get("/two-factor/recovery-codes")
        .await
        .assert_redirect("/account");
    app.get("/two-factor/setup")
        .await
        .assert_redirect("/account");
    // Enrolling again while it's on changes nothing.
    app.post("/two-factor/enable", &[])
        .await
        .assert_redirect("/account");
    assert_eq!(secret, self::secret(&app, &ana).await);
    app.get("/account")
        .await
        .assert_see("On since")
        .assert_see("8 recovery codes left");
}

#[renox::test]
async fn the_secret_is_stored_sealed_and_codes_hashed() {
    let (app, ana) = app().await;
    let (secret, codes) = turn_on(&app, &ana).await;
    let (stored, hashes): (String, String) =
        renox::db::sql("SELECT secret, recovery_codes FROM two_factor WHERE user_id = ?")
            .bind(ana.id)
            .fetch_one_as(app.db())
            .await
            .unwrap();
    assert!(!stored.contains(&secret));
    for code in &codes {
        assert!(!hashes.contains(code.as_str()));
    }
}

#[renox::test]
async fn logging_in_asks_for_the_code_after_the_password() {
    let (app, ana) = app().await;
    let (secret, _) = turn_on(&app, &ana).await;
    password(&app, &ana).await;
    // Not logged in yet.
    app.get("/account").await.assert_redirect("/login");
    app.get("/two-factor/challenge")
        .await
        .assert_ok()
        .assert_see("authenticator app");

    // Wait for the next step, so the code isn't the one used to confirm.
    app.travel(Duration::from_secs(30));
    let next = app.at_travelled_time(async { code(&secret, 0) }).await;
    // Where the login would have gone: `/` here (no `redirect_to`).
    app.post("/two-factor/challenge", &[("code", next.as_str())])
        .await
        .assert_redirect("/");
    app.get("/account").await.assert_ok();

    // The same code can't be used twice.
    password(&app, &ana).await;
    app.htmx()
        .post("/two-factor/challenge", &[("code", next.as_str())])
        .await
        .assert_invalid("code");
}

#[renox::test]
async fn codes_from_the_steps_next_to_now_work_but_older_ones_dont() {
    let (app, ana) = app().await;
    let (secret, _) = turn_on(&app, &ana).await;
    // Two minutes later: four steps on.
    app.travel(Duration::from_secs(120));
    password(&app, &ana).await;
    let too_old = app.at_travelled_time(async { code(&secret, -90) }).await;
    app.htmx()
        .post("/two-factor/challenge", &[("code", too_old.as_str())])
        .await
        .assert_invalid("code");
    // A phone whose clock is a step behind.
    let behind = app.at_travelled_time(async { code(&secret, -30) }).await;
    app.post("/two-factor/challenge", &[("code", behind.as_str())])
        .await
        .assert_redirect("/");
}

#[renox::test]
async fn wrong_codes_count_towards_the_login_throttle() {
    let (app, ana) = app().await;
    let (secret, _) = turn_on(&app, &ana).await;
    password(&app, &ana).await;
    for _ in 0..5 {
        app.htmx()
            .post("/two-factor/challenge", &[("code", "000000")])
            .await
            .assert_invalid("code");
    }
    app.post(
        "/two-factor/challenge",
        &[("code", code(&secret, 30).as_str())],
    )
    .await
    .assert_status(429);
    app.assert_database_has("audit_logs", &[("action", &"auth.login_failed")])
        .await;
}

#[renox::test]
async fn a_waiting_login_expires() {
    let (app, ana) = app().await;
    turn_on(&app, &ana).await;
    password(&app, &ana).await;
    app.travel(Duration::from_secs(11 * 60));
    app.get("/two-factor/challenge")
        .await
        .assert_redirect("/login");
}

#[renox::test]
async fn recovery_codes_work_once_each() {
    let (app, ana) = app().await;
    let (_, codes) = turn_on(&app, &ana).await;
    password(&app, &ana).await;
    // Typed loosely: upper case, spaces.
    let typed = codes[0].to_uppercase().replace('-', " ");
    app.post("/two-factor/challenge", &[("code", typed.as_str())])
        .await
        .assert_redirect("/");
    app.get("/account")
        .await
        .assert_see("7 recovery codes left");
    app.assert_database_has(
        "audit_logs",
        &[("action", &"two_factor.recovery_code_used")],
    )
    .await;

    password(&app, &ana).await;
    app.htmx()
        .post("/two-factor/challenge", &[("code", codes[0].as_str())])
        .await
        .assert_invalid("code");
    app.post("/two-factor/challenge", &[("code", codes[1].as_str())])
        .await
        .assert_redirect("/");
}

#[renox::test]
async fn new_recovery_codes_replace_the_old_ones() {
    let (app, ana) = app().await;
    let (_, old) = turn_on(&app, &ana).await;
    app.fake_events();
    app.post("/two-factor/recovery-codes", &[])
        .await
        .assert_redirect("/two-factor/recovery-codes");
    let page = app.get("/two-factor/recovery-codes").await.text();
    assert!(!page.contains(&old[0]));
    password(&app, &ana).await;
    app.htmx()
        .post("/two-factor/challenge", &[("code", old[0].as_str())])
        .await
        .assert_invalid("code");
    assert!(app.emitted::<RecoveryCodeUsed>().is_empty());
}

#[renox::test]
async fn turning_it_off_needs_the_password_and_ends_the_challenge() {
    let (app, ana) = app().await;
    turn_on(&app, &ana).await;
    // The confirmation runs out after three hours (the session, two hours
    // without a visit, is kept alive halfway).
    app.travel(Duration::from_secs(100 * 60));
    app.get("/account").await.assert_ok();
    app.travel(Duration::from_secs(100 * 60));
    app.delete("/two-factor")
        .await
        .assert_redirect("/confirm-password");
    assert!(
        TwoFactorCredential::enabled(app.db(), ana.id)
            .await
            .unwrap()
    );

    app.confirm_password();
    app.delete("/two-factor").await.assert_redirect("/account");
    assert!(
        TwoFactorCredential::of(app.db(), ana.id)
            .await
            .unwrap()
            .is_none()
    );
    app.assert_database_has("audit_logs", &[("action", &"two_factor.disabled")])
        .await;
    app.assert_database_has("audit_logs", &[("action", &"two_factor.enabled")])
        .await;

    // Logging in is the password alone again.
    app.post("/logout", &[]).await;
    app.post(
        "/login",
        &[("email", ana.email.as_str()), ("password", PASSWORD)],
    )
    .await
    .assert_redirect("/");
}

#[renox::test]
async fn it_works_without_the_audit_module() {
    let app = TestApp::new(
        App::new()
            .module(Auth::new().account())
            .module(TwoFactor::new()),
    )
    .await;
    let ana = User::register(app.db(), "Ana", "ana@example.com", PASSWORD)
        .await
        .unwrap();
    turn_on(&app, &ana).await;
    assert!(
        TwoFactorCredential::enabled(app.db(), ana.id)
            .await
            .unwrap()
    );
}

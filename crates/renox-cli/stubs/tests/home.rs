use renox::testing::TestApp;

#[renox::test]
async fn the_home_page_works() {
    let app = TestApp::new({{crate_name}}::app()).await;
    app.get("/").await.assert_ok().assert_see("<h1>");
}

#[renox::test]
async fn guests_can_register() {
    let app = TestApp::new({{crate_name}}::app()).await;
    app.post(
        "/register",
        &[
            ("name", "Arif"),
            ("email", "arif@example.com"),
            ("password", "rahasia123"),
            ("password_confirmation", "rahasia123"),
        ],
    )
    .await
    .assert_redirect("/");
    app.assert_database_has("users", &[("email", &"arif@example.com")]).await;
    // Registered and logged in: the account page is theirs.
    app.get("/account").await.assert_ok().assert_see("arif@example.com");
}

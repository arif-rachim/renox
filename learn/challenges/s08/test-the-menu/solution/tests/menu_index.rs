use renox::testing::TestApp;

#[renox::test]
async fn an_empty_menu_says_so() {
    // Nothing is seeded, so there is nothing to list.
    let app = TestApp::new(coffee::app()).await;
    app.get("/menu")
        .await
        .assert_ok()
        .assert_see("Nothing on the menu yet");
}

#[renox::test]
async fn items_come_in_order_within_a_category() {
    let app = TestApp::new(coffee::app()).await;
    app.kernel().seed().await.unwrap();
    let response = app.get("/menu").await;
    response.assert_ok();

    let page = response.text();
    let cappuccino = page.find("Cappuccino").expect("Cappuccino is listed");
    let flat_white = page.find("Flat white").expect("Flat white is listed");
    assert!(cappuccino < flat_white, "coffees are listed by name");
}

use renox::prelude::*;
use renox::testing::TestApp;

use coffee::app::home::drink_builder::DrinkBuilder;

#[renox::test]
async fn the_home_page_mounts_the_builder() {
    let app = TestApp::new(coffee::app()).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("data-rx-live=\"drink-builder\"")
        .assert_see("A small drink: $3.00");
}

#[renox::test]
async fn choosing_a_size_changes_the_price() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder
        .call_with("size", json!(["large"]))
        .await
        .assert_ok()
        .assert_see("$4.50");
    assert_eq!(builder.component().size, "large");
}

#[renox::test]
async fn an_unknown_size_is_a_400() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder
        .call_with("size", json!(["huge"]))
        .await
        .assert_status(400);
    assert_eq!(builder.component().size, "small");
}

#[renox::test]
async fn shots_stay_between_one_and_four() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder.call("remove_shot").await.assert_ok();
    assert_eq!(builder.component().shots, 1);
    for _ in 0..5 {
        builder.call("add_shot").await.assert_ok();
    }
    assert_eq!(builder.component().shots, 4);
    // Four shots in a small cup: $3.00 and three extra shots at 75 cents.
    builder.call("_refresh").await.assert_see("$5.25");
}

#[renox::test]
async fn the_name_follows_the_input() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder.set("name", "Ana");
    builder
        .call("_refresh")
        .await
        .assert_ok()
        .assert_see("Ready for Ana");
    assert_eq!(builder.component().name, "Ana");
}

#[renox::test]
async fn ordering_needs_a_name() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder.call("order").await.assert_invalid("name");
}

#[renox::test]
async fn ordering_with_a_name_shows_a_toast() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder.set("name", "Ana");
    builder.call("order").await.assert_ok();
}

#[renox::test]
async fn unknown_actions_are_404() {
    let app = TestApp::new(coffee::app()).await;
    let mut builder = app.live(DrinkBuilder::default());
    builder.call("nope").await.assert_status(404);
}

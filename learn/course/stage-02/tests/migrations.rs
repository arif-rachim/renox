use renox::db::sql;
use renox::testing::TestApp;

#[renox::test]
async fn migrations_create_the_menu_tables() {
    let app = TestApp::new(coffee::app()).await;
    for table in ["categories", "menu_items"] {
        let count = sql(format!("SELECT COUNT(*) FROM {table}"))
            .scalar::<i64>(app.db())
            .await;
        assert_eq!(count.unwrap(), 0, "{table}");
    }
}

#[renox::test]
async fn migrations_roll_back_and_run_again() {
    let app = TestApp::new(coffee::app()).await;
    let count = || sql("SELECT COUNT(*) FROM menu_items").scalar::<i64>(app.db());

    // TestApp ran every migration in one batch: undo it.
    app.kernel().rollback(1).await.unwrap();
    assert!(count().await.is_err(), "the table is gone");

    app.kernel().migrate().await.unwrap();
    assert_eq!(count().await.unwrap(), 0);
}

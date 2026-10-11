use renox::db::sql;
use renox::prelude::*;
use renox::testing::TestApp;

#[renox::test]
async fn the_allergens_column_is_nullable() {
    let app = TestApp::new(coffee::app()).await;
    let db = app.db();
    let user = User::register(db, "Bea", "bea@example.com", "password123")
        .await
        .unwrap();
    sql("INSERT INTO categories (name) VALUES ('Pastry')")
        .execute(db)
        .await
        .unwrap();
    // Without a note the row is valid: the column is nullable.
    for name in ["Espresso", "Croissant"] {
        sql("INSERT INTO menu_items (category_id, user_id, name, price) VALUES (1, ?, ?, 375)")
            .bind(user.id)
            .bind(name)
            .execute(db)
            .await
            .unwrap();
    }
    sql("UPDATE menu_items SET allergens = ? WHERE name = 'Croissant'")
        .bind("milk, gluten")
        .execute(db)
        .await
        .unwrap();
    let notes = sql("SELECT COALESCE(allergens, '') FROM menu_items ORDER BY name")
        .scalars::<String>(db)
        .await
        .unwrap();
    assert_eq!(notes, ["milk, gluten", ""]);
}

#[renox::test]
async fn the_model_has_the_field() {
    // Fails when the model and the tables disagree: a field without a column, or the other way round.
    TestApp::new(coffee::app())
        .await
        .assert_models_match_schema()
        .await;
}

#[renox::test]
async fn the_migration_rolls_back_and_runs_again() {
    let app = TestApp::new(coffee::app()).await;
    let count = || sql("SELECT COUNT(allergens) FROM menu_items").scalar::<i64>(app.db());
    assert_eq!(count().await.unwrap(), 0);

    // TestApp ran every migration in one batch: undo it, then run it again.
    app.kernel().rollback(1).await.unwrap();
    assert!(count().await.is_err(), "the table is gone");
    app.kernel().migrate().await.unwrap();
    assert_eq!(count().await.unwrap(), 0);
}

#[test]
fn the_down_file_removes_the_column() {
    let down: Vec<String> = std::fs::read_dir("migrations")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".down.sql"))
        .map(|path| std::fs::read_to_string(path).unwrap())
        .filter(|sql| sql.contains("allergens"))
        .collect();
    assert_eq!(down.len(), 1, "one .down.sql file should mention allergens");
    assert!(down[0].to_uppercase().contains("DROP COLUMN"));
}

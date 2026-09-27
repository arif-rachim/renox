//! Runs on SQLite, or on PostgreSQL with TEST_DATABASE_URL set.

use postgres_app::Task;
use renox::chrono::NaiveDate;
use renox::prelude::*;
use renox::testing::TestApp;

async fn app() -> TestApp {
    TestApp::new(postgres_app::app()).await
}

#[renox::test]
async fn tasks_keep_their_types_on_either_database() {
    let app = app().await;
    app.post(
        "/tasks",
        &[("title", "Beli kopi"), ("due_on", "2026-10-01")],
    )
    .await
    .assert_redirect("/");
    let task = Task::where_eq("title", "Beli kopi")
        .first(app.db())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.due_on, NaiveDate::from_ymd_opt(2026, 10, 1));
    assert!(!task.done);

    app.post(&format!("/tasks/{}", task.id), &[("_method", "PATCH")])
        .await
        .assert_redirect("/");
    assert!(Task::find(app.db(), task.id).await.unwrap().unwrap().done);
}

#[renox::test]
async fn search_ignores_case_and_overdue_uses_dates() {
    let app = app().await;
    let yesterday = renox::db::now().date_naive().pred_opt().unwrap();
    for (title, due_on) in [("Beli KOPI", Some(yesterday)), ("Bayar listrik", None)] {
        Task::create(
            app.db(),
            Task {
                title: title.into(),
                due_on,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    app.get("/?q=kopi")
        .await
        .assert_see("Beli KOPI")
        .assert_dont_see("Bayar listrik");
    app.get("/?overdue=true")
        .await
        .assert_see("Beli KOPI")
        .assert_dont_see("Bayar listrik");
}

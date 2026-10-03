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

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(postgres_app::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Task::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Task::query().count(app.db()).await.unwrap(), seeded);
}

#[renox::test]
async fn a_blank_title_comes_back_with_the_date_kept() {
    let app = app().await;
    app.post("/tasks", &[("title", " "), ("due_on", "2026-10-01")])
        .await
        .assert_redirect("/");
    app.get("/")
        .await
        .assert_see(r#"value="2026-10-01""#)
        .assert_see("The title field is required.");
    assert_eq!(Task::query().count(app.db()).await.unwrap(), 0);
}

#[renox::test]
async fn overdue_means_before_today_and_not_done() {
    let app = app().await;
    let today = renox::db::now().date_naive();
    let yesterday = today.pred_opt().unwrap();
    for (title, due_on, done) in [
        ("Due today", Some(today), false),
        ("Late", Some(yesterday), false),
        ("Late but done", Some(yesterday), true),
        ("Someday", None, false),
    ] {
        let task = Task {
            title: title.into(),
            due_on,
            done,
            ..Default::default()
        };
        Task::create(app.db(), task).await.unwrap();
    }
    app.get("/?overdue=true")
        .await
        .assert_see("</button> Late")
        .assert_dont_see("Due today")
        .assert_dont_see("Late but done")
        .assert_dont_see("Someday");
    // Tasks without a date come last on both databases.
    let page = app.get("/").await.text();
    let (late, someday) = (page.find("Late").unwrap(), page.find("Someday").unwrap());
    assert!(late < page.find("Due today").unwrap() && page.find("Due today").unwrap() < someday);
}

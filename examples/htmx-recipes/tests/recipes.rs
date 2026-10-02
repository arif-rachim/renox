//! Each recipe answers htmx with the smallest fragment and plain requests
//! with a redirect.

use htmx_recipes::app::tasks::Task;
use renox::prelude::*;
use renox::testing::TestApp;

async fn with_tasks(n: usize) -> TestApp {
    let app = TestApp::new(htmx_recipes::app()).await;
    for i in 1..=n {
        Task::create(
            app.db(),
            Task {
                title: format!("Task {i}"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    app
}

#[renox::test]
async fn the_page_loads_more_rows_as_you_scroll() {
    let app = with_tasks(20).await;
    app.get("/")
        .await
        .assert_ok()
        .assert_see("<html")
        .assert_see(">Task 20<") // newest first
        .assert_dont_see(">Task 5<")
        .assert_see(r#"hx-get="/?before=6" hx-trigger="revealed""#)
        .assert_see("20 open");

    // A task added meanwhile doesn't shift the next batch (it goes by id,
    // not page number), so no row repeats.
    app.htmx()
        .post("/tasks", &[("title", "Late arrival")])
        .await;
    // The loader asks with htmx and gets only the next rows, and no loader
    // after the last ones.
    let more = app.htmx().get("/?before=6").await;
    more.assert_ok()
        .assert_dont_see(">Task 6<")
        .assert_see(">Task 5<")
        .assert_see(">Task 1<")
        .assert_dont_see("<html")
        .assert_dont_see("before=");
}

#[renox::test]
async fn plain_forms_skip_duplicates_too_and_the_empty_note_follows_the_list() {
    let app = with_tasks(0).await;
    app.get("/")
        .await
        .assert_see(r#"<p id="empty" class="muted" hx-swap-oob="true">"#);
    // The first task hides the note (out of band); deleting it shows it again.
    app.htmx()
        .post("/tasks", &[("title", "Buy coffee")])
        .await
        .assert_see(r#"<p id="empty" class="muted" hidden hx-swap-oob="true">"#);
    // Without htmx: a redirect and a toast, still no copy.
    app.post("/tasks", &[("title", "  Buy coffee ")])
        .await
        .assert_redirect("/");
    assert_eq!(Task::query().count(app.db()).await.unwrap(), 1);
    // Edits are trimmed like new tasks.
    let task = Task::query().first(app.db()).await.unwrap().unwrap();
    app.patch(
        &format!("/tasks/{}", task.id),
        &[("title", "  Grind beans  ")],
    )
    .await;
    assert_eq!(
        Task::find_or_404(app.db(), task.id).await.unwrap().title,
        "Grind beans"
    );
    app.htmx()
        .delete(&format!("/tasks/{}", task.id))
        .await
        .assert_see(r#"<p id="empty" class="muted" hx-swap-oob="true">"#);
}

#[renox::test]
async fn the_modal_adds_a_row_and_closes() {
    let app = with_tasks(0).await;
    let res = app.htmx().post("/tasks", &[("title", "Buy coffee")]).await;
    res.assert_ok()
        .assert_header("hx-trigger", "task-added")
        .assert_see(r#"<li id="task-1""#)
        .assert_see(">Buy coffee<")
        .assert_see(r#"<small id="open-count" hx-swap-oob="true">1 open</small>"#)
        .assert_dont_see("<html");

    // The same task again: the server retargets the answer to the existing
    // row (HX-Retarget, HX-Reswap) instead of adding a copy, closes the
    // modal and says why in a toast, all in one HX-Trigger.
    let res = app
        .htmx()
        .post("/tasks", &[("title", " Buy coffee ")])
        .await;
    res.assert_ok()
        .assert_header("hx-retarget", "#task-1")
        .assert_header("hx-reswap", "outerHTML")
        .assert_see(r#"<li id="task-1""#);
    let trigger: renox::serde_json::Value =
        renox::serde_json::from_str(res.header("hx-trigger").unwrap()).unwrap();
    assert!(trigger.get("task-added").is_some(), "{trigger}");
    assert_eq!(
        trigger["renox:toast"]["toasts"][0]["message"],
        "That task is already on the list."
    );
    app.assert_database_count("tasks", 1).await;

    // Errors come back as 422 JSON, shown in the modal's form.
    app.htmx()
        .post("/tasks", &[("title", "")])
        .await
        .assert_invalid("title");
    // Without JavaScript it's a normal form post.
    app.post("/tasks", &[("title", "Buy tea")])
        .await
        .assert_redirect("/");
    app.assert_database_count("tasks", 2).await;
}

#[renox::test]
async fn titles_are_edited_in_place() {
    let app = with_tasks(1).await;
    app.htmx()
        .get("/tasks/1/edit")
        .await
        .assert_see(r#"value="Task 1""#)
        .assert_see(r#"hx-patch="/tasks/1""#)
        .assert_see(r#"hx-trigger="keyup[key=='Escape']""#);
    app.htmx()
        .patch("/tasks/1", &[("title", "Renamed")])
        .await
        .assert_ok()
        .assert_see(">Renamed<")
        .assert_see(r#"hx-trigger="dblclick""#);
    app.htmx()
        .patch("/tasks/1", &[("title", &"x".repeat(101))])
        .await
        .assert_invalid("title");
    // Escape: the row as it is.
    app.htmx().get("/tasks/1").await.assert_see(">Renamed<");
    app.htmx().get("/tasks/9/edit").await.assert_not_found();
}

#[renox::test]
async fn checkboxes_toggle_and_rows_delete_in_place() {
    let app = with_tasks(2).await;
    app.htmx()
        .patch("/tasks/1/toggle", &[])
        .await
        .assert_see(r#"class="task done""#)
        .assert_see("checked");
    assert!(Task::find_or_404(app.db(), 1).await.unwrap().done);
    app.htmx()
        .patch("/tasks/1/toggle", &[])
        .await
        .assert_dont_see("checked");

    // The row's part of the answer is empty, so htmx swaps the row for
    // nothing; the open count comes along out of band, and a toast says it.
    let res = app.htmx().delete("/tasks/1").await;
    res.assert_ok();
    assert_eq!(
        res.text(),
        r#"<small id="open-count" hx-swap-oob="true">1 open</small><p id="empty" class="muted" hidden hx-swap-oob="true">Nothing to do. Add a task.</p>"#
    );
    let trigger: renox::serde_json::Value =
        renox::serde_json::from_str(res.header("hx-trigger").unwrap()).unwrap();
    assert_eq!(
        trigger["renox:toast"]["toasts"][0]["message"],
        "“Task 1” deleted."
    );
    app.delete("/tasks/2").await.assert_redirect("/");
    app.assert_database_count("tasks", 0).await;
}

#[renox::test]
async fn bulk_actions_refresh_or_redirect() {
    let app = with_tasks(3).await;
    for id in [1, 2] {
        app.htmx().patch(&format!("/tasks/{id}/toggle"), &[]).await;
    }

    app.htmx()
        .post("/tasks/clear-done", &[])
        .await
        .assert_header("hx-refresh", "true");
    // The toast waited in the session for the reloaded page.
    app.get("/").await.assert_see("2 done tasks cleared.");
    app.assert_database_count("tasks", 1).await;

    app.htmx().patch("/tasks/3/toggle", &[]).await;
    app.htmx()
        .post("/tasks/archive", &[])
        .await
        .assert_hx_redirect("/summary");
    app.get("/summary")
        .await
        .assert_see("1 task archived.")
        .assert_see("0 tasks still open.");
    app.post("/tasks/archive", &[])
        .await
        .assert_redirect("/summary");
}

//! Each handler answers an htmx request with the smallest piece of HTML that
//! changes (a row, some rows, nothing) and a plain request with a redirect,
//! so the page works without JavaScript too.
//!
//! Made with `rnx make:module tasks`, `rnx make:model Task --module tasks -m`
//! and `rnx make:factory Task --module tasks`, then filled in.

use renox::fake::Fake;
use renox::fake::faker::lorem::en::Sentence;
use renox::prelude::*;
use renox::{HxReswap, HxRetarget};
use serde::{Deserialize, Serialize};

/// Rows per page (and per infinite-scroll load).
const PER_PAGE: u32 = 15;

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "tasks")]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub done: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Factory for Task {
    fn definition() -> Self {
        Task {
            title: Sentence(3..6).fake(),
            done: (0..4).fake::<u8>() == 0,
            ..Default::default()
        }
    }
}

pub struct Tasks;

impl Module for Tasks {
    fn name(&self) -> &'static str {
        "tasks"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("tasks.index")
            .post("/tasks", store)
            .name("tasks.store")
            .get("/tasks/{id}", show)
            .name("tasks.show")
            .get("/tasks/{id}/edit", edit)
            .name("tasks.edit")
            .patch("/tasks/{id}", update)
            .name("tasks.update")
            .patch("/tasks/{id}/toggle", toggle)
            .name("tasks.toggle")
            .delete("/tasks/{id}", destroy)
            .name("tasks.destroy")
            .post("/tasks/clear-done", clear_done)
            .name("tasks.clear_done")
            .post("/tasks/archive", archive)
            .name("tasks.archive")
            .get("/summary", summary)
            .name("summary")
    }
}

#[derive(Deserialize)]
struct ListParams {
    /// The infinite scroll's cursor: rows older than this id.
    before: Option<i64>,
}

/// The page, or (for the infinite scroll's `?before=…`) just the next rows.
/// The scroll goes by id, not page number: a task added meanwhile at the
/// top would shift the pages and repeat a row.
async fn index(State(db): State<Db>, htmx: Htmx, Query(params): Query<ListParams>) -> Result<View> {
    let mut rows = Task::query()
        .when(params.before.is_some(), |q| {
            q.where_op("id", "<", params.before.unwrap_or_default())
        })
        .order_by_desc("id")
        .limit(u64::from(PER_PAGE) + 1)
        .get(&db)
        .await?;
    let more = rows.len() > PER_PAGE as usize;
    rows.truncate(PER_PAGE as usize);
    let next = more.then(|| rows.last().map(|t| t.id)).flatten();
    if htmx.wants_fragment() {
        return Ok(view("tasks/_rows.html", context! { rows, next }));
    }
    let open = Task::where_eq("done", false).count(&db).await?;
    let total = Task::query().count(&db).await?;
    Ok(view(
        "tasks/index.html",
        context! { rows, next, open, total },
    ))
}

#[derive(Deserialize, Serialize, Validate)]
struct TaskForm {
    #[validate(required, max = 100)]
    title: String,
}

/// From the modal: the new row (and the open count, out of band), and an
/// event the modal listens for to close. Invalid input gets a 422 whose
/// errors appear in the modal's form.
///
/// A task that's already on the list isn't added twice: the server changes
/// where the answer goes (`HX-Retarget` to that row, `HX-Reswap: outerHTML`)
/// and says so in a toast. (When that row hasn't been scrolled into the
/// page yet, htmx finds no target and swaps nothing; the toast still shows.)
async fn store(State(db): State<Db>, htmx: Htmx, Valid(form): Valid<TaskForm>) -> Result<Response> {
    let title = form.title.trim().to_owned();
    let existing = Task::where_eq("title", &title).first(&db).await?;
    if existing.is_some() && !htmx.request {
        // A plain form: back to the list, with the same toast.
        let toast = Toast::info("That task is already on the list.");
        return Ok((toast, Redirect::to("/")).into_response());
    }
    if let Some(task) = existing {
        let target = format!("#task-{}", task.id);
        return Ok((
            HxRetarget(target),
            HxReswap("outerHTML".into()),
            HxTrigger("task-added".into()),
            Toast::info("That task is already on the list."),
            answer(&db, Some(task)).await?,
        )
            .into_response());
    }
    let task = Task::create(
        &db,
        Task {
            title,
            ..Default::default()
        },
    )
    .await?;
    if htmx.request {
        return Ok((
            HxTrigger("task-added".into()),
            answer(&db, Some(task)).await?,
        )
            .into_response());
    }
    Ok(Redirect::to("/").into_response())
}

/// One row, as shown in the list (the inline edit's Cancel asks for it).
async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let task = Task::find_or_404(&db, id).await?;
    Ok(view("tasks/_row.html", context! { task }))
}

/// The row as a small form, swapped in on double-click.
async fn edit(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let task = Task::find_or_404(&db, id).await?;
    Ok(view("tasks/_edit.html", context! { task }))
}

async fn update(
    State(db): State<Db>,
    htmx: Htmx,
    Path(id): Path<i64>,
    Valid(form): Valid<TaskForm>,
) -> Result<Response> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.title = form.title.trim().to_owned();
    task.save(&db).await?;
    row_or_home(&db, htmx, task).await
}

async fn toggle(State(db): State<Db>, htmx: Htmx, Path(id): Path<i64>) -> Result<Response> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.done = !task.done;
    task.save(&db).await?;
    row_or_home(&db, htmx, task).await
}

/// htmx swaps the row with an empty answer, which removes it; the open
/// count comes along out of band, and a toast confirms it.
async fn destroy(State(db): State<Db>, htmx: Htmx, Path(id): Path<i64>) -> Result<Response> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.delete(&db).await?;
    let toast = Toast::success(format!("“{}” deleted.", task.title));
    if htmx.request {
        return Ok((toast, answer(&db, None).await?).into_response());
    }
    Ok((toast, Redirect::to("/")).into_response())
}

/// Many rows change at once: simplest to reload the page (`HX-Refresh`).
/// The toast waits in the session for the reloaded page.
async fn clear_done(State(db): State<Db>, htmx: Htmx) -> Result<Response> {
    let gone = Task::where_eq("done", true).delete(&db).await?;
    let toast = Toast::success(format!("{gone} done tasks cleared."));
    if htmx.request {
        return Ok((toast, HxRefresh).into_response());
    }
    Ok((toast, Redirect::to("/")).into_response())
}

/// Done with the list: go to another page (`HX-Redirect` for htmx, 303 for
/// a plain form, both from `htmx.redirect`).
async fn archive(State(db): State<Db>, htmx: Htmx) -> Result<Response> {
    let done = Task::where_eq("done", true).count(&db).await?;
    Task::where_eq("done", true).delete(&db).await?;
    let toast = Toast::success(match done {
        1 => "1 task archived.".to_owned(),
        n => format!("{n} tasks archived."),
    });
    Ok((toast, htmx.redirect("/summary")).into_response())
}

async fn summary(State(db): State<Db>) -> Result<View> {
    let open = Task::where_eq("done", false).count(&db).await?;
    Ok(view("tasks/summary.html", context! { open }))
}

async fn row_or_home(db: &Db, htmx: Htmx, task: Task) -> Result<Response> {
    if htmx.request {
        return Ok(answer(db, Some(task)).await?.into_response());
    }
    Ok(Redirect::to("/").into_response())
}

/// The row (or nothing), and out of band the open count and the empty
/// list's message (shown or hidden) (tasks/answer.html).
async fn answer(db: &Db, task: Option<Task>) -> Result<View> {
    let open = Task::where_eq("done", false).count(db).await?;
    let total = Task::query().count(db).await?;
    Ok(view("tasks/answer.html", context! { task, open, total })
        .fragment("row")
        .also("count")
        .also("empty"))
}

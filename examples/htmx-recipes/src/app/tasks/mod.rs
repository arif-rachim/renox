//! Each handler answers an htmx request with the smallest piece of HTML that
//! changes (a row, some rows, nothing) and a plain request with a redirect,
//! so the page works without JavaScript too.

use renox::fake::Fake;
use renox::fake::faker::lorem::en::Sentence;
use renox::prelude::*;
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

/// The page, or (for the infinite scroll's `?page=2…`) just the next rows.
async fn index(State(db): State<Db>, htmx: Htmx, Page(page): Page) -> Result<View> {
    let tasks = Task::query().latest().paginate(&db, page, PER_PAGE).await?;
    if htmx.wants_fragment() {
        return Ok(view("tasks/_rows.html", context! { tasks }));
    }
    let open = Task::where_eq("done", false).count(&db).await?;
    Ok(view("tasks/index.html", context! { tasks, open }))
}

#[derive(Deserialize, Serialize)]
struct TaskForm {
    title: String,
}

impl Validate for TaskForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(100);
    }
}

/// From the modal: the new row, and an event the modal listens for to close.
/// Invalid input gets a 422 whose errors appear in the modal's form.
async fn store(State(db): State<Db>, htmx: Htmx, Valid(form): Valid<TaskForm>) -> Result<Response> {
    let task = Task::create(
        &db,
        Task {
            title: form.title,
            ..Default::default()
        },
    )
    .await?;
    if htmx.request {
        return Ok((
            HxTrigger("task-added".into()),
            view("tasks/_row.html", context! { task }),
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
    task.title = form.title;
    task.save(&db).await?;
    row_or_home(htmx, task)
}

async fn toggle(State(db): State<Db>, htmx: Htmx, Path(id): Path<i64>) -> Result<Response> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.done = !task.done;
    task.save(&db).await?;
    row_or_home(htmx, task)
}

/// htmx swaps the row with this empty answer, which removes it.
async fn destroy(State(db): State<Db>, htmx: Htmx, Path(id): Path<i64>) -> Result<Response> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.delete(&db).await?;
    if htmx.request {
        return Ok(StatusCode::OK.into_response());
    }
    Ok(Redirect::to("/").into_response())
}

/// Many rows change at once: simplest to reload the page (`HX-Refresh`).
async fn clear_done(State(db): State<Db>, htmx: Htmx, session: Session) -> Result<Response> {
    let gone = Task::where_eq("done", true).delete(&db).await?;
    session.flash("status", format!("{gone} done tasks cleared."))?;
    if htmx.request {
        return Ok(HxRefresh.into_response());
    }
    Ok(Redirect::to("/").into_response())
}

/// Done with the list: go to another page (`HX-Redirect` for htmx, 303 for
/// a plain form, both from `htmx.redirect`).
async fn archive(State(db): State<Db>, htmx: Htmx, session: Session) -> Result<Response> {
    let done = Task::where_eq("done", true).count(&db).await?;
    Task::where_eq("done", true).delete(&db).await?;
    session.flash("status", format!("{done} tasks archived."))?;
    Ok(htmx.redirect("/summary"))
}

async fn summary(State(db): State<Db>) -> Result<View> {
    let open = Task::where_eq("done", false).count(&db).await?;
    Ok(view("tasks/summary.html", context! { open }))
}

fn row_or_home(htmx: Htmx, task: Task) -> Result<Response> {
    if htmx.request {
        return Ok(view("tasks/_row.html", context! { task }).into_response());
    }
    Ok(Redirect::to("/").into_response())
}

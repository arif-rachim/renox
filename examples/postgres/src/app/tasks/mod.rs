//! Made with `rnx new tasks --database postgres`, `rnx make:module tasks` and
//! `rnx make:model Task --module tasks -m` (which wrote the PostgreSQL SQL;
//! the SQLite file was added by hand).

use renox::chrono::NaiveDate;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "tasks")]
pub struct Task {
    pub id: i64,
    pub title: String,
    /// BOOLEAN on PostgreSQL, 0/1 on SQLite.
    pub done: bool,
    /// DATE on PostgreSQL, text on SQLite.
    pub due_on: Option<NaiveDate>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

pub struct Tasks;

impl Module for Tasks {
    fn name(&self) -> &'static str {
        "tasks"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .post("/tasks", store)
            .name("tasks.store")
            .patch("/tasks/{id}", toggle)
            .name("tasks.toggle")
    }
}

#[derive(Deserialize)]
struct Filter {
    q: Option<String>,
    overdue: Option<bool>,
}

async fn index(
    State(db): State<Db>,
    Page(page): Page,
    Query(filter): Query<Filter>,
) -> Result<View> {
    let mut query = Task::query().order_by("due_on").order_by("id");
    if let Some(q) = filter.q.as_deref().filter(|q| !q.is_empty()) {
        query = query.where_like("title", format!("%{q}%")); // ignores case on both
    }
    if filter.overdue == Some(true) {
        let today = renox::db::now().date_naive();
        query = query.where_eq("done", false).where_op("due_on", "<", today);
    }
    let tasks = query.paginate(&db, page, 20).await?;
    Ok(view("tasks/index.html", context! { tasks, q => filter.q }))
}

#[derive(Deserialize)]
struct TaskForm {
    title: String,
    due_on: Option<NaiveDate>,
}

impl Validate for TaskForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title).required().max(200);
    }
}

async fn store(State(db): State<Db>, Valid(form): Valid<TaskForm>) -> Result<Redirect> {
    let task = Task {
        title: form.title,
        due_on: form.due_on,
        ..Default::default()
    };
    Task::create(&db, task).await?;
    Ok(Redirect::to("/"))
}

async fn toggle(State(db): State<Db>, Path(id): Path<i64>) -> Result<Redirect> {
    let mut task = Task::find_or_404(&db, id).await?;
    task.done = !task.done;
    task.save(&db).await?;
    Ok(Redirect::to("/"))
}

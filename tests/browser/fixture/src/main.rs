//! The pages `tests/browser` drives in headless Chrome: forms that fail
//! validation, the UI kit's widgets and overlays. Each page uses the kit
//! the way an app would; the browser tests check what renox.js and
//! renox-ui.js then do with it.

use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// A form that fails in every way the error placement handles.
#[derive(Deserialize, Serialize, Validate)]
struct Signup {
    #[validate(required, max = 10)]
    name: String,
    #[validate(required, email)]
    email: String,
    #[serde(default)]
    #[validate(each(required, max = 5))]
    tags: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct Line {
    name: String,
    #[serde(default)]
    qty: Option<i64>,
}

/// Rows of a repeater (nested names: `lines[0][name]`).
#[derive(Deserialize, Serialize)]
struct Order {
    #[serde(default)]
    lines: Vec<Line>,
}

impl Validate for Order {
    fn rules(&self, v: &mut Validator) {
        for (i, line) in self.lines.iter().enumerate() {
            v.field(&format!("lines.{i}.name"), &line.name).required();
            v.field(&format!("lines.{i}.qty"), &line.qty)
                .required()
                .min(1);
        }
    }
}

/// A wizard's two steps.
#[derive(Deserialize, Serialize, Validate)]
struct Trip {
    #[validate(required)]
    from: String,
    #[validate(required)]
    to: String,
}

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", || async { view("home.html", context! {}) })
            .name("home")
            .get("/form", || async { view("form.html", context! {}) })
            .post("/signup", |Valid(form): Valid<Signup>| async move {
                Toast::success(format!("Welcome, {}.", form.name))
            })
            .get("/plain", || async { view("plain.html", context! {}) })
            .post("/plain", |Valid(form): Valid<Signup>| async move {
                (
                    Toast::success(format!("Saved {}.", form.name)),
                    Redirect::to("/plain"),
                )
            })
            .get("/widgets", || async { view("widgets.html", context! {}) })
            .post("/order", |Valid(order): Valid<Order>| async move {
                format!("{} lines", order.lines.len())
            })
            .post("/trip", |Valid(trip): Valid<Trip>| async move {
                format!("{} to {}", trip.from, trip.to)
            })
            .post("/slow", || async {
                renox::tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                "done"
            })
            .post("/slow-plain", || async {
                renox::tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                Redirect::to("/widgets")
            })
            .get("/overlays", || async { view("overlays.html", context! {}) })
            .post("/toast", || async {
                Toast::warning("Stock is low.")
                    .link("Unsafe", "javascript:alert(1)")
                    .link("Open stock", "/stock")
                    .seconds(1)
            })
            .post("/sheet", |Valid(form): Valid<Trip>| async move {
                (Toast::success(format!("{} saved.", form.from)), "ok")
            })
            .get("/stock", || async { "stock page" })
            // A request still running when the server is told to stop.
            .get("/pause", || async {
                renox::tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                "finished"
            })
    }
}

/// Appends `line` to the file `FIXTURE_LOG` names, for tests/process to read.
fn note(line: &str) {
    use std::io::Write;
    if let Ok(path) = std::env::var("FIXTURE_LOG")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// A job that notes it ran.
#[derive(Serialize, Deserialize)]
struct Touch {
    n: u32,
}

impl Job for Touch {
    const NAME: &'static str = "fixture-touch";
    async fn handle(self, _ctx: JobContext) -> Result {
        note(&format!("job {}", self.n));
        Ok(())
    }
}

fn main() -> Result {
    let mut app = App::new()
        .module(Pages)
        .job::<Touch>()
        // `jobs:push 3`: queues three Touch jobs.
        .command("jobs:push", "Queues Touch jobs", |args, state| async move {
            let count: u32 = args
                .positional()
                .first()
                .and_then(|n| n.parse().ok())
                .unwrap_or(1);
            for n in 1..=count {
                state.dispatch(Touch { n }).await?;
            }
            Ok(())
        })
        // `ask:me`: asks every kind of question and prints the answers.
        .command("ask:me", "Asks questions", |_args, _state| async move {
            let name = renox::prompt::ask("Name?").await?;
            let sure = renox::prompt::confirm("Sure?", false).await?;
            let size = renox::prompt::choice("Size?", &["small", "large"], None).await?;
            let secret = renox::prompt::secret("Secret?").await?;
            println!(
                "answers: {name} | {sure} | {size} | {} characters",
                secret.len()
            );
            Ok(())
        });
    // A task every second, only when asked (tests/process).
    if std::env::var("FIXTURE_TICK").is_ok() {
        app = app.schedule(|s| {
            s.every(std::time::Duration::from_secs(1), "tick", |_state| async {
                note("tick");
                Ok(())
            });
        });
    }
    app.run()
}

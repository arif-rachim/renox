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
    }
}

fn main() -> Result {
    App::new().module(Pages).run()
}

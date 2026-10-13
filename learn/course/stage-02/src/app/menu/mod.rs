pub mod category;
pub mod model;
use renox::prelude::*;

pub struct Menu;

impl Module for Menu {
    fn name(&self) -> &'static str {
        "menu"
    }

    fn register(&self, app: &mut Registry) {
        app.model::<category::Category>();
        app.model::<model::MenuItem>();
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/menu", index).name("menu.index")
    }
}

async fn index() -> View {
    view("menu/index.html", context! {})
}

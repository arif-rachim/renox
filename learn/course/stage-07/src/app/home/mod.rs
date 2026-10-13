pub mod drink_builder;

use drink_builder::DrinkBuilder;
use renox::live_component::LiveContext;
use renox::prelude::*;

pub struct Home;

impl Module for Home {
    fn name(&self) -> &'static str {
        "home"
    }

    fn register(&self, app: &mut Registry) {
        app.live_component::<DrinkBuilder>();
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/", index).name("home")
    }
}

async fn index(ctx: LiveContext) -> Result<View> {
    let builder = ctx.mount(DrinkBuilder::default()).await?;
    Ok(view("home/index.html", context! { builder }))
}

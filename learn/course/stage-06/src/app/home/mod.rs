pub mod drink_builder;

use drink_builder::DrinkBuilder;
use renox::live_component::LiveContext;
use renox::prelude::*;

pub struct Home;

impl Module for Home {
    fn name(&self) -> &'static str {
        "home"
    }

    // [learn:home.register]
    fn register(&self, app: &mut Registry) {
        app.live_component::<DrinkBuilder>();
    }
    // [/learn:home.register]

    fn routes(&self) -> Routes {
        Routes::new().get("/", index).name("home")
    }
}

// [learn:home.mount]
async fn index(ctx: LiveContext) -> Result<View> {
    let builder = ctx.mount(DrinkBuilder::default()).await?;
    Ok(view("home/index.html", context! { builder }))
}
// [/learn:home.mount]

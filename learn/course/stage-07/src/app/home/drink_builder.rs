use renox::live_component::LiveContext;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// A drink being built on the home page. Nothing here touches the database:
/// the state is the choices, and the price is worked out from them.
#[derive(Serialize, Deserialize, Validate)]
pub struct DrinkBuilder {
    /// `"small"`, `"medium"` or `"large"`.
    pub size: String,
    /// Espresso shots, from 1 to 4.
    pub shots: i64,
    #[validate(required, max = 30, label = "Name on the cup")]
    pub name: String,
}

impl Default for DrinkBuilder {
    fn default() -> Self {
        DrinkBuilder {
            size: "small".into(),
            shots: 1,
            name: String::new(),
        }
    }
}

#[renox::live_component(view = "live/drink_builder.html", name = "drink-builder")]
impl DrinkBuilder {
    /// Extra values for the view: the price in cents. They are not in the snapshot.
    async fn data(&self, _ctx: &LiveContext) -> Result<renox::serde_json::Value> {
        let base = match self.size.as_str() {
            "medium" => 375,
            "large" => 450,
            _ => 300,
        };
        let price = base + 75 * (self.shots - 1);
        Ok(json!({ "price": price }))
    }

    #[live(action)]
    async fn size(&mut self, _ctx: &mut LiveContext, size: String) -> Result {
        if !["small", "medium", "large"].contains(&size.as_str()) {
            return Err(Error::BadRequest("Pick small, medium or large.".into()));
        }
        self.size = size;
        Ok(())
    }

    #[live(action)]
    async fn add_shot(&mut self, _ctx: &mut LiveContext) -> Result {
        self.shots = (self.shots + 1).min(4);
        Ok(())
    }

    #[live(action)]
    async fn remove_shot(&mut self, _ctx: &mut LiveContext) -> Result {
        self.shots = (self.shots - 1).max(1);
        Ok(())
    }

    #[live(action)]
    async fn order(&mut self, ctx: &mut LiveContext) -> Result {
        ctx.validate(&*self).await?;
        ctx.toast(Toast::success(format!(
            "One {} for {}.",
            self.size, self.name
        )));
        Ok(())
    }
}

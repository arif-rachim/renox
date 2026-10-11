use renox::live_component::LiveContext;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::model::MenuItem;

/// The stock of one menu item, sold and refilled from its page. The state is
/// only the item's id: the stock, the name and the author are read from the
/// database every time.
#[derive(Serialize, Deserialize, Default)]
pub struct StockCounter {
    pub item_id: i64,
}

#[renox::live_component(view = "live/stock_counter.html", name = "stock-counter")]
impl StockCounter {
    /// What the view shows: the item as it is now, and whether this user may change it.
    async fn data(&self, ctx: &LiveContext) -> Result<renox::serde_json::Value> {
        let item = MenuItem::find_or_404(&ctx.state().db, self.item_id).await?;
        Ok(json!({
            "item": item,
            "can_update": ctx.user().is_ok_and(|user| user.can("update", &item)),
        }))
    }

    #[live(action)]
    async fn sell(&mut self, ctx: &mut LiveContext) -> Result {
        let db = &ctx.state().db;
        let mut item = MenuItem::find_or_404(db, self.item_id).await?;
        ctx.user()?.authorize("update", &item)?;
        let left = match item.stock {
            Some(n) if n > 0 => n - 1,
            _ => return Err(Error::BadRequest("Nothing left to sell.".into())),
        };
        item.stock = Some(left);
        item.save(db).await?;
        ctx.toast(Toast::success(if left == 0 {
            format!("{} is sold out.", item.name)
        } else {
            format!("Sold one {}.", item.name)
        }));
        Ok(())
    }

    #[live(action)]
    async fn sell_many(&mut self, ctx: &mut LiveContext, n: i64) -> Result {
        let db = &ctx.state().db;
        let mut item = MenuItem::find_or_404(db, self.item_id).await?;
        ctx.user()?.authorize("update", &item)?;
        let left = item.stock.unwrap_or(0);
        if !(1..=left).contains(&n) {
            return Err(Error::BadRequest(format!("Sell between 1 and {left}.")));
        }
        item.stock = Some(left - n);
        item.save(db).await?;
        ctx.toast(Toast::success(format!("Sold {n} {}.", item.name)));
        Ok(())
    }

    #[live(action)]
    async fn restock(&mut self, ctx: &mut LiveContext, amount: i64) -> Result {
        let db = &ctx.state().db;
        let mut item = MenuItem::find_or_404(db, self.item_id).await?;
        ctx.user()?.authorize("update", &item)?;
        if !(1..=24).contains(&amount) {
            return Err(Error::BadRequest("Add between 1 and 24.".into()));
        }
        item.stock = Some(item.stock.unwrap_or(0) + amount);
        item.save(db).await?;
        ctx.toast(Toast::success(format!("Added {amount} {}.", item.name)));
        Ok(())
    }
}

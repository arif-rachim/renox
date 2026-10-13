use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// One thing on the menu: a drink, or something to eat.
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "menu_items", form)]
pub struct MenuItem {
    pub id: i64,
    pub name: String,
    /// In cents: 450 is $4.50.
    pub price: i64,
    pub stock: Option<i64>,
}

impl MenuItem {
    /// The items under `cents`, cheapest first.
    pub async fn cheaper_than(db: &Db, cents: i64) -> Result<Vec<MenuItem>> {
        let items = MenuItem::query()
            .where_(MenuItem::PRICE.lt(cents))
            .order_by(MenuItem::PRICE)
            .get(db)
            .await?;
        Ok(items)
    }
}

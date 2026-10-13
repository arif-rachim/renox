use renox::prelude::*;
use serde::{Deserialize, Serialize};

// [learn:menu.item-model]
/// One thing on the menu: a drink, or something to eat.
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "menu_items", unique(name), index(category_id), index(user_id))]
pub struct MenuItem {
    pub id: i64,
    #[model(references = "categories")]
    pub category_id: i64,
    /// Who added it.
    #[model(references = "users")]
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    /// In cents: 450 is $4.50.
    pub price: i64,
    #[model(default = "true")]
    pub available: bool,
    /// Portions left today. `None` means they aren't counted, like a drink.
    pub stock: Option<i64>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}
// [/learn:menu.item-model]

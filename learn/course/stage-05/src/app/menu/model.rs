use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// One thing on the menu: a drink, or something to eat.
// [learn:menu.item-model]
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(
    table = "menu_items",
    unique(name),
    index(category_id),
    index(user_id),
    form
)]
pub struct MenuItem {
    pub id: i64,
    #[model(references = "categories")]
    #[form(validate(required, exists("categories", "id")))]
    pub category_id: i64,
    /// Who added it.
    #[model(references = "users")]
    #[form(skip)]
    pub user_id: i64,
    #[form(validate(required, max = 80, unique("menu_items", "name")))]
    pub name: String,
    #[form(validate(max = 300))]
    pub description: Option<String>,
    /// In cents: 450 is $4.50.
    #[form(validate(required, min = 1, max = 100000))]
    pub price: i64,
    #[model(default = "true")]
    pub available: bool,
    /// Portions left today. `None` means they aren't counted, like a drink.
    #[form(validate(min = 0, max = 999))]
    pub stock: Option<i64>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}
// [/learn:menu.item-model]

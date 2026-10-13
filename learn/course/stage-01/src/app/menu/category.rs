use renox::prelude::*;
use serde::{Deserialize, Serialize};

// [learn:menu.category-model]
/// A group of menu items: Coffee, Tea, Pastry.
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "categories", unique(name))]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}
// [/learn:menu.category-model]

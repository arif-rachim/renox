use renox::fake::Fake;
use renox::fake::faker::lorem::en::Word;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products", soft_deletes)]
pub struct Product {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    /// In the smallest currency unit (e.g. rupiah), to avoid float rounding.
    pub price: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub deleted_at: Option<DateTime>,
}

impl Factory for Product {
    fn definition() -> Self {
        Product {
            name: Word().fake(),
            price: (1_000..100_000).fake(),
            ..Default::default()
        }
    }
}

impl Product {
    /// A fake product owned by `user`, for seeders and tests.
    pub fn for_owner(user: &User) -> Self {
        Product {
            user_id: user.id,
            ..Product::make()
        }
    }
}

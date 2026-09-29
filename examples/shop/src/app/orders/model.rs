use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// Where an order is. Stored as text (`pending`, `paid`, …).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
pub enum OrderStatus {
    /// Placed, waiting for the bank transfer.
    #[default]
    Pending,
    Paid,
    Shipped,
    /// By the admin, or by the daily task when nobody paid for 3 days.
    Cancelled,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    pub user_id: i64,
    pub status: OrderStatus,
    /// In rupiah.
    pub total: i64,
    pub address: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "order_items")]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    /// `None` once the product is deleted; the name and price stay.
    pub product_id: Option<i64>,
    pub name: String,
    pub price: i64,
    pub quantity: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Order {
    pub async fn items(&self, db: &Db) -> Result<Vec<OrderItem>> {
        OrderItem::where_eq("order_id", self.id)
            .order_by("id")
            .get(db)
            .await
    }
}

/// Customers see their own orders; admins see every order. `has_role` on
/// the plain `User` a policy gets reads the roles the request loaded.
impl Policy for Order {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "view" => self.user_id == user.id || user.has_role(crate::ADMIN),
            _ => false,
        }
    }
}

//! Orders, their lines and payments (Pagila's `payment`, plus orders).
//!
//! - An [`Order`] is placed online or at a store's counter; its
//!   **operating store** is the one that sells it (and earns a fee when
//!   the goods are another store's).
//! - Each [`OrderItem`] keeps the **owner store** of the goods sold: for
//!   goods on consignment that's another store, whose books get the
//!   revenue (#245).
//! - A [`Payment`] pays for an order, a rental or a work order: a [`Morph`]
//!   reference ([`PAYABLE`]), received by the store that took the money.
//!
//! Migration: `migrations/20260101000700_create_sales_tables.*`.

use renox::db::relations::Morph;
use renox::prelude::*;
use serde::Serialize;

use crate::app::access::{StoreAttr, StoreRecord, catalogue};

/// Where the order came from.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Channel {
    /// The web shop.
    #[default]
    Online,
    /// A store's counter.
    Counter,
}

/// How the customer gets the goods.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fulfilment {
    /// Picked up at the operating store.
    #[default]
    Pickup,
    /// Delivered to `delivery_address_id`.
    Delivery,
}

/// Where an order stands.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderStatus {
    /// Placed, not paid yet.
    #[default]
    Pending,
    /// Paid, being prepared.
    Paid,
    /// Ready to pick up, or out for delivery.
    Ready,
    /// In the customer's hands.
    Completed,
    /// Called off before it was paid.
    Cancelled,
    /// Paid back.
    Refunded,
}

/// An order of bikes and gear.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    /// What the customer sees: `N-24051`.
    pub number: String,
    /// `None` for an anonymous counter sale.
    pub customer_id: Option<i64>,
    /// The store that sells it.
    pub operating_store_id: i64,
    pub channel: Channel,
    pub fulfilment: Fulfilment,
    pub status: OrderStatus,
    /// Money in the smallest unit of `APP_CURRENCY`.
    pub subtotal: i64,
    pub discount: i64,
    pub delivery_fee: i64,
    pub total: i64,
    pub delivery_address_id: Option<i64>,
    pub placed_at: Option<DateTime>,
    pub paid_at: Option<DateTime>,
    /// Who sold it at the counter (a `staff` row).
    pub served_by: Option<i64>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl StoreRecord for Order {
    const VIEW: &'static str = catalogue::ORDERS_VIEW;
    const STORE_COLUMNS: &'static [&'static str] = &["operating_store_id"];

    fn store_id(&self, attr: StoreAttr) -> Option<i64> {
        match attr {
            StoreAttr::Operating | StoreAttr::Location => Some(self.operating_store_id),
            // Each line has its owner; the order itself has none.
            StoreAttr::Owner => None,
        }
    }
}

/// A line of an order.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "order_items")]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub variant_id: i64,
    /// Whose goods these were (another store's for consigned goods).
    pub owner_store_id: i64,
    pub quantity: i64,
    pub unit_price: i64,
    pub total: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// What a payment pays for: `payable_type` is `orders`, `rentals` or
/// `work_orders`, `payable_id` the row.
pub const PAYABLE: Morph = Morph::new("payable_type", "payable_id");

/// How a customer paid.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentMethod {
    #[default]
    Cash,
    Card,
    /// Online, through a payment gateway (`gateway_reference`).
    Gateway,
}

/// Where a payment stands.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentStatus {
    #[default]
    Pending,
    Paid,
    Failed,
    Refunded,
}

/// A payment for an order, a rental or a work order (Pagila's `payment`).
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "payments")]
pub struct Payment {
    pub id: i64,
    pub customer_id: Option<i64>,
    pub payable_type: String,
    pub payable_id: i64,
    /// The store that received the money (the operating store).
    pub store_id: i64,
    pub amount: i64,
    pub method: PaymentMethod,
    pub gateway_reference: Option<String>,
    pub status: PaymentStatus,
    pub paid_at: Option<DateTime>,
    /// Who took it (a `staff` row).
    pub received_by: Option<i64>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl StoreRecord for Payment {
    const VIEW: &'static str = catalogue::ORDERS_VIEW;
    const STORE_COLUMNS: &'static [&'static str] = &["store_id"];

    fn store_id(&self, _attr: StoreAttr) -> Option<i64> {
        Some(self.store_id)
    }
}

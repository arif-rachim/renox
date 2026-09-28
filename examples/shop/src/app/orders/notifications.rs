//! What customers and admins are told about orders.

use renox::auth::{Channel, Notification, Recipient};
use renox::mail::Mail;
use renox::prelude::*;

use super::model::{Order, OrderItem};

/// To the customer, when the order is placed: a mail with what to pay, and
/// a row in their notifications.
pub struct OrderConfirmation {
    pub order: Order,
    pub items: Vec<OrderItem>,
}

impl Notification for OrderConfirmation {
    fn kind(&self) -> &'static str {
        "order-confirmation"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database]
    }

    fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
        state.mail_view(
            to.email().unwrap_or_default(),
            format!("Order #{} received", self.order.id),
            "mail/order_confirmation", // .html and .txt
            context! { order => self.order, items => self.items },
        )
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        json!({ "order_id": self.order.id, "total": self.order.total })
    }
}

/// To every admin, when an order is placed.
pub struct NewOrder(pub Order);

impl Notification for NewOrder {
    fn kind(&self) -> &'static str {
        "new-order"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Database]
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        json!({ "order_id": self.0.id, "total": self.0.total })
    }
}

/// To the customer, when the admin ships the order.
pub struct OrderShipped(pub Order);

impl Notification for OrderShipped {
    fn kind(&self) -> &'static str {
        "order-shipped"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database]
    }

    fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
        state.mail_view(
            to.email().unwrap_or_default(),
            format!("Order #{} is on its way", self.0.id),
            "mail/order_shipped",
            context! { order => self.0 },
        )
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        json!({ "order_id": self.0.id })
    }
}

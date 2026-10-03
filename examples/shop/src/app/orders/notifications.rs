//! What customers and admins are told about orders.

use renox::auth::{Channel, DatabaseMessage, Notification, Recipient};
use renox::mail::Mail;
use renox::prelude::*;

use super::model::{Order, OrderItem};

/// A text in the recipient's language: `to_database` runs in it, like
/// `to_mail` (`renox::context::app()` is the running app).
fn t(key: &str, params: &[(&str, &dyn std::fmt::Display)]) -> String {
    match renox::context::app() {
        Some(state) => state.current_lang().t(key, params),
        None => key.to_owned(),
    }
}

/// An amount the way the `money` filter writes it (`APP_CURRENCY`).
fn money(amount: i64) -> String {
    match renox::context::app() {
        Some(state) => renox::format_money(
            amount as f64,
            &state.config.currency,
            None,
            &state.current_lang().locale,
        ),
        None => amount.to_string(),
    }
}

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
            t("mail.confirmation.subject", &[("id", &self.order.id)]),
            "mail/order_confirmation", // .html and .txt
            context! { order => self.order, items => self.items },
        )
    }

    // What the bell shows (and pushes as a toast while the customer is on
    // the site); `order_id` and `total` stay for the shop's own pages.
    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        DatabaseMessage::info(t("notifications.placed", &[("id", &self.order.id)]))
            .body(t(
                "notifications.pay",
                &[("total", &money(self.order.total)), ("days", &3)],
            ))
            .url(format!("/orders/{}", self.order.id))
            .with("order_id", self.order.id)
            .with("total", self.order.total)
            .into()
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
        DatabaseMessage::success(t("notifications.new_order", &[("id", &self.0.id)]))
            .body(t(
                "notifications.new_order_body",
                &[
                    ("total", &money(self.0.total)),
                    ("address", &self.0.address),
                ],
            ))
            .url(format!("/orders/{}", self.0.id))
            .link(
                t("notifications.all_orders", &[]),
                "/admin/orders?status=pending",
            )
            .with("order_id", self.0.id)
            .with("total", self.0.total)
            .into()
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
            t(
                if self.0.pickup {
                    "mail.shipped.ready_subject"
                } else {
                    "mail.shipped.subject"
                },
                &[("id", &self.0.id)],
            ),
            "mail/order_shipped",
            context! { order => self.0 },
        )
    }

    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        let key = if self.0.pickup {
            "notifications.ready"
        } else {
            "notifications.shipped"
        };
        DatabaseMessage::success(t(key, &[("id", &self.0.id)]))
            .url(format!("/orders/{}", self.0.id))
            .with("order_id", self.0.id)
            .into()
    }
}

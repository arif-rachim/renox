use renox::auth::{Channel, DatabaseMessage, Notification, Recipient};
use renox::mail::Mail;
use renox::prelude::*;

use super::Order;

/// Tells an admin about a new order: by mail, and in the database for an
/// in-app list (`user.unread_notifications(&db)`).
pub struct NewOrder(pub Order);

impl Notification for NewOrder {
    fn kind(&self) -> &'static str {
        "new-order"
    }

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail, Channel::Database]
    }

    fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
        let order = &self.0;
        Ok(Mail::new(
            to.email().unwrap_or_default(),
            format!("New order #{}", order.id),
            format!(
                "{} ordered {} ({}).",
                order.customer_email,
                order.item,
                super::money(state, order.total)
            ),
        ))
    }

    // A `DatabaseMessage`: what the UI kit's `notification_bell` shows,
    // plus the app's own keys.
    fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
        DatabaseMessage::info(format!("New order #{}", self.0.id))
            .body(format!(
                "Total: {}",
                renox::context::app()
                    .map(|state| super::money(&state, self.0.total))
                    .unwrap_or_else(|| self.0.total.to_string())
            ))
            .with("order_id", self.0.id)
            .with("total", self.0.total)
            .into()
    }
}

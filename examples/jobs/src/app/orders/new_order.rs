use renox::auth::{Channel, Notification};
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

    fn to_mail(&self, user: &User, _: &AppState) -> Result<Mail> {
        let order = &self.0;
        Ok(Mail::new(
            &user.email,
            format!("New order #{}", order.id),
            format!(
                "{} ordered {} (Rp {}).",
                order.customer_email, order.item, order.total
            ),
        ))
    }

    fn to_database(&self, _: &User) -> renox::serde_json::Value {
        json!({ "order_id": self.0.id, "total": self.0.total })
    }
}

//! Telling the customer: queued mails on the kit's mail layout and, for
//! customers with an account, an in-app notification.
//!
//! | Moment | Mail (`resources/views/mail/sales/…`) | Notification |
//! |---|---|---|
//! | paid | `confirmation`: the lines, the totals, pickup or delivery | "Order N-… is paid" |
//! | ready for pickup | `ready` | "Ready for pickup" |
//! | sent out | `shipped` | "On its way" |
//! | returned and refunded | `refund` | "Refunded" |
//! | not paid in 30 minutes | `expired` | "Cancelled" |
//!
//! Mails are written in the language the customer ordered in
//! (`orders.locale`, `mail_view_in`), even when the queue or a member of
//! staff sends them later. The link in each mail is a signed URL to the
//! order (`orders.signed`), so a guest can open it without an account.

use std::time::Duration;

use renox::auth::{Channel, DatabaseMessage, Notification, Recipient};
use renox::prelude::*;
use serde::Serialize;

use super::model::{Fulfilment, Order, OrderItem};
use crate::app::accounts::model::{Customer, FullAddress};
use crate::app::catalog::model::{Product, ProductVariant};
use crate::app::staff::model::Store;

/// A signed link to the order works this long (the return window and a bit).
pub const LINK_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 30);

/// What a mail or a page shows of an order.
#[derive(Serialize, Debug, Clone)]
pub struct OrderView {
    #[serde(flatten)]
    pub order: Order,
    pub lines: Vec<LineView>,
    pub store: Option<Store>,
    pub customer: Option<Customer>,
    /// The delivery address in one line.
    pub address: Option<String>,
    pub delivery: bool,
}

/// A line with its product.
#[derive(Serialize, Debug, Clone)]
pub struct LineView {
    #[serde(flatten)]
    pub item: OrderItem,
    pub name: String,
    pub sku: String,
    pub variant: String,
    pub slug: String,
}

impl OrderView {
    /// The order with its lines, products, store, customer and address:
    /// seven queries however many lines.
    pub async fn load(db: &Db, order: Order) -> Result<OrderView> {
        let items = OrderItem::where_eq("order_id", order.id)
            .order_by("id")
            .get(db)
            .await?;
        let variants =
            ProductVariant::find_many(db, items.iter().map(|i| i.variant_id).collect::<Vec<_>>())
                .await?;
        // Discontinued products still name the lines of old orders.
        let products = Product::query()
            .with_trashed()
            .where_in(
                "id",
                variants.iter().map(|v| v.product_id).collect::<Vec<_>>(),
            )
            .get(db)
            .await?;
        let lines = items
            .into_iter()
            .map(|item| {
                let variant = variants.iter().find(|v| v.id == item.variant_id);
                let product = variant.and_then(|v| products.iter().find(|p| p.id == v.product_id));
                LineView {
                    name: product.map(|p| p.name.clone()).unwrap_or_default(),
                    slug: product.map(|p| p.slug.clone()).unwrap_or_default(),
                    sku: variant.map(|v| v.sku.clone()).unwrap_or_default(),
                    variant: variant
                        .map(|v| {
                            [v.size.clone(), v.colour.clone()]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · ")
                        })
                        .unwrap_or_default(),
                    item,
                }
            })
            .collect();
        let store = Store::find(db, order.operating_store_id).await?;
        let customer = match order.customer_id {
            Some(id) => {
                Customer::query()
                    .with_trashed()
                    .where_eq("id", id)
                    .first(db)
                    .await?
            }
            None => None,
        };
        let address = match order.delivery_address_id {
            Some(id) => FullAddress::load(db, vec![id])
                .await?
                .remove(&id)
                .map(|a| a.line()),
            None => None,
        };
        Ok(OrderView {
            delivery: order.fulfilment == Fulfilment::Delivery,
            order,
            lines,
            store,
            customer,
            address,
        })
    }
}

/// The moments a customer hears about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    Paid,
    Ready,
    Shipped,
    Refunded,
    Expired,
}

impl Moment {
    /// The mail's view and the translation keys' middle part.
    pub fn key(self) -> &'static str {
        match self {
            Moment::Paid => "confirmation",
            Moment::Ready => "ready",
            Moment::Shipped => "shipped",
            Moment::Refunded => "refund",
            Moment::Expired => "expired",
        }
    }
}

/// Queues the mail for `moment` (when the customer gave an address) and
/// notifies their account (when they have one). `refund`: the amount paid
/// back, for the refund mail.
pub async fn tell(state: &AppState, order: &Order, moment: Moment, refund: Option<i64>) -> Result {
    let Some(customer_id) = order.customer_id else {
        return Ok(()); // a walk-in at the counter
    };
    let view = OrderView::load(&state.db, order.clone()).await?;
    let Some(customer) = view.customer.clone() else {
        return Ok(());
    };
    let locale = order
        .locale
        .clone()
        .unwrap_or_else(|| state.config.locale.clone());
    let lang = state.lang(&locale);
    let number = order.number.clone();
    let url = state.signed_url("orders.signed", &[&order.id], LINK_TTL)?;
    let subject = lang.t(
        &format!("sales.mail.{}.subject", moment.key()),
        &[("number", &number)],
    );
    if let Some(email) = customer.email.as_deref().filter(|e| !e.is_empty()) {
        let mail = state.mail_view_in(
            &locale,
            email,
            subject.clone(),
            &format!("mail/sales/{}", moment.key()),
            context! { order => view, url, refund, name => customer.name.clone() },
        )?;
        state.queue_mail(mail).await?;
    }
    if let Some(user_id) = customer.user_id
        && let Some(user) = User::find(&state.db, user_id).await?
    {
        let body = lang.t(
            &format!("sales.notice.{}", moment.key()),
            &[("number", &number)],
        );
        state
            .notify(
                &user,
                &OrderNotice {
                    moment,
                    order_id: order.id,
                    title: subject,
                    body,
                },
            )
            .await?;
    }
    let _ = customer_id;
    Ok(())
}

/// The in-app notification about an order (a `DatabaseMessage`, shown by
/// the kit's bell and notification list).
pub struct OrderNotice {
    pub moment: Moment,
    pub order_id: i64,
    pub title: String,
    pub body: String,
}

impl Notification for OrderNotice {
    fn kind(&self) -> &'static str {
        match self.moment {
            Moment::Paid => "order-paid",
            Moment::Ready => "order-ready",
            Moment::Shipped => "order-shipped",
            Moment::Refunded => "order-refunded",
            Moment::Expired => "order-expired",
        }
    }

    fn channels(&self, _to: &Recipient) -> Vec<Channel> {
        vec![Channel::Database]
    }

    fn to_database(&self, _: &Recipient, _state: &AppState) -> Result<renox::serde_json::Value> {
        let message = match self.moment {
            Moment::Expired => DatabaseMessage::warning(&self.title),
            Moment::Refunded => DatabaseMessage::info(&self.title),
            _ => DatabaseMessage::success(&self.title),
        };
        Ok(message
            .body(&self.body)
            .url(format!("/orders/{}", self.order_id))
            .with("order_id", self.order_id)
            .into())
    }
}

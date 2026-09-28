//! Turning a cart into an order, all or nothing.

use renox::prelude::*;

use super::model::{Order, OrderItem, OrderStatus};
use crate::app::cart::{self, CartItem};

pub enum Checkout {
    Placed(Order),
    EmptyCart,
    /// These products don't have enough stock left; nothing was changed.
    OutOfStock(Vec<String>),
}

/// Takes the stock, writes the order and its items and empties the cart in
/// one transaction: if any product runs out, none of it happens.
///
/// `db.retrying` runs the attempt again only on database conflicts (SQLite
/// busy, a PostgreSQL deadlock between two carts taking the same products).
/// The attempt borrows `lines` and `address` from here and opens its own
/// transaction; running out of stock rolls it back and still answers
/// `Checkout::OutOfStock`. Each attempt starts from the cart as read before
/// the transaction, so running it again is safe.
pub async fn place(db: &Db, user_id: i64, address: &str) -> Result<Checkout> {
    let lines = cart::lines(db, user_id).await?;
    if lines.is_empty() {
        return Ok(Checkout::EmptyCart);
    }

    db.retrying(3, || async {
        let mut tx = db.begin().await?;
        let mut short = Vec::new();
        for line in &lines {
            // Checked and taken in one statement, so two buyers of the last
            // item can't both get it.
            let taken = renox::db::sql(
                "UPDATE products SET stock = stock - ? \
                 WHERE id = ? AND active = ? AND stock >= ?",
            )
            .bind(line.item.quantity)
            .bind(line.product.id)
            .bind(true)
            .bind(line.item.quantity)
            .execute(&mut tx)
            .await?;
            if taken == 0 {
                short.push(line.product.name.clone());
            }
        }
        if !short.is_empty() {
            tx.rollback().await?; // the stock taken for the other lines goes back
            return Ok(Checkout::OutOfStock(short));
        }

        let order = Order::create(
            &mut tx,
            Order {
                user_id,
                status: OrderStatus::Pending,
                total: lines.iter().map(|l| l.subtotal).sum(),
                address: address.to_owned(),
                ..Default::default()
            },
        )
        .await?;
        let items = lines
            .iter()
            .map(|line| OrderItem {
                order_id: order.id,
                product_id: Some(line.product.id),
                name: line.product.name.clone(),
                price: line.product.price,
                quantity: line.item.quantity,
                ..Default::default()
            })
            .collect();
        OrderItem::insert_many(&mut tx, items).await?;
        CartItem::where_eq("user_id", user_id)
            .delete(&mut tx)
            .await?;
        tx.commit().await?;
        Ok(Checkout::Placed(order))
    })
    .await
}

/// Cancels an order that is still pending and puts its items back in stock.
/// Returns false if it wasn't pending (paid meanwhile, or already cancelled).
pub async fn cancel(db: &Db, order: &Order) -> Result<bool> {
    // Read before the transaction: on SQLite, while it's open, everything
    // goes through `&mut tx` (a test database has only one connection).
    let items = order.items(db).await?;
    let mut tx = db.begin().await?;
    let changed = Order::where_eq("id", order.id)
        .where_eq("status", OrderStatus::Pending)
        .update(&mut tx, &[("status", &OrderStatus::Cancelled)])
        .await?;
    if changed == 0 {
        return Ok(false);
    }
    for item in items {
        if let Some(product_id) = item.product_id {
            renox::db::sql("UPDATE products SET stock = stock + ? WHERE id = ?")
                .bind(item.quantity)
                .bind(product_id)
                .execute(&mut tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}

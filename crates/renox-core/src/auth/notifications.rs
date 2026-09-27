//! Notifications: one message to a user, delivered by email, stored for an
//! in-app list, or both.
//!
//! ```ignore
//! struct OrderShipped { order_id: i64 }
//!
//! impl Notification for OrderShipped {
//!     fn kind(&self) -> &'static str { "order-shipped" }
//!     fn channels(&self) -> Vec<Channel> { vec![Channel::Mail, Channel::Database] }
//!
//!     fn to_mail(&self, user: &User, state: &AppState) -> Result<Mail> {
//!         state.mail_view(&user.email, "Pesanan dikirim", "mail/shipped", context! { id => self.order_id })
//!     }
//!
//!     fn to_database(&self, _: &User) -> serde_json::Value {
//!         json!({ "order_id": self.order_id })
//!     }
//! }
//!
//! state.notify(&user, &OrderShipped { order_id }).await?;
//! let unread = user.unread_notifications(&db).await?;
//! ```

use anyhow::anyhow;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

use super::User;
use crate::db::{DateTime, Db, now};
use crate::mail::Mail;
use crate::{AppState, Result};

/// Where a notification goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Sent now with `state.mailer` (dispatch a job to send it later).
    Mail,
    /// Stored in the `notifications` table for an in-app list.
    Database,
}

pub trait Notification: Send + Sync {
    /// Stored with database notifications, e.g. to pick an icon.
    fn kind(&self) -> &'static str;

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail]
    }

    fn to_mail(&self, _user: &User, _state: &AppState) -> Result<Mail> {
        Err(anyhow!("notification `{}` has no mail version", self.kind()).into())
    }

    fn to_database(&self, _user: &User) -> Value {
        Value::Null
    }
}

/// A stored notification.
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseNotification {
    pub id: i64,
    pub kind: String,
    pub data: Value,
    pub read_at: Option<DateTime>,
    pub created_at: DateTime,
}

impl AppState {
    /// Delivers `notification` to `user` on each of its channels.
    pub async fn notify(&self, user: &User, notification: &impl Notification) -> Result {
        for channel in notification.channels() {
            match channel {
                Channel::Mail => {
                    let mail = notification.to_mail(user, self)?;
                    self.mailer.send(mail).await?;
                }
                Channel::Database => {
                    sqlx::query(
                        "INSERT INTO notifications (user_id, kind, data, created_at) VALUES (?, ?, ?, ?)",
                    )
                    .bind(user.id)
                    .bind(notification.kind())
                    .bind(notification.to_database(user).to_string())
                    .bind(now())
                    .execute(&self.db)
                    .await?;
                }
            }
        }
        Ok(())
    }
}

fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<DatabaseNotification> {
    let data: String = row.try_get("data")?;
    Ok(DatabaseNotification {
        id: row.try_get("id")?,
        kind: row.try_get("kind")?,
        data: serde_json::from_str(&data).unwrap_or(Value::Null),
        read_at: row.try_get("read_at")?,
        created_at: row.try_get("created_at")?,
    })
}

impl User {
    /// The user's notifications, newest first.
    pub async fn notifications(&self, db: &Db, limit: u32) -> Result<Vec<DatabaseNotification>> {
        let rows = sqlx::query(
            "SELECT id, kind, data, read_at, created_at FROM notifications \
             WHERE user_id = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(self.id)
        .bind(i64::from(limit))
        .fetch_all(db)
        .await?;
        rows.iter().map(from_row).collect()
    }

    pub async fn unread_notifications(&self, db: &Db) -> Result<Vec<DatabaseNotification>> {
        let rows = sqlx::query(
            "SELECT id, kind, data, read_at, created_at FROM notifications \
             WHERE user_id = ? AND read_at IS NULL ORDER BY id DESC",
        )
        .bind(self.id)
        .fetch_all(db)
        .await?;
        rows.iter().map(from_row).collect()
    }

    pub async fn unread_notification_count(&self, db: &Db) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(self.id)
        .fetch_one(db)
        .await?)
    }

    /// Marks one of the user's notifications read; returns whether it was theirs.
    pub async fn mark_notification_read(&self, db: &Db, id: i64) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE notifications SET read_at = COALESCE(read_at, ?) WHERE id = ? AND user_id = ?",
        )
        .bind(now())
        .bind(id)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn mark_all_notifications_read(&self, db: &Db) -> Result<u64> {
        let done = sqlx::query(
            "UPDATE notifications SET read_at = ? WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(now())
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(done.rows_affected())
    }
}

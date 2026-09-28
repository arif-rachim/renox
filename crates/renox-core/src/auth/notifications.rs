//! Notifications: one message to a user (or to an address), delivered by
//! email, stored for an in-app list, sent through the app's own channels
//! (WhatsApp, SMS, Slack…), or all of these.
//!
//! ```
//! # use renox::prelude::*;
//! use renox::auth::{Channel, Notification, Recipient};
//! use renox::mail::Mail;
//!
//! struct OrderShipped { order_id: i64 }
//!
//! impl Notification for OrderShipped {
//!     fn kind(&self) -> &'static str { "order-shipped" }
//!     fn channels(&self) -> Vec<Channel> {
//!         vec![Channel::Mail, Channel::Database, Channel::Custom("whatsapp")]
//!     }
//!
//!     fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
//!         let email = to.email().unwrap_or_default();
//!         state.mail_view(&email, "Pesanan dikirim", "mail/shipped", context! { id => self.order_id })
//!     }
//!
//!     fn to_database(&self, _: &Recipient) -> renox::serde_json::Value {
//!         json!({ "order_id": self.order_id })
//!     }
//!
//!     fn to_channel(&self, _channel: &str, _: &Recipient) -> Result<renox::serde_json::Value> {
//!         Ok(json!({ "text": format!("Pesanan #{} sudah dikirim", self.order_id) }))
//!     }
//! }
//!
//! # async fn demo(state: AppState, user: User, db: Db, order_id: i64) -> Result {
//! state.notify(&user, &OrderShipped { order_id }).await?;          // now
//! state.notify_later(&user, &OrderShipped { order_id }).await?;    // through the queue
//! // Someone without an account:
//! let guest = Recipient::to("mail", "tamu@example.com").and("whatsapp", "+6281234567890");
//! state.notify_to(&guest, &OrderShipped { order_id }).await?;
//! let unread = user.unread_notifications(&db).await?;
//! # let _ = unread; Ok(()) }
//!
//! // The app's own channel: `message` is what `to_channel` returned.
//! # let _ =
//! App::new().channel("whatsapp", |state, to: Recipient, message| async move {
//!     let phone = to.address("whatsapp").or_else(|| to.user.as_ref()?.get("phone"));
//!     let _ = (state, phone, message); // call your provider's API here
//!     Ok(())
//! })
//! # ;
//! ```

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::User;
use crate::db::{DateTime, Db, now};
use crate::i18n::with_locale;
use crate::mail::Mail;
use crate::queue::{Job, JobContext};
use crate::{AppState, Result};

/// Where a notification goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Channel {
    /// Sent with `state.mailer` (`to_mail`).
    Mail,
    /// Stored in the `notifications` table for an in-app list
    /// (`to_database`); only for users.
    Database,
    /// One of the app's own channels, registered with `App::channel`
    /// (`to_channel`).
    Custom(&'static str),
}

/// Who a notification goes to: a user, addresses for someone without an
/// account, or a user with an extra address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Recipient {
    pub user: Option<User>,
    /// An address per channel, e.g. `mail` → `a@b.c`, `whatsapp` → `+62…`.
    pub routes: BTreeMap<String, String>,
    /// The language to write in; see [`Recipient::locale`].
    #[serde(default)]
    pub language: Option<String>,
}

impl Recipient {
    pub fn for_user(user: &User) -> Self {
        Self {
            user: Some(user.clone()),
            routes: BTreeMap::new(),
            language: None,
        }
    }

    /// Someone without an account: `Recipient::to("mail", "a@b.c")`.
    pub fn to(channel: &str, address: impl Into<String>) -> Self {
        Self::default().and(channel, address)
    }

    /// Adds (or replaces) the address for `channel`.
    pub fn and(mut self, channel: &str, address: impl Into<String>) -> Self {
        self.routes.insert(channel.to_owned(), address.into());
        self
    }

    /// The address for `channel`; for `mail`, the user's email when no
    /// other address was given.
    pub fn address(&self, channel: &str) -> Option<String> {
        self.routes.get(channel).cloned().or_else(|| match channel {
            "mail" => self.user.as_ref().map(|u| u.email.clone()),
            _ => None,
        })
    }

    pub fn email(&self) -> Option<String> {
        self.address("mail")
    }

    /// Writes to this recipient in `locale` (e.g. `"id"`).
    pub fn in_locale(mut self, locale: impl Into<String>) -> Self {
        self.language = Some(locale.into());
        self
    }

    /// The recipient's language: the one given with [`Recipient::in_locale`],
    /// else the user's `locale` column if the `users` table has one.
    /// Notifications build their messages in it (`t()` in mail views,
    /// `state.current_lang()` in code).
    pub fn locale(&self) -> Option<String> {
        self.language.clone().or_else(|| {
            self.user
                .as_ref()?
                .get::<String>("locale")
                .filter(|l| !l.is_empty())
        })
    }
}

impl From<&User> for Recipient {
    fn from(user: &User) -> Self {
        Self::for_user(user)
    }
}

impl From<&Recipient> for Recipient {
    fn from(recipient: &Recipient) -> Self {
        recipient.clone()
    }
}

pub trait Notification: Send + Sync {
    /// Stored with database notifications, e.g. to pick an icon.
    fn kind(&self) -> &'static str;

    fn channels(&self) -> Vec<Channel> {
        vec![Channel::Mail]
    }

    /// The channels for this recipient (e.g. WhatsApp only for users who
    /// turned it on); `channels()` unless overridden.
    fn channels_for(&self, _to: &Recipient) -> Vec<Channel> {
        self.channels()
    }

    fn to_mail(&self, _to: &Recipient, _state: &AppState) -> Result<Mail> {
        Err(anyhow!("notification `{}` has no mail version", self.kind()).into())
    }

    fn to_database(&self, _to: &Recipient) -> Value {
        Value::Null
    }

    /// The message for one of the app's own channels; the channel's handler
    /// gets it.
    fn to_channel(&self, channel: &str, _to: &Recipient) -> Result<Value> {
        Err(anyhow!(
            "notification `{}` has no version for the `{channel}` channel",
            self.kind()
        )
        .into())
    }
}

/// A stored notification.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct DatabaseNotification {
    pub id: i64,
    pub kind: String,
    pub data: Value,
    pub read_at: Option<DateTime>,
    pub created_at: DateTime,
}

pub(crate) type ChannelFn = Arc<
    dyn Fn(
            AppState,
            Recipient,
            Value,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result> + Send>>
        + Send
        + Sync,
>;

pub(crate) fn channel_fn<F, Fut>(send: F) -> ChannelFn
where
    F: Fn(AppState, Recipient, Value) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result> + Send + 'static,
{
    Arc::new(move |state, to, message| Box::pin(send(state, to, message)))
}

/// Delivers a message to one of the app's channels from the queue.
#[derive(Serialize, Deserialize)]
pub(crate) struct SendToChannel {
    channel: String,
    to: Recipient,
    message: Value,
}

impl Job for SendToChannel {
    const NAME: &'static str = "renox.send-to-channel";
    const MAX_ATTEMPTS: u32 = 5;

    async fn handle(self, ctx: JobContext) -> Result {
        let send = ctx.state.channel(&self.channel)?;
        send(ctx.state.clone(), self.to, self.message).await
    }
}

/// The channels a notification is delivered to, in delivery order: the
/// database row first, so a failure there doesn't leave a sent message
/// behind that a retry would send again; mail last.
fn ordered(notification: &impl Notification, to: &Recipient) -> Vec<Channel> {
    let mut channels = notification.channels_for(to);
    channels.sort_by_key(|channel| match channel {
        Channel::Database => 0,
        Channel::Custom(_) => 1,
        _ => 2,
    });
    channels
}

impl AppState {
    fn channel(&self, name: &str) -> Result<ChannelFn> {
        self.channels.get(name).cloned().ok_or_else(|| {
            anyhow!("no `{name}` notification channel: register it with `App::channel`").into()
        })
    }

    async fn store_notification(&self, to: &Recipient, notification: &impl Notification) -> Result {
        // Only users have an in-app list.
        let Some(user) = &to.user else { return Ok(()) };
        crate::db::sql(
            "INSERT INTO notifications (user_id, kind, data, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(user.id)
        .bind(notification.kind())
        .bind(notification.to_database(to).to_string())
        .bind(now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Delivers `notification` to `user` now, on each of its channels.
    pub async fn notify(&self, user: &User, notification: &impl Notification) -> Result {
        self.notify_to(&Recipient::for_user(user), notification)
            .await
    }

    /// Delivers `notification` now to a user or to addresses; see
    /// [`Recipient`]. The database row is written first and mail sent last,
    /// so a failure doesn't leave a message behind that a retry would send
    /// again.
    pub async fn notify_to(&self, to: &Recipient, notification: &impl Notification) -> Result {
        if self.fakes.record_notification(notification.kind(), to) {
            return Ok(());
        }
        let locale = to.locale();
        let locale = locale.as_deref();
        for channel in ordered(notification, to) {
            match channel {
                Channel::Database => self.store_notification(to, notification).await?,
                Channel::Custom(name) => {
                    let send = self.channel(name)?;
                    let message = with_locale(locale, || notification.to_channel(name, to))?;
                    send(self.clone(), to.clone(), message).await?;
                }
                Channel::Mail => {
                    let mail = with_locale(locale, || notification.to_mail(to, self))?;
                    self.mailer.send(mail).await?;
                }
            }
        }
        Ok(())
    }

    /// Like `notify_to`, but mail and the app's channels are sent by queue
    /// workers, each channel as its own job with its own retries. The
    /// messages are built now; the database row is written now.
    pub async fn notify_later(
        &self,
        to: impl Into<Recipient>,
        notification: &impl Notification,
    ) -> Result {
        let to = to.into();
        if self.fakes.record_notification(notification.kind(), &to) {
            return Ok(());
        }
        let locale = to.locale();
        let locale = locale.as_deref();
        for channel in ordered(notification, &to) {
            match channel {
                Channel::Database => self.store_notification(&to, notification).await?,
                Channel::Custom(name) => {
                    self.channel(name)?; // fail now for an unknown channel
                    let message = with_locale(locale, || notification.to_channel(name, &to))?;
                    self.dispatch(SendToChannel {
                        channel: name.to_owned(),
                        to: to.clone(),
                        message,
                    })
                    .await?;
                }
                Channel::Mail => {
                    let mail = with_locale(locale, || notification.to_mail(&to, self))?;
                    self.queue_mail(mail).await?;
                }
            }
        }
        Ok(())
    }
}

fn from_row(row: &crate::db::Row) -> Result<DatabaseNotification> {
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
        let rows = crate::db::sql(
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
        let rows = crate::db::sql(
            "SELECT id, kind, data, read_at, created_at FROM notifications \
             WHERE user_id = ? AND read_at IS NULL ORDER BY id DESC",
        )
        .bind(self.id)
        .fetch_all(db)
        .await?;
        rows.iter().map(from_row).collect()
    }

    pub async fn unread_notification_count(&self, db: &Db) -> Result<i64> {
        Ok(crate::db::sql(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(self.id)
        .scalar(db)
        .await?)
    }

    /// Marks one of the user's notifications read; returns whether it was theirs.
    pub async fn mark_notification_read(&self, db: &Db, id: i64) -> Result<bool> {
        let done = crate::db::sql(
            "UPDATE notifications SET read_at = COALESCE(read_at, ?) WHERE id = ? AND user_id = ?",
        )
        .bind(now())
        .bind(id)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(done > 0)
    }

    pub async fn mark_all_notifications_read(&self, db: &Db) -> Result<u64> {
        let done = crate::db::sql(
            "UPDATE notifications SET read_at = ? WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(now())
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(done)
    }
}

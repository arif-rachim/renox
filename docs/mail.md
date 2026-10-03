# Mail and notifications

Renox sends email with [lettre](https://lettre.rs) over SMTP (rustls, no OpenSSL), renders it
from MiniJinja templates with a text version, and can send it through the queue with retries.
On top of mail, a **notification** is one message to a user (or to an address) delivered on
several channels: mail, a row in the `notifications` table for an in-app list, and your own
channels (WhatsApp, SMS, Slack). The short version is in the [cheat-sheet](../CHEATSHEET.md)
("Mail and notifications"); [examples/jobs](../examples/jobs) queues receipts and reports
(with cc, bcc, reply-to and attachments), [examples/shop](../examples/shop) sends order
notifications with the bell, and [examples/backoffice](../examples/backoffice) tells users in the bell when a payment
arrives or an export is ready.

## Configuration

`MAIL_MAILER` picks the driver:

| Driver | What it does |
|---|---|
| `log` (default) | Writes each mail (to, subject, text) to the server log. Nothing leaves the machine. |
| `smtp` | Sends through `MAIL_HOST`. Use it in production. |
| `memory` | Keeps every mail in memory, for tests (`TestApp` uses it). |

Any other value fails at boot. The SMTP settings:

| Variable | Default | Meaning |
|---|---|---|
| `MAIL_HOST` | `localhost` | The SMTP server. |
| `MAIL_PORT` | the encryption's usual port | e.g. 587 for `starttls`, 465 for `tls`, 1025 for Mailpit. |
| `MAIL_ENCRYPTION` | `starttls` | `tls`, `starttls` or `none` (a local test server only). |
| `MAIL_USERNAME`, `MAIL_PASSWORD` | none | Credentials; used only when both are set. |
| `MAIL_FROM_ADDRESS` | `hello@example.com` | The sender; must be an email address or `smtp` fails at boot. |
| `MAIL_FROM_NAME` | `APP_NAME` | The sender's name. |
| `MAIL_TIMEOUT` | 10 | Seconds one send may take, from connecting to the last reply. |

Set `APP_URL` to the public address, so links in mails point to the right host.

### More mailers, and failover

`App::mailer(name, settings)` adds a mailer with a name: a newsletter provider, an account
kept apart from receipts, or a second SMTP provider. `MailConfig::from_env(config, "BACKUP")`
reads `BACKUP_MAILER` (the app's `MAIL_MAILER` when unset, so `log` while developing and
`memory` in tests), `BACKUP_HOST`, `_PORT`, `_USERNAME`, `_PASSWORD`, `_ENCRYPTION`,
`_TIMEOUT` and `BACKUP_FROM_ADDRESS`/`_FROM_NAME` (else `MAIL_FROM_*`).

```rust
use renox::prelude::*;
use renox::mail::{Mail, MailConfig};

fn app() -> App {
    App::new()
        .mailer("newsletter", |config| MailConfig::from_env(config, "NEWSLETTER"))
        .mailer("backup", |config| MailConfig::from_env(config, "BACKUP"))
}

async fn send(state: AppState) -> Result {
    let news = Mail::new("ann@example.com", "October", "What's new.");
    state.mailer_named("newsletter")?.send(news.clone()).await?; // now
    state.queue_mail_via("newsletter", news).await?;             // through the queue
    Ok(())
}
```

`MAIL_FAILOVER=backup` (names from `App::mailer`, comma-separated, tried in order) hands a
mail on when the default mailer fails, e.g. the main SMTP provider is down: the mail is sent
by the next one that works, with a warning in the log, and counts as the app's (it's in
`/_renox/mail` and `app.sent_mail()`). A mail that can't be sent at all, such as one with a
bad address, isn't handed on. A name that no mailer has fails at boot.

## Sending a mail

`Mail::new(to, subject, text)` is a plain-text mail; the builder adds the rest. Addresses are
`a@b.c` or `Name <a@b.c>`.

```rust
use renox::prelude::*;
use renox::mail::Mail;

async fn invoice(State(state): State<AppState>) -> Result {
    let pdf: Vec<u8> = b"%PDF-1.7 ...".to_vec();
    let mail = Mail::new("ben@example.com", "Invoice INV-001", "Your invoice is attached.")
        .html("<p>Your invoice is attached.</p>") // text is still sent as the plain part
        .also_to("sarah@example.com")
        .cc("sales@example.com")
        .bcc("archive@example.com")
        .reply_to("Coffee Shop <hello@shop.example>")
        .from("Coffee Shop Billing <billing@shop.example>") // instead of MAIL_FROM_*
        .attach("INV-001.pdf", "application/pdf", pdf);
    state.mailer.send(mail).await // now, in this request
}
```

- `state.mailer.send(mail)` sends now; the request waits for the SMTP server (up to
  `MAIL_TIMEOUT`) and gets its error.
- `state.queue_mail(mail)` queues it (job `renox.send-mail`) and returns the job id. A worker
  sends it with up to **five attempts**; after the last one it lands in `failed_jobs`
  (`queue:failed`, `queue:retry`). Prefer it in requests, so a slow mail server doesn't slow
  the page. See [queue.md](queue.md).
- An invalid address, a mail with no recipient or a bad attachment content type is a
  permanent error: a queued mail fails at once instead of retrying.

## Mail views

`state.mail_view(to, subject, view, ctx)` renders `resources/views/{view}.html` as the HTML
part and `{view}.txt` as the text part. Without a `.txt` file the text is made from the HTML
(tags dropped, links written as `text (url)`). It returns a `Mail`, so the builder methods
above still apply.

```rust
use renox::prelude::*;

async fn receipt(state: &AppState, order_id: i64, pdf: Vec<u8>) -> Result {
    let mail = state
        .mail_view(
            "ben@example.com",
            format!("Receipt for order #{order_id}"),
            "mail/receipt", // mail/receipt.html (+ mail/receipt.txt)
            context! { order_id, total => 75_000 },
        )?
        .bcc("archive@example.com")
        .attach("receipt.pdf", "application/pdf", pdf);
    state.queue_mail(mail).await?;
    Ok(())
}
```

Besides `ctx`, mail templates see `app.name`, `app.url`, `app.locale` and `t(key, …)`. Renox
ships a layout and components, styled inline for mail clients:

```html
{# resources/views/mail/receipt.html #}
{% extends "renox/mail/layout.html" %}
{% from "renox/mail/components.html" import button, panel, table, divider %}
{% block content %}
<p>{{ t('mail.thanks') }}</p>
{% call panel() %}Order #{{ order_id }} ships tomorrow.{% endcall %}
{{ table([["Coffee", "Rp 18.000"]], head=["Item", "Price"], total=["Total", "Rp 18.000"]) }}
{{ divider() }}
{{ button(app.url ~ "/orders/" ~ order_id, "View your order") }}
{% endblock %}
```

- `renox/mail/layout.html` has the blocks `title`, `content` and `footer` (`© {{ app.name }}`
  by default).
- `renox/mail/components.html`: `button(url, label)` (a button with the URL written under it),
  `panel()` (used with `{% call %}`), `table(rows, head=none, total=none)` (the last column
  right-aligned) and `divider()`. `renox/mail/button.html` has the same `button` alone.
- To restyle every mail, put your own `resources/views/renox/mail/layout.html` (or
  `components.html`) in the app: app views override built-ins by file name. The same works for
  the password-reset and verification mails (`renox/mail/auth/reset-password.html` / `.txt`,
  `verify-email.html` / `.txt`); their texts come from the `renox.auth.*` translations (e.g.
  `renox.auth.mail_reset_subject`).

`rnx make:mail order_shipped` writes `resources/views/mail/order_shipped.html` (on the layout,
with a button) and `order_shipped.txt`.

## Localized mail

`mail_view` renders in the current language: a notification's recipient's (below), else the
request's, else `APP_LOCALE`. Outside a request (a job, a command) pick one:

```rust
use renox::prelude::*;

async fn welcome(state: &AppState, user: &User) -> Result {
    // This mail only, in Spanish (from lang/es.json): t() in the view and the subject use "es".
    let subject = state.lang("es").t("mail.welcome", &[("name", &user.name)]);
    let mail = state.mail_view_in("es", &user.email, subject, "mail/welcome", context! {})?;
    state.queue_mail(mail).await?;

    // Or for the rest of this job or task:
    renox::i18n::set_current_locale("es");
    let subject = state.current_lang().t("mail.welcome", &[("name", &user.name)]);
    let _ = subject;
    Ok(())
}
```

Translations live in `resources/lang/<locale>.json` (see the cheat-sheet, "Cache, session,
uploads, translations").

## The preview page

With `APP_DEBUG` on, `/_renox/mail` lists the last 50 mails sent (by any driver), and
`/_renox/mail/{id}` shows one: recipients, cc/bcc, reply-to, from, attachments, the HTML in a
sandboxed frame and the text. With the default `log` driver, this is where the password-reset
link is while developing. The route doesn't exist with debug off.

## Notifications

A notification implements `Notification`: a `kind` (stored with database rows, used in
tests), its channels, and one version of the message per channel.

| Channel | Method | Delivered by |
|---|---|---|
| `Channel::Mail` (the default) | `to_mail(&self, to, state) -> Result<Mail>` | `state.mailer`, to `to.email()` |
| `Channel::Database` | `to_database(&self, to) -> Value` | a row in `notifications` (users only) |
| `Channel::Custom("whatsapp")` | `to_channel(&self, channel, to) -> Result<Value>` | the handler registered with `App::channel` |

```rust
use renox::prelude::*;
use renox::auth::{Channel, DatabaseMessage, Notification, Recipient};
use renox::mail::Mail;

struct OrderShipped { order_id: i64 }

impl Notification for OrderShipped {
    fn kind(&self) -> &'static str { "order-shipped" }

    // Per recipient (Laravel's `via`); only Channel::Mail by default.
    fn channels(&self, to: &Recipient) -> Vec<Channel> {
        let mut channels = vec![Channel::Mail, Channel::Database];
        if to.address("whatsapp").is_some() {
            channels.push(Channel::Custom("whatsapp"));
        }
        channels
    }

    fn to_mail(&self, to: &Recipient, state: &AppState) -> Result<Mail> {
        let subject = state.current_lang().t("mail.shipped", &[("id", &self.order_id)]);
        state.mail_view(to.email().unwrap_or_default(), subject, "mail/order_shipped",
            context! { order_id => self.order_id })
    }

    // What the in-app list and the kit's bell show; see "Database notifications".
    fn to_database(&self, _: &Recipient, _: &AppState) -> Result<renox::serde_json::Value> {
        Ok(DatabaseMessage::success(format!("Order #{} shipped", self.order_id))
            .url(format!("/orders/{}", self.order_id))
            .with("order_id", self.order_id)
            .into())
    }

    fn to_channel(&self, _channel: &str, _: &Recipient, _: &AppState) -> Result<renox::serde_json::Value> {
        Ok(json!({ "text": format!("Order #{} shipped", self.order_id) }))
    }
}

fn app() -> App {
    App::new()
        .module(Auth::new())
        // Your own channel: `message` is what to_channel built.
        .channel("whatsapp", |state, to: Recipient, message| async move {
            let phone = to.address("whatsapp").or_else(|| to.user.as_ref()?.get("phone"));
            let _ = (state, phone, message); // call your provider's API here
            Ok(())
        })
}

async fn ship(state: &AppState, user: &User) -> Result {
    let shipped = OrderShipped { order_id: 7 };
    state.notify(user, &shipped).await?;       // now, every channel
    state.notify_later(user, &shipped).await?; // mail and custom channels through the queue
    // Someone without an account (no database row for them):
    let guest = Recipient::to("mail", "guest@example.com")
        .and("whatsapp", "+6281234567890")
        .in_locale("es");
    state.notify(&guest, &shipped).await
}
```

- `state.notify(&user, &n)` delivers now; `state.notify(&recipient, &n)` does the same for
  a `Recipient`. The database row is written first and mail sent last, so a failure doesn't
  leave a sent message behind that a retry would send again.
- `state.notify_later(to, &n)` builds every message now and writes the database row now, then
  queues each mail and each custom-channel message as its own job (five attempts each, then
  `failed_jobs`). It takes a `&User` or a `&Recipient`. An unregistered custom channel fails
  at once.
- A `Recipient` is `Recipient::for_user(&user)` (or `(&user).into()`), `Recipient::to(channel,
  address)` for someone without an account, plus `.and(channel, address)` for more addresses.
  `to.address(channel)` returns the address for a channel; for `mail` it falls back to the
  user's email, which `to.email()` returns.
- `Notification` is not `Serialize`: the queue stores the built messages, never the
  notification itself.

`rnx make:notification OrderShipped --module orders` writes a notification with mail and
database versions into the module.

## Localized notifications

Each message is built in the recipient's language: `Recipient::in_locale("es")`, else the
user's `locale` column when the `users` table has one, else the current language. While
`to_mail`, `to_database` and `to_channel` run, `t()` in mail views and `state.current_lang()` in code use it,
so one notification sent to many users speaks each one's language.

## Database notifications

`Channel::Database` rows are a user's in-app list. `DatabaseNotification` has `id`, `kind`,
`data` (what `to_database` returned), `read_at` (`None` while unread) and `created_at`.

`to_database` may return any JSON, but a `DatabaseMessage` is what Renox's own list and the UI
kit's bell know how to show: a status (`success`, `info`, `warning`, `error`: its icon), a
title, a `body`, a `url` (where opening it goes) and links (`link(label, url)`, or
`action(ToastAction::link(…))`). `.with(key, value)` keeps the app's own keys next to them
(`notification.data.order_id`), and `notification.message()` reads the message back.

### The bell

`Auth::new().notifications()` turns on a ready-made list, like Filament's database
notifications:

```rust
use renox::prelude::*;

fn app() -> App {
    App::new().module(Auth::new().notifications())
}
```

and in the layout's navigation bar (the kit's `navbar`, see [ui.md](ui.md)):

```html
{% from "renox/ui.html" import navbar, notification_bell %}
{% call navbar(app.name, href=route('home')) %}
  <span class="rx-spacer"></span>
  {{ notification_bell(unread_notifications) }} {# nothing for guests #}
{% endcall %}
```

- `unread_notifications`, the logged-in user's unread count, is in every view (0 for guests);
  the bell's badge shows it ("99+" past 99) and its label reads "Notifications, 3 unread".
- A click opens a panel with the latest 20: each with its icon, title, body, time ("5 minutes
  ago") and links, a dot while unread, buttons to mark it read or unread and to delete it,
  and "Mark all as read" and "Clear all" on top. Opening one (its title, when it has a `url`)
  marks it read and follows the link. Esc or a click outside closes the panel.
- While a page is open, new notifications arrive by themselves: the page keeps a
  Server-Sent Events stream (`/notifications/stream`), and each new one shows as a toast with
  an "Open" link and updates the badge, as do reads in another tab.
- Without JavaScript the bell is a link to `/notifications`, the same list as a page (with
  "Older" pages), in the app's `layouts/app.html` (else Renox's sign-in layout). Replace
  `renox/notifications.html` to change it; its `panel` block is what the bell shows.

The routes, all for logged-in users: `notifications.index` (`GET /notifications`, the page;
with `HX-Request` the panel alone), `notifications.stream`, `notifications.read`,
`notifications.unread`, `notifications.open` (`POST /notifications/{id}/…`),
`notifications.destroy` (`DELETE /notifications/{id}`), `notifications.read_all` (`POST
/notifications/read-all`) and `notifications.clear` (`DELETE /notifications`). The kit texts
are under `ui.notifications.*`.

How new ones arrive: a stream sends `count` (the unread count) first and whenever it changes,
and `notification` (the new one as JSON) when one is stored. A notification stored in the same
process (a request, or the queue workers inside `serve`) wakes the user's streams at once;
streams also look at the table every 15 seconds, so one stored by another server or by a
separate `queue:work` arrives within that time. A stream ends after five minutes and the
browser opens a new one, so a session that logged out or was revoked doesn't keep one open,
and every stream ends when the server shuts down. Behind a proxy, keep SSE unbuffered (see
[operations.md](operations.md)).

### Your own list

```rust
use renox::prelude::*;

async fn inbox(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let notifications = user.notifications(&db, 20).await?; // newest first, at most 20
    let unread = user.unread_notification_count(&db).await?;
    let _all_unread = user.unread_notifications(&db).await?; // every unread one, newest first
    user.mark_all_notifications_read(&db).await?;           // returns how many
    Ok(view("inbox.html", context! { notifications, unread }))
}

async fn read_one(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result {
    // false when the notification isn't this user's
    if !user.mark_notification_read(&db, id).await? {
        return Err(Error::NotFound);
    }
    Ok(())
}
```

Read notifications stay until you prune them: `my-app notifications:prune --days 30` (from the
`Auth` module; 30 days by default) deletes those read more than that long ago, and unread ones
stay however old. Schedule it, or call `renox::auth::prune_read_notifications(&db, age)` from
your own task:

```rust
use renox::prelude::*;
use std::time::Duration;

fn app() -> App {
    App::new().module(Auth::new()).schedule(|s| {
        s.daily_at("04:00", "prune-notifications", |state| async move {
            let month = Duration::from_secs(30 * 24 * 60 * 60);
            renox::auth::prune_read_notifications(&state.db, month).await?;
            Ok(())
        });
    })
}
```

Deleting a user's account deletes their notifications too.

## Testing

`TestApp` uses the `memory` mailer: `app.sent_mail()` (or `app.mailer().sent()`) returns every
`Mail` sent, and `app.assert_mail_sent(to, subject)` checks one (`subject` is a part of the
subject). Queued mail is sent when the test runs the queue (`app.run_jobs().await`).
`app.fake_notifications()` records notifications instead of delivering them (no mail, no
rows, no channel calls), whether sent with `notify` or `notify_later`.

```rust
use renox::prelude::*;
use renox::auth::{Notification, Recipient};
use renox::testing::TestApp;

struct Hello;

impl Notification for Hello {
    fn kind(&self) -> &'static str { "hello" } // mail only, by default
}

#[renox::test]
async fn mail_and_notifications() {
    let app = TestApp::new(App::new().module(Auth::new())).await;
    let state = app.state();

    let mail = state.mail_view("ben@example.com", "Welcome", "renox/mail/layout", context! {}).unwrap();
    state.queue_mail(mail).await.unwrap();
    app.run_jobs().await;
    app.assert_mail_sent("ben@example.com", "Welcome");
    assert!(app.sent_mail()[0].is_for("ben@example.com"));

    app.fake_notifications();
    let guest = Recipient::to("mail", "guest@example.com");
    state.notify(&guest, &Hello).await.unwrap();
    app.assert_notified_to("guest@example.com", "hello");
    // also app.assert_notified(&user, "hello"), app.notifications(), app.assert_nothing_notified()
}
```

More in [testing.md](testing.md) ("Jobs, events, notifications, mail, HTTP").

## In production

- Use `MAIL_MAILER=smtp` and send from requests with `queue_mail` / `notify_later`: if the
  SMTP server is down or silent, a direct `mailer.send` fails within `MAIL_TIMEOUT`, while
  queued mail and notifications get five attempts and then wait in `failed_jobs` for
  `queue:retry` (see [operations.md](operations.md), the failure table).
- The built-in password-reset and verification mails are sent directly, in the request; a
  second provider in `MAIL_FAILOVER` keeps them going when the first is down.
- Prune read notifications (above).

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `MAIL_MAILER=smtp\|log\|array` | `MAIL_MAILER=smtp\|log\|memory` |
| `Mailable` class + Blade view | `state.mail_view(to, subject, "mail/x", ctx)` (`.html` + `.txt`) |
| `Mail::to()->cc()->bcc()`, `replyTo`, `from`, `attach` | `Mail` builder: `also_to`, `cc`, `bcc`, `reply_to`, `from`, `attach` |
| `Mail::send` / `Mail::queue` | `state.mailer.send(mail)` / `state.queue_mail(mail)` |
| `Mail::mailer('postmark')`, the `failover` transport | `App::mailer(name, …)` + `state.mailer_named(name)` / `queue_mail_via`, `MAIL_FAILOVER` |
| Markdown mail components (`x-mail::button`, `panel`, `table`) | `renox/mail/components.html`: `button`, `panel`, `table`, `divider` |
| `Mail::to($u)->locale('es')` | `state.mail_view_in("es", …)` |
| `php artisan make:mail` | `rnx make:mail` (templates only) |
| Mail preview packages, Mailpit | `/_renox/mail` (debug) |
| `Notification` with `via()` | `Notification` with `channels(to)` |
| `toMail`, `toDatabase` / `toArray`, custom channels | `to_mail`, `to_database`, `to_channel` + `App::channel` |
| `$user->notify()`, `ShouldQueue` | `state.notify(&user, &n)`, `state.notify_later(&user, &n)` |
| `Notification::route('mail', …)` (on-demand) | `Recipient::to("mail", …).and(…)` + `state.notify` |
| `HasLocalePreference` | `Recipient::in_locale` or a `users.locale` column |
| `$user->notifications`, `unreadNotifications`, `markAsRead`, `markAsUnread` | `user.notifications(&db, n)`, `notifications_before`, `unread_notifications`, `mark_notification_read`, `mark_notification_unread`, `mark_all_notifications_read`, `delete_notification`, `delete_notifications` |
| Filament's `Notification::make()->title()->body()->sendToDatabase($user)` | `DatabaseMessage::success(title).body(…).url(…)` from `to_database` |
| Filament's database notifications modal, polling or Echo | `Auth::new().notifications()` + `notification_bell(unread_notifications)`, Server-Sent Events |
| `model:prune` on notifications | `notifications:prune --days 30` |
| `Mail::fake()`, `Notification::fake()`, `assertSentTo` | memory mailer + `sent_mail()` / `assert_mail_sent`, `fake_notifications()` + `assert_notified` / `assert_notified_to` |

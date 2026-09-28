//! Sending email: SMTP in production, the log or memory in development and
//! tests, HTML templates with a text version, and a preview page.
//!
//! ```
//! # use renox::prelude::*;
//! # async fn demo(state: AppState, order: renox::serde_json::Value) -> Result {
//! let mail = state.mail_view("budi@example.com", "Struk pesanan", "mail/receipt", context! { order })?;
//! state.mailer.send(mail.clone()).await?;   // now
//! state.queue_mail(mail).await?;            // through the queue, with retries
//! # Ok(()) }
//! ```
//!
//! `MAIL_MAILER` picks the driver: `smtp` (`MAIL_HOST`, `MAIL_PORT`,
//! `MAIL_USERNAME`, `MAIL_PASSWORD`, `MAIL_ENCRYPTION` = `tls`, `starttls` or
//! `none`), `log` (the default: messages go to the server log) or `memory`
//! (for tests: `kernel.mailer().sent()`). With `APP_DEBUG` on, the last 50
//! messages are listed at `/_renox/mail`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use axum::Router;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::get;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};

use crate::db::{DateTime, now};
use crate::queue::{Job, JobContext};
use crate::{AppState, Config, Error, Result, context};

/// How many sent messages the preview page keeps.
const OUTBOX: usize = 50;

/// One email message. `html` is optional; `text` is always sent.
///
/// ```
/// # use renox::mail::Mail;
/// let pdf: Vec<u8> = b"%PDF-1.7 ...".to_vec();
/// let mail = Mail::new("budi@example.com", "Invoice INV-001", "Your invoice is attached.")
///     .also_to("finance@example.com")
///     .cc("sales@example.com")
///     .bcc("archive@example.com")
///     .reply_to("Toko Kopi <halo@toko.id>")
///     .from("Toko Kopi Billing <billing@toko.id>") // instead of MAIL_FROM_*
///     .attach("INV-001.pdf", "application/pdf", pdf);
/// ```
///
/// Addresses are `a@b.c` or `Name <a@b.c>`; an invalid one fails the send
/// (and a queued mail isn't retried for it).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct Mail {
    pub to: Vec<String>,
    pub subject: String,
    pub text: String,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    /// Sender instead of `MAIL_FROM_ADDRESS` / `MAIL_FROM_NAME`.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// A file sent with a mail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct Attachment {
    pub filename: String,
    /// e.g. `application/pdf`.
    pub content_type: String,
    /// Stored as base64 when the mail is queued.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

mod base64_bytes {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(data))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

impl Mail {
    pub fn new(to: impl Into<String>, subject: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            to: vec![to.into()],
            subject: subject.into(),
            text: text.into(),
            html: None,
            cc: Vec::new(),
            bcc: Vec::new(),
            reply_to: None,
            from: None,
            attachments: Vec::new(),
        }
    }

    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }

    /// Another recipient.
    pub fn also_to(mut self, address: impl Into<String>) -> Self {
        self.to.push(address.into());
        self
    }

    pub fn cc(mut self, address: impl Into<String>) -> Self {
        self.cc.push(address.into());
        self
    }

    pub fn bcc(mut self, address: impl Into<String>) -> Self {
        self.bcc.push(address.into());
        self
    }

    pub fn reply_to(mut self, address: impl Into<String>) -> Self {
        self.reply_to = Some(address.into());
        self
    }

    /// Sends as `address` instead of `MAIL_FROM_ADDRESS` / `MAIL_FROM_NAME`.
    pub fn from(mut self, address: impl Into<String>) -> Self {
        self.from = Some(address.into());
        self
    }

    pub fn attach(
        mut self,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.attachments.push(Attachment {
            filename: filename.into(),
            content_type: content_type.into(),
            data: data.into(),
        });
        self
    }

    /// Whether `address` is among `to`, `cc` or `bcc`.
    pub fn is_for(&self, address: &str) -> bool {
        self.to
            .iter()
            .chain(&self.cc)
            .chain(&self.bcc)
            .any(|a| a == address || a.ends_with(&format!("<{address}>")))
    }
}

/// Mail settings, from `MAIL_*`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MailConfig {
    /// `smtp`, `log` or `memory`.
    pub mailer: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    /// `tls` (usually port 465), `starttls` (587) or `none` (e.g. Mailpit on 1025).
    pub encryption: String,
    pub from_address: String,
    pub from_name: Option<String>,
    /// How long sending one mail over SMTP may take, from `MAIL_TIMEOUT` in
    /// seconds (default 10).
    pub timeout: std::time::Duration,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            mailer: "log".into(),
            host: "localhost".into(),
            port: None,
            username: None,
            password: None,
            encryption: "starttls".into(),
            from_address: "hello@example.com".into(),
            timeout: std::time::Duration::from_secs(10),
            from_name: None,
        }
    }
}

#[derive(Clone)]
enum Driver {
    Log,
    Memory,
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
        timeout: std::time::Duration,
    },
}

#[derive(Debug, Clone, Serialize)]
struct Sent {
    id: u64,
    at: DateTime,
    mail: Mail,
}

/// Sends mail with the configured driver and remembers recent messages for
/// the preview page and tests.
#[derive(Clone)]
pub struct Mailer {
    driver: Driver,
    outbox: Arc<Mutex<(u64, VecDeque<Sent>)>>,
    /// The memory driver keeps everything; others keep the last `OUTBOX` in debug.
    keep: Option<usize>,
}

impl Mailer {
    pub(crate) fn from_config(config: &Config) -> anyhow::Result<Self> {
        let mail = &config.mail;
        let (driver, keep) = match mail.mailer.as_str() {
            "log" => (Driver::Log, config.debug.then_some(OUTBOX)),
            "memory" => (Driver::Memory, Some(usize::MAX)),
            "smtp" => (smtp(config)?, config.debug.then_some(OUTBOX)),
            other => bail!("MAIL_MAILER must be smtp, log or memory, got `{other}`"),
        };
        Ok(Self {
            driver,
            outbox: Arc::default(),
            keep,
        })
    }

    pub async fn send(&self, mail: Mail) -> Result {
        match &self.driver {
            Driver::Log => tracing::info!(
                "mail (log driver)\nTo: {}\nSubject: {}\n\n{}\n",
                mail.to.join(", "),
                mail.subject,
                mail.text
            ),
            Driver::Memory => {}
            Driver::Smtp {
                transport,
                from,
                timeout,
            } => {
                let message = message(from, &mail)?;
                // lettre's own timeout doesn't cover a server that accepts the
                // connection and then says nothing.
                tokio::time::timeout(*timeout, transport.send(message))
                    .await
                    .map_err(|_| anyhow::anyhow!("no answer from the SMTP server in {timeout:?}"))
                    .and_then(|sent| sent.map_err(anyhow::Error::from))
                    .with_context(|| format!("sending mail to {}", mail.to.join(", ")))?;
            }
        }
        self.remember(mail);
        Ok(())
    }

    fn remember(&self, mail: Mail) {
        let Some(keep) = self.keep else { return };
        let mut outbox = self.outbox.lock().unwrap_or_else(|e| e.into_inner());
        outbox.0 += 1;
        let id = outbox.0;
        outbox.1.push_back(Sent {
            id,
            at: now(),
            mail,
        });
        while outbox.1.len() > keep {
            outbox.1.pop_front();
        }
    }

    /// Messages sent so far that the mailer kept: all of them with the
    /// `memory` driver, the last 50 with `APP_DEBUG` on.
    pub fn sent(&self) -> Vec<Mail> {
        let outbox = self.outbox.lock().unwrap_or_else(|e| e.into_inner());
        outbox.1.iter().map(|s| s.mail.clone()).collect()
    }

    fn kept(&self) -> Vec<Sent> {
        self.outbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .1
            .iter()
            .cloned()
            .collect()
    }
}

fn smtp(config: &Config) -> anyhow::Result<Driver> {
    let mail = &config.mail;
    let mut builder = match mail.encryption.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&mail.host)?,
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&mail.host)?,
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&mail.host),
        other => bail!("MAIL_ENCRYPTION must be tls, starttls or none, got `{other}`"),
    };
    if let Some(port) = mail.port {
        builder = builder.port(port);
    }
    builder = builder.timeout(Some(mail.timeout));
    if let (Some(user), Some(password)) = (&mail.username, &mail.password) {
        builder = builder.credentials(Credentials::new(user.clone(), password.clone()));
    }
    let name = mail
        .from_name
        .clone()
        .unwrap_or_else(|| config.name.clone());
    let address = mail.from_address.parse().with_context(|| {
        format!(
            "MAIL_FROM_ADDRESS `{}` is not an email address",
            mail.from_address
        )
    })?;
    Ok(Driver::Smtp {
        transport: builder.build(),
        from: Mailbox::new(Some(name), address),
        timeout: mail.timeout,
    })
}

fn mailbox(address: &str) -> Result<Mailbox> {
    address.trim().parse().map_err(|err| {
        Error::permanent(
            anyhow::Error::new(err).context(format!("`{address}` is not an email address")),
        )
    })
}

fn message(from: &Mailbox, mail: &Mail) -> Result<Message> {
    use lettre::message::{Attachment as Part, SinglePart};

    if mail.to.is_empty() {
        return Err(Error::permanent(anyhow::anyhow!(
            "the mail has no recipient"
        )));
    }
    let from = match &mail.from {
        Some(address) => mailbox(address)?,
        None => from.clone(),
    };
    let mut builder = Message::builder().from(from).subject(mail.subject.clone());
    for address in &mail.to {
        builder = builder.to(mailbox(address)?);
    }
    for address in &mail.cc {
        builder = builder.cc(mailbox(address)?);
    }
    for address in &mail.bcc {
        builder = builder.bcc(mailbox(address)?);
    }
    if let Some(address) = &mail.reply_to {
        builder = builder.reply_to(mailbox(address)?);
    }
    let body = match &mail.html {
        Some(html) => MultiPart::alternative_plain_html(mail.text.clone(), html.clone()),
        None => MultiPart::mixed().singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(mail.text.clone()),
        ),
    };
    let message = if mail.attachments.is_empty() {
        match &mail.html {
            Some(_) => builder.multipart(body),
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(mail.text.clone()),
        }
    } else {
        let mut mixed = MultiPart::mixed().multipart(body);
        for file in &mail.attachments {
            let content_type = ContentType::parse(&file.content_type).map_err(|err| {
                Error::permanent(anyhow::anyhow!(
                    "attachment `{}`: `{}` is not a content type ({err})",
                    file.filename,
                    file.content_type
                ))
            })?;
            mixed = mixed
                .singlepart(Part::new(file.filename.clone()).body(file.data.clone(), content_type));
        }
        builder.multipart(mixed)
    };
    Ok(message.map_err(anyhow::Error::from)?)
}

/// Sends a mail from the queue; see `AppState::queue_mail`.
#[derive(Serialize, Deserialize)]
pub(crate) struct SendMail(pub Mail);

impl Job for SendMail {
    const NAME: &'static str = "renox.send-mail";
    const MAX_ATTEMPTS: u32 = 5;

    async fn handle(self, ctx: JobContext) -> Result {
        ctx.state.mailer.send(self.0).await
    }
}

impl AppState {
    /// Renders `{view}.html` into a mail, with `{view}.txt` as the text
    /// version when it exists (otherwise text made from the HTML).
    /// Templates see `app` (`app.locale` too) and `t(key, …)` in the
    /// [`current_locale`](crate::i18n::current_locale) besides `ctx`, can
    /// extend `renox/mail/layout.html` and import
    /// `renox/mail/components.html` (`button`, `panel`, `table`).
    pub fn mail_view(
        &self,
        to: impl Into<String>,
        subject: impl Into<String>,
        view: &str,
        ctx: impl Serialize,
    ) -> Result<Mail> {
        let locale = crate::i18n::current_locale(self);
        let ctx = minijinja::value::merge_maps([
            minijinja::Value::from_serialize(&ctx),
            context! {
                app => context! { name => self.config.name, url => self.config.url, locale => locale },
                t => crate::view::translate_function(self, &locale),
            },
        ]);
        let html = self.views.render(&format!("{view}.html"), &ctx)?;
        let text = match self.views.render(&format!("{view}.txt"), &ctx) {
            Ok(text) => text,
            Err(err) if is_not_found(&err) => html_to_text(&html),
            Err(err) => return Err(err.into()),
        };
        Ok(Mail::new(to, subject, text.trim().to_owned()).html(html))
    }

    /// `mail_view` in `locale` (the recipient's language).
    pub fn mail_view_in(
        &self,
        locale: &str,
        to: impl Into<String>,
        subject: impl Into<String>,
        view: &str,
        ctx: impl Serialize,
    ) -> Result<Mail> {
        crate::i18n::with_locale(Some(locale), || self.mail_view(to, subject, view, ctx))
    }

    /// Sends `mail` from a queue worker, retrying up to five times.
    pub async fn queue_mail(&self, mail: Mail) -> Result<i64> {
        self.dispatch(SendMail(mail)).await
    }
}

fn is_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<minijinja::Error>()
        .is_some_and(|e| e.kind() == minijinja::ErrorKind::TemplateNotFound)
}

/// A readable text version of an HTML mail: tags dropped, blocks separated
/// by blank lines, table cells by spaces, links written as `text (url)`.
pub(crate) fn html_to_text(html: &str) -> String {
    let body = html
        .find("<body")
        .and_then(|start| html[start..].find('>').map(|end| start + end + 1))
        .map_or(html, |start| &html[start..]);
    let mut out = String::new();
    let mut href: Option<String> = None;
    let mut rest = body;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let tag = &rest[open + 1..open + close];
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match name.as_str() {
            "a" if !tag.starts_with('/') => {
                href = tag
                    .split("href=\"")
                    .nth(1)
                    .and_then(|v| v.split('"').next())
                    .map(str::to_owned);
            }
            "a" => {
                if let Some(url) = href.take() {
                    out.push_str(&format!(" ({url})"));
                }
            }
            "style" | "head" | "title" if !tag.starts_with('/') => {
                let end = format!("</{name}");
                if let Some(skip) = rest[open..].to_ascii_lowercase().find(&end) {
                    rest = &rest[open + skip..];
                    continue;
                }
            }
            "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "table" => out.push('\n'),
            "td" | "th" => out.push(' '),
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&#x2f;", "/")
        .replace("&amp;", "&");
    let mut text = String::new();
    let mut blank = false;
    for line in decoded
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
    {
        if line.is_empty() {
            blank = !text.is_empty();
            continue;
        }
        if blank {
            text.push('\n');
            blank = false;
        }
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// `/_renox/mail`: recently sent messages, in development only.
pub(crate) fn preview_router() -> Router<AppState> {
    Router::new()
        .route("/_renox/mail", get(list))
        .route("/_renox/mail/{id}", get(show))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "<style>body{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#222}\
table{width:100%;border-collapse:collapse}td{padding:.5rem;border-bottom:1px solid #eee}\
iframe{width:100%;height:32rem;border:1px solid #ddd;border-radius:.5rem}pre{white-space:pre-wrap;background:#f6f6f6;padding:1rem}</style>";

async fn list(State(state): State<AppState>) -> Html<String> {
    let rows: String = state
        .mailer
        .kept()
        .iter()
        .rev()
        .map(|s| {
            format!(
                "<tr><td>{}</td><td><a href=\"/_renox/mail/{}\">{}</a></td><td>{}</td></tr>",
                s.at.format("%H:%M:%S"),
                s.id,
                escape(&s.mail.subject),
                escape(&s.mail.to.join(", "))
            )
        })
        .collect();
    let rows = if rows.is_empty() {
        "<tr><td>No mail sent yet.</td></tr>".to_owned()
    } else {
        rows
    };
    Html(format!(
        "<!doctype html><meta charset=utf-8><title>Mail · Renox</title>{STYLE}<h1>Sent mail</h1><table>{rows}</table>"
    ))
}

async fn show(State(state): State<AppState>, Path(id): Path<u64>) -> Result<Html<String>> {
    let sent = state
        .mailer
        .kept()
        .into_iter()
        .find(|s| s.id == id)
        .ok_or(Error::NotFound)?;
    let html = sent.mail.html.as_deref().map_or(String::new(), |html| {
        format!(
            "<h2>HTML</h2><iframe sandbox srcdoc=\"{}\"></iframe>",
            escape(html)
        )
    });
    let mut details = format!("<p>To: {}</p>", escape(&sent.mail.to.join(", ")));
    for (label, list) in [("Cc", &sent.mail.cc), ("Bcc", &sent.mail.bcc)] {
        if !list.is_empty() {
            details.push_str(&format!("<p>{label}: {}</p>", escape(&list.join(", "))));
        }
    }
    for (label, value) in [("Reply-To", &sent.mail.reply_to), ("From", &sent.mail.from)] {
        if let Some(value) = value {
            details.push_str(&format!("<p>{label}: {}</p>", escape(value)));
        }
    }
    if !sent.mail.attachments.is_empty() {
        let files: Vec<String> = sent
            .mail
            .attachments
            .iter()
            .map(|a| {
                format!(
                    "{} ({}, {} bytes)",
                    escape(&a.filename),
                    escape(&a.content_type),
                    a.data.len()
                )
            })
            .collect();
        details.push_str(&format!("<p>Attachments: {}</p>", files.join(", ")));
    }
    Ok(Html(format!(
        "<!doctype html><meta charset=utf-8><title>{subject} · Renox</title>{STYLE}\
         <p><a href=\"/_renox/mail\">&larr; All mail</a></p><h1>{subject}</h1>{details}{html}\
         <h2>Text</h2><pre>{text}</pre>",
        subject = escape(&sent.mail.subject),
        text = escape(&sent.mail.text),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_readable_text() {
        let html = r#"<html><head><style>p{color:red}</style></head><body>
            <h1>Halo &amp; selamat</h1><p>Klik <a href="https://x.id/a?b=1&amp;c=2">di sini</a>.</p>
            <table><tr><td>Total</td><td>Rp 10.000</td></tr></table></body></html>"#;
        assert_eq!(
            html_to_text(html),
            "Halo & selamat\n\nKlik di sini (https://x.id/a?b=1&c=2).\n\nTotal Rp 10.000\n"
        );
    }
}

//! A guestbook showing Renox's features so far: named routes, views with a
//! layout, sessions and flash messages, CSRF, HTMX fragments, validation with
//! old input, SQLite with a model, migrations, a seeder and pagination,
//! login/registration and the account page from the `Auth` module, and an event whose listener
//! queues a job, plus a scheduled task and an app command
//! (`cargo run -- entries:prune --days 7`).
//!
//! The app lives in this library (`app()`), so `tests/` can boot it;
//! `main.rs` only runs it. Run it from this directory:
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed     # optional: 30 fake entries
//! cargo run
//! ```

use renox::fake::Fake;
use renox::fake::faker::lorem::en::Sentence;
use renox::fake::faker::name::en::FirstName;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Default)]
#[model(table = "entries")]
struct Entry {
    id: i64,
    name: String,
    message: String,
    /// Storage key of an optional photo, e.g. `public/entries/abc.jpg`.
    photo: Option<String>,
    created_at: Option<DateTime>,
    updated_at: Option<DateTime>,
}

impl Factory for Entry {
    fn definition() -> Self {
        Entry {
            name: FirstName().fake(),
            message: Sentence(3..8).fake(),
            ..Default::default()
        }
    }
}

#[derive(Deserialize)]
struct EntryForm {
    name: String,
    message: String,
    photo: Option<Upload>,
}

impl Validate for EntryForm {
    fn rules(&self, v: &mut Validator) {
        // Field names in messages come from resources/lang/*.json
        // (`renox.validation.attributes.*`), in the visitor's language.
        v.field("name", &self.name).required().max(50);
        v.field("message", &self.message).required().between(3, 280);
        v.field("photo", &self.photo).image().max(2048);
    }
}

/// Emitted when someone signs the guestbook.
#[derive(Clone)]
struct EntryPosted {
    entry_id: i64,
}

impl Event for EntryPosted {}

/// Thanks the guest in the background (to the log, with `MAIL_MAILER=log`).
#[derive(Serialize, Deserialize)]
struct ThankGuest {
    entry_id: i64,
}

impl Job for ThankGuest {
    const NAME: &'static str = "thank-guest";

    async fn handle(self, ctx: JobContext) -> Result {
        let entry = Entry::find_or_404(&ctx.state.db, self.entry_id).await?;
        let mail = renox::mail::Mail::new(
            "owner@example.com",
            format!("{} menulis di buku tamu", entry.name),
            entry.message,
        );
        ctx.state.mailer.send(mail).await
    }
}

struct Guestbook;

impl Module for Guestbook {
    fn name(&self) -> &'static str {
        "guestbook"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("guestbook.index")
            .post("/entries", store)
            .name("guestbook.store")
            .get("/halo/{nama}", greet)
            .name("greet")
            .get("/bahasa/{locale}", switch_language)
            .name("language")
    }

    fn register(&self, app: &mut Registry) {
        app.job::<ThankGuest>()
            .listen(|event: EntryPosted, state| async move {
                state
                    .dispatch(ThankGuest {
                        entry_id: event.entry_id,
                    })
                    .await?;
                Ok(())
            });
        // `cargo run -- entries:prune --days 7`
        app.command(
            "entries:prune",
            "Delete entries older than --days (default 30)",
            prune,
        );
        app.schedule()
            .every_minute("count-entries", |state| async move {
                let total = Entry::query().count(&state.db).await?;
                tracing::info!(total, "guestbook entries");
                Ok(())
            });
    }
}

/// The `entries:prune` command.
async fn prune(state: AppState, args: renox::command::Args) -> Result {
    let days: i64 = args
        .value("--days")
        .unwrap_or("30")
        .parse()
        .map_err(|_| Error::BadRequest("--days must be a number".into()))?;
    let cutoff = renox::db::now() - renox::chrono::TimeDelta::days(days);
    let deleted = Entry::query()
        .where_op("created_at", "<", cutoff)
        .delete(&state.db)
        .await?;
    println!("Deleted {deleted} entries older than {days} days.");
    Ok(())
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let entries = Entry::query().latest().paginate(&db, page, 10).await?;
    Ok(view("guestbook/index.html", context! { entries }).fragment("entries"))
}

/// Invalid input never gets here: `Valid` sends regular posts back with the
/// errors and old input, and HTMX posts get a 422 the bundled script shows
/// next to the fields.
async fn store(
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
    back: Back,
    lang: Lang,
    Valid(form): Valid<EntryForm>,
) -> Result<Response> {
    let photo = match &form.photo {
        Some(photo) => Some(photo.store_public(&state.storage, "entries").await?),
        None => None,
    };
    let entry = Entry::create(
        &state.db,
        Entry {
            name: form.name,
            message: form.message,
            photo,
            ..Default::default()
        },
    )
    .await?;
    state.emit(EntryPosted { entry_id: entry.id }).await?;

    if htmx.request {
        let entries = Entry::query().latest().paginate(&state.db, 1, 10).await?;
        return Ok((
            HxTrigger("entry-added".into()),
            view("guestbook/index.html", context! { entries }).fragment("entries"),
        )
            .into_response());
    }
    session.flash(
        "status",
        lang.t("guestbook.thanks", &[("name", &entry.name)]),
    )?;
    Ok(back.into_response())
}

/// Remembers the visitor's language and goes back to where they were.
async fn switch_language(session: Session, back: Back, Path(locale): Path<String>) -> Result<Back> {
    renox::i18n::set_locale(&session, &locale)?;
    Ok(back)
}

async fn greet(Path(nama): Path<String>) -> String {
    format!("Halo, {nama}!")
}

/// The guestbook app; `main.rs` runs it and the tests boot it.
pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new().account().redirect_to("/")) // login, register, /account
        .module(Guestbook)
        .seeder(|db| async move {
            Entry::create_many(&db, 30).await?;
            Ok(())
        })
}

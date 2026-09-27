//! A guestbook showing Renox's features so far: named routes, views with a
//! layout, sessions and flash messages, CSRF, old input, HTMX fragments, and
//! SQLite with a model, migrations, a seeder and pagination.
//!
//! Run it from this directory:
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

#[derive(Serialize, Deserialize)]
struct EntryForm {
    name: String,
    message: String,
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
    }
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let entries = Entry::query().latest().paginate(&db, page, 10).await?;
    Ok(view("guestbook/index.html", context! { entries }).fragment("entries"))
}

async fn store(
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
    back: Back,
    Form(form): Form<EntryForm>,
) -> Result<Response> {
    if form.name.trim().is_empty() || form.message.trim().is_empty() {
        session.flash_input(&form)?;
        session.flash_errors(&context! { form => ["Nama dan pesan wajib diisi."] })?;
        // Validation responses for HTMX forms arrive in M3; until then reload the page.
        if htmx.request {
            return Ok(HxRedirect(state.url("guestbook.index", &[])?).into_response());
        }
        return Ok(back.into_response());
    }

    Entry::create(
        &state.db,
        Entry {
            name: form.name,
            message: form.message,
            ..Default::default()
        },
    )
    .await?;

    if htmx.request {
        let entries = Entry::query().latest().paginate(&state.db, 1, 10).await?;
        return Ok((
            HxTrigger("entry-added".into()),
            view("guestbook/index.html", context! { entries }).fragment("entries"),
        )
            .into_response());
    }
    session.flash("status", "Terima kasih, pesanmu tersimpan!")?;
    Ok(back.into_response())
}

async fn greet(Path(nama): Path<String>) -> String {
    format!("Halo, {nama}!")
}

fn main() -> renox::Result {
    App::new()
        .migrations(renox::migrations!())
        .module(Guestbook)
        .seeder(|db| async move {
            Entry::create_many(&db, 30).await?;
            Ok(())
        })
        .run()
}

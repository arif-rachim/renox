//! A tiny guestbook showing Renox's M1 features: named routes, views with a
//! layout, sessions and flash messages, CSRF, old input and HTMX fragments.
//!
//! Run it from this directory: `cargo run`.

use std::sync::Mutex;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// Kept in memory until the database arrives in M2.
static ENTRIES: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
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

async fn index() -> View {
    let entries = ENTRIES.lock().unwrap().clone();
    view("guestbook/index.html", context! { entries }).fragment("entries")
}

async fn store(
    State(state): State<AppState>,
    session: Session,
    htmx: Htmx,
    back: Back,
    Form(entry): Form<Entry>,
) -> Result<Response> {
    if entry.name.trim().is_empty() || entry.message.trim().is_empty() {
        session.flash_input(&entry)?;
        session.flash_errors(&context! { form => ["Nama dan pesan wajib diisi."] })?;
        // Validation responses for HTMX forms arrive in M3; until then reload the page.
        if htmx.request {
            return Ok(HxRedirect(state.url("guestbook.index", &[])?).into_response());
        }
        return Ok(back.into_response());
    }

    ENTRIES.lock().unwrap().insert(0, entry);

    if htmx.request {
        let entries = ENTRIES.lock().unwrap().clone();
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
    App::new().module(Guestbook).run()
}

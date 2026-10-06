//! The public home page and the language switch.
//!
//! The app itself was made with `rnx new bikeshop`; this module is the home
//! page it wrote, moved onto the bike shop's public layout. The catalogue
//! (#233) gives the home page its products later.

pub mod explain;

use renox::prelude::*;

/// The home page and the language menu's route.
pub struct Home;

impl Module for Home {
    fn name(&self) -> &'static str {
        "home"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .post("/locale/{locale}", locale)
            .name("locale.update")
    }
}

/// The languages the shop is written in (`resources/lang/<locale>.json`).
pub const LOCALES: [(&str, &str); 2] = [("en", "English"), ("es", "Español")];

async fn index() -> View {
    view("home/index.html", context! {})
}

/// Remembers the visitor's language (the layout's language menu) and goes
/// back to the page they were on. An unknown language is ignored.
async fn locale(session: Session, back: Back, Path(locale): Path<String>) -> Result<Back> {
    if LOCALES.iter().any(|(code, _)| *code == locale) {
        renox::i18n::remember_locale(&session, &locale)?;
    }
    Ok(back)
}

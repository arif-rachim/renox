//! Interactive blocks for Renox apps: components the UI kit doesn't carry,
//! as template macros on the kit's tokens, with their JavaScript loaded only
//! on pages that use them.
//!
//! ```
//! use renox::prelude::*;
//! use renox_blocks::Blocks;
//!
//! # let _ =
//! App::new().module(Blocks::new())
//! # ;
//! ```
//!
//! Then, in a template:
//!
//! ```html
//! {% from "renox-blocks/blocks.html" import quantity, swatches, gallery %}
//! <form method="post" action="/cart">
//!   {{ csrf_field() }}
//!   {{ swatches("size", "Frame size", sizes, selected="M", required=true) }}
//!   {{ quantity("qty", 1, min=1, max=10) }}
//! </form>
//! ```
//!
//! - Input: `quantity` (a stepper), `range_slider` (two handles),
//!   `keypad` (a point-of-sale number pad), `swatches` (variant chips),
//!   `datetime_range` (a start and an end, each a day and a time).
//! - Showing data: `gallery` (a carousel), `history` (a timeline),
//!   `compare_plans` (a pricing table).
//! - Scheduling and work: `month_calendar`, `availability` (resources
//!   against hours), `kanban` (with `kanban_card`).
//!
//! The form blocks send plain fields, so `Valid<T>` reads them as any
//! other. Each block works with the keyboard, follows
//! `prefers-reduced-motion` and works under `CSP=strict`. Its texts are
//! English, and an app translates them in its lang files ([`TEXTS`] lists
//! the keys). The guide is docs/blocks.md in the Renox repository.

#![warn(missing_docs)]

use renox::minijinja::value::{Kwargs, Value};
use renox::minijinja::{self, State};
use renox::prelude::*;

mod assets;
mod texts;

pub use texts::TEXTS;

/// The macros, compiled in. An app replaces the file with one of the same
/// name under its views directory (`resources/views/renox-blocks/blocks.html`).
const VIEWS: &[(&str, &str)] = &[(
    "renox-blocks/blocks.html",
    include_str!("../views/blocks.html"),
)];

/// The blocks module: add it to the app, then import the macros from
/// `renox-blocks/blocks.html`.
#[derive(Debug, Clone, Default)]
pub struct Blocks {}

impl Blocks {
    /// The module with its defaults.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for Blocks {
    fn name(&self) -> &'static str {
        "blocks"
    }

    fn register(&self, app: &mut Registry) {
        assets::register(app);
        app.templates(|env| {
            for (name, source) in VIEWS {
                // The app's own file of that name wins.
                if env.get_template(name).is_err() {
                    let _ = env.add_template(name, source);
                }
            }
            env.add_function("renox_blocks", || {
                minijinja::Value::from_safe_string(assets::tags())
            });
            env.add_function("renox_blocks_t", text);
        });
    }
}

/// `renox_blocks_t('blocks.gallery.position', n=1, total=4)`: the app's
/// translation of a block's text (through the page's `t`), else the
/// English one, with its `:name` placeholders filled.
fn text(state: &State, key: &str, kwargs: Kwargs) -> Result<String, minijinja::Error> {
    let mut params: Vec<(String, Value)> = Vec::new();
    for name in kwargs.args() {
        params.push((name.to_owned(), kwargs.get::<Value>(name)?));
    }
    kwargs.assert_all_used()?;
    let translated = state
        .lookup("t")
        .and_then(|t| {
            let args = Kwargs::from_iter(params.iter().map(|(k, v)| (k.clone(), v.clone())));
            t.call(state, &[Value::from(key), Value::from(args)]).ok()
        })
        .and_then(|said| said.as_str().map(str::to_owned));
    if let Some(said) = translated
        && said != key
    {
        return Ok(said);
    }
    let english = texts::english(key).unwrap_or(key);
    let params: Vec<(&str, String)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.to_string()))
        .collect();
    Ok(renox::i18n::format(english, &params, None))
}

/// Compiles the Rust in docs/blocks.md (the guide) as doctests.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/blocks.md")]
pub struct Guide;

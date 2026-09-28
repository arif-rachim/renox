//! Translations for app texts, and the language of each request.
//!
//! Texts live in `resources/lang/{locale}.json` (`LANG_PATH`), nested or flat:
//!
//! ```json
//! { "produk": { "disimpan": ":nama tersimpan", "jumlah": "Satu produk|:count produk" } }
//! ```
//!
//! ```
//! # use renox::prelude::*;
//! // templates: {{ t('produk.disimpan', nama='Kopi') }}  {{ t('produk.jumlah', count=3) }}
//! async fn store(lang: Lang, session: Session, back: Back) -> Result<Back> {
//!     session.flash("status", lang.t("produk.disimpan", &[("nama", &"Kopi")]))?;
//!     Ok(back)
//! }
//! # fn demo(session: &Session) -> Result {
//! renox::i18n::set_locale(&session, "en")?;   // this visitor's language from now on
//! # Ok(()) }
//! ```
//!
//! A request's language is the one stored with `set_locale`, else `APP_LOCALE`.
//! Missing keys fall back to `APP_FALLBACK_LOCALE`, then to the key itself.
//! Renox's own texts can be replaced or translated from the same files:
//! `renox.validation.required`, `renox.validation.attributes.email` and
//! `renox.auth.login_title` (see the built-in pages for the other keys).

use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Context;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use serde_json::Value;

use crate::{AppState, Result, Session};

const SESSION_KEY: &str = "_locale";
/// How often debug mode looks for changed translation files.
const RELOAD_CHECK: Duration = Duration::from_secs(1);

pub(crate) type Texts = Arc<HashMap<String, String>>;

struct Loaded {
    locales: HashMap<String, Texts>,
    fingerprint: Vec<(PathBuf, SystemTime)>,
    checked: Instant,
}

/// Every locale's texts, keyed by dotted path.
pub struct Translator {
    dir: PathBuf,
    reload: bool,
    loaded: RwLock<Loaded>,
}

fn fingerprint(dir: &Path) -> Vec<(PathBuf, SystemTime)> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let modified = p
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (p, modified)
        })
        .collect();
    files.sort();
    files
}

fn flatten(prefix: &str, value: &Value, out: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&key, value, out);
            }
        }
        Value::String(text) => {
            out.insert(prefix.to_owned(), text.clone());
        }
        Value::Null => {}
        other => {
            out.insert(prefix.to_owned(), other.to_string());
        }
    }
}

fn parse(locales: &mut HashMap<String, Texts>, file: &str, text: &str) -> anyhow::Result<()> {
    let locale = file.strip_suffix(".json").unwrap_or(file).to_owned();
    let json: Value =
        serde_json::from_str(text).with_context(|| format!("{file} is not valid JSON"))?;
    let mut flat = HashMap::new();
    flatten("", &json, &mut flat);
    locales.insert(locale, Arc::new(flat));
    Ok(())
}

fn read(dir: &Path) -> anyhow::Result<HashMap<String, Texts>> {
    let mut locales = HashMap::new();
    for (path, _) in fingerprint(dir) {
        let locale = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let json: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        let mut flat = HashMap::new();
        flatten("", &json, &mut flat);
        locales.insert(locale, Arc::new(flat));
    }
    Ok(locales)
}

impl Translator {
    /// Uses translation files compiled into the binary.
    pub(crate) fn embedded(files: &[(&str, &str)]) -> anyhow::Result<Self> {
        let mut locales = HashMap::new();
        for (file, text) in files
            .iter()
            .filter(|(f, _)| f.ends_with(".json") && !f.contains('/'))
        {
            parse(&mut locales, file, text)?;
        }
        Ok(Self {
            dir: PathBuf::new(),
            reload: false,
            loaded: RwLock::new(Loaded {
                locales,
                fingerprint: Vec::new(),
                checked: Instant::now(),
            }),
        })
    }

    /// Loads `dir`; a file that isn't valid JSON is an error.
    pub(crate) fn load(dir: &Path, reload: bool) -> anyhow::Result<Self> {
        Ok(Self {
            dir: dir.to_path_buf(),
            reload,
            loaded: RwLock::new(Loaded {
                locales: read(dir)?,
                fingerprint: fingerprint(dir),
                checked: Instant::now(),
            }),
        })
    }

    fn refresh(&self) {
        if !self.reload {
            return;
        }
        {
            let loaded = self.loaded.read().unwrap_or_else(|e| e.into_inner());
            if loaded.checked.elapsed() < RELOAD_CHECK {
                return;
            }
        }
        let mut loaded = self.loaded.write().unwrap_or_else(|e| e.into_inner());
        loaded.checked = Instant::now();
        let now = fingerprint(&self.dir);
        if now != loaded.fingerprint {
            match read(&self.dir) {
                Ok(locales) => loaded.locales = locales,
                Err(err) => tracing::error!(error = ?err, "keeping the previous translations"),
            }
            loaded.fingerprint = now;
        }
    }

    /// The texts of one locale (empty if there is no file for it).
    pub(crate) fn texts(&self, locale: &str) -> Texts {
        self.refresh();
        let loaded = self.loaded.read().unwrap_or_else(|e| e.into_inner());
        loaded.locales.get(locale).cloned().unwrap_or_default()
    }

    /// Locales with a translation file.
    pub fn locales(&self) -> Vec<String> {
        self.refresh();
        let loaded = self.loaded.read().unwrap_or_else(|e| e.into_inner());
        let mut locales: Vec<_> = loaded.locales.keys().cloned().collect();
        locales.sort();
        locales
    }

    /// The text for `key` in `locale`, else in `fallback`, else the key itself.
    pub fn get(&self, locale: &str, fallback: &str, key: &str) -> String {
        if let Some(text) = self.texts(locale).get(key) {
            return text.clone();
        }
        self.texts(fallback)
            .get(key)
            .cloned()
            .or_else(|| {
                builtin(locale)
                    .or_else(|| builtin(fallback))?
                    .get(key)
                    .map(|t| (*t).to_owned())
            })
            .unwrap_or_else(|| key.to_owned())
    }
}

/// Texts Renox's own templates use (the UI kit), for English and Indonesian;
/// an app's `lang/*.json` can change them.
fn builtin(locale: &str) -> Option<&'static HashMap<&'static str, &'static str>> {
    static EN: std::sync::LazyLock<HashMap<&str, &str>> = std::sync::LazyLock::new(|| {
        HashMap::from([
            ("ui.optional", "optional"),
            ("ui.cancel", "Cancel"),
            ("ui.close", "Close"),
            ("ui.dismiss", "Dismiss"),
            ("ui.more", "More"),
            ("ui.errors_title", "Please check the highlighted fields."),
        ])
    });
    static ID: std::sync::LazyLock<HashMap<&str, &str>> = std::sync::LazyLock::new(|| {
        HashMap::from([
            ("ui.optional", "opsional"),
            ("ui.cancel", "Batal"),
            ("ui.close", "Tutup"),
            ("ui.dismiss", "Tutup"),
            ("ui.more", "Lainnya"),
            ("ui.errors_title", "Periksa kembali isian yang ditandai."),
        ])
    });
    match locale {
        "en" => Some(&EN),
        "id" => Some(&ID),
        _ => None,
    }
}

/// Fills `:name` (and `:Name`, capitalised) placeholders, and picks the
/// singular or plural side of `one|many` texts when `count` is given.
pub fn format(text: &str, params: &[(&str, String)], count: Option<i64>) -> String {
    let text = match (count, text.split_once('|')) {
        (Some(1), Some((one, _))) => one.to_owned(),
        (Some(_), Some((_, many))) => many.to_owned(),
        _ => text.to_owned(),
    };
    let mut params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    if let Some(count) = count {
        params.push(("count".into(), count.to_string()));
    }
    // Longest names first, so `:name` doesn't eat the start of `:names`.
    params.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = text;
    for (name, value) in params {
        let mut capitalised = value.clone();
        if let Some(first) = capitalised.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let mut upper_name = name.clone();
        if let Some(first) = upper_name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        out = out
            .replace(&format!(":{upper_name}"), &capitalised)
            .replace(&format!(":{name}"), &value);
    }
    out
}

/// The language chosen for the current request.
#[derive(Debug, Clone)]
pub(crate) struct RequestLocale(pub String);

/// The request's language in [`crate::context`], for code without the
/// request (mail views, notifications).
#[derive(Debug, Clone)]
struct ContextLocale(String);

thread_local! {
    /// Set while a notification builds its messages for a recipient.
    static OVERRIDE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The language to use now: the recipient's while a notification is built,
/// else the current request's (or the one a job set with
/// [`with_locale`]), else `APP_LOCALE`.
pub fn current_locale(state: &AppState) -> String {
    OVERRIDE
        .with(|o| o.borrow().clone())
        .or_else(|| crate::context::get::<ContextLocale>().map(|l| l.0))
        .unwrap_or_else(|| state.config.locale.clone())
}

/// Runs `f` (synchronous code, e.g. building a mail) in `locale`; `None`
/// leaves the language as it is.
pub fn with_locale<T>(locale: Option<&str>, f: impl FnOnce() -> T) -> T {
    let Some(locale) = locale else { return f() };
    let previous = OVERRIDE.with(|o| o.borrow_mut().replace(locale.to_owned()));
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            OVERRIDE.with(|o| *o.borrow_mut() = previous);
        }
    }
    let _restore = Restore(previous);
    f()
}

/// Makes `locale` the language of the rest of this request, job or task
/// (e.g. a job that mails a user in their language).
pub fn set_current_locale(locale: &str) {
    crate::context::set(ContextLocale(locale.to_owned()));
}

/// Makes `locale` this visitor's language from the next request on.
pub fn set_locale(session: &Session, locale: &str) -> Result {
    session.put(SESSION_KEY, locale)
}

/// Picks the request's language: the session's choice if Renox knows the
/// locale (a translation file or a built-in language), else `APP_LOCALE`.
pub(crate) async fn middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let chosen = req
        .extensions()
        .get::<Session>()
        .and_then(|s| s.get::<String>(SESSION_KEY))
        .filter(|locale| {
            matches!(locale.as_str(), "en" | "id")
                || state.translator.locales().iter().any(|l| l == locale)
        });
    let locale = chosen.unwrap_or_else(|| state.config.locale.clone());
    set_current_locale(&locale);
    req.extensions_mut().insert(RequestLocale(locale));
    next.run(req).await
}

pub(crate) fn request_locale(extensions: &axum::http::Extensions, state: &AppState) -> String {
    extensions
        .get::<RequestLocale>()
        .map(|l| l.0.clone())
        .unwrap_or_else(|| state.config.locale.clone())
}

/// The current request's language and its texts.
///
/// ```
/// # use renox::prelude::*;
/// async fn index(lang: Lang) -> String { lang.t("welcome", &[("name", &"Arif")]) }
/// ```
#[derive(Clone)]
pub struct Lang {
    pub locale: String,
    pub(crate) state: AppState,
}

impl Lang {
    /// The request's language, for code that has the state but no request.
    pub(crate) fn of(state: &AppState, locale: &str) -> Self {
        Self {
            locale: locale.to_owned(),
            state: state.clone(),
        }
    }

    pub(crate) fn texts(&self) -> Texts {
        self.state.translator.texts(&self.locale)
    }

    pub fn t(&self, key: &str, params: &[(&str, &dyn Display)]) -> String {
        let params: Vec<(&str, String)> = params.iter().map(|(k, v)| (*k, v.to_string())).collect();
        format(&self.raw(key), &params, None)
    }

    /// Like `t`, choosing the `one|many` side by `count` and filling `:count`.
    pub fn choice(&self, key: &str, count: i64, params: &[(&str, &dyn Display)]) -> String {
        let params: Vec<(&str, String)> = params.iter().map(|(k, v)| (*k, v.to_string())).collect();
        format(&self.raw(key), &params, Some(count))
    }

    fn raw(&self, key: &str) -> String {
        self.state
            .translator
            .get(&self.locale, &self.state.config.fallback_locale, key)
    }
}

impl AppState {
    /// The texts of `locale`, for code without a request (a job, a mail
    /// to someone in another language).
    pub fn lang(&self, locale: &str) -> Lang {
        Lang::of(self, locale)
    }

    /// The texts of the [`current_locale`].
    pub fn current_lang(&self) -> Lang {
        Lang::of(self, &current_locale(self))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> std::result::Result<Self, Infallible> {
        let state = parts
            .extensions
            .get::<AppState>()
            .cloned()
            .expect("the auth middleware puts AppState in every request");
        Ok(Self {
            locale: request_locale(&parts.extensions, &state),
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_placeholders_and_plurals() {
        let p = [("nama", "kopi".to_owned())];
        assert_eq!(format(":Nama, :nama!", &p, None), "Kopi, kopi!");
        assert_eq!(
            format("Satu produk|:count produk", &[], Some(1)),
            "Satu produk"
        );
        assert_eq!(
            format("Satu produk|:count produk", &[], Some(4)),
            "4 produk"
        );
        // Like Laravel: 0 and negative counts take the plural form.
        assert_eq!(
            format("Satu produk|:count produk", &[], Some(0)),
            "0 produk"
        );
        assert_eq!(format("one|:count many", &[], Some(-1)), "-1 many");
        // Without a plural form the text is used as it is.
        assert_eq!(format(":count item", &[], Some(3)), "3 item");
        assert_eq!(
            format(
                ":name :names",
                &[("name", "a".into()), ("names", "b".into())],
                None
            ),
            "a b"
        );
    }

    #[test]
    fn flattens_nested_files() {
        let json = serde_json::json!({ "a": { "b": "x", "n": 3 }, "c": "y" });
        let mut out = HashMap::new();
        flatten("", &json, &mut out);
        assert_eq!(out["a.b"], "x");
        assert_eq!(out["a.n"], "3");
        assert_eq!(out["c"], "y");
    }
}

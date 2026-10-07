//! "About this page": every page of the bike shop explains itself.
//!
//! Each area keeps the explanations of its pages next to its routes, in
//! `src/app/<area>/explain.rs`, as a list of [`Explanation`]s keyed by the
//! route's name. This module collects them ([`all`]), finds the one for the
//! page being shown ([`for_route`]) and turns it into what the templates
//! show, in the visitor's language ([`Page`]).
//!
//! Two places show them:
//! - the "About this page" panel in both layouts
//!   (`resources/views/about/_panel.html`), filled by the
//!   `about_page(request.route, request.path)` template function registered
//!   in [`register`];
//! - `/about/pages`, the index of every page, filtered by Renox feature or by
//!   who uses the page (`src/app/about/mod.rs`).
//!
//! `BIKESHOP_EXPLAIN=false` hides the panels for a "clean" demo
//! ([`enabled`]); `/about/pages` stays.
//!
//! ## Languages
//!
//! The texts here are English. Another language reads them from its
//! `resources/lang/<locale>.json` under `about_page.<route name>.…`:
//! `title`, `purpose`, `who`, `under_hood` and `features.<n>` (the n-th
//! feature's "why", counted from 0); the API names, docs links and file
//! paths are never translated. A text missing there falls back to English,
//! so a page is never without its explanation. For the home page in
//! Spanish (`resources/lang/es.json`):
//!
//! ```json
//! { "about_page": { "home": { "title": "Inicio", "features": { "0": "…" } } } }
//! ```
//!
//! ## The walker test
//!
//! `tests/about.rs` walks every GET route of the app (`route:list`'s table)
//! and fails when one has no explanation (or isn't in [`not_pages`] with a
//! reason), when a docs link points to a file or `#anchor` that doesn't exist
//! under `docs/`, or when a source file doesn't exist in the repository.

use renox::minijinja::{self, Value};
use serde::Serialize;

/// The repository's address on GitHub, for links to source files and guides.
pub const REPOSITORY: &str = "https://github.com/arif-rachim/renox/blob/main/";

/// The documentation site, for links to the guides (`docs/ui.md` is
/// `/docs/ui` there).
pub const DOCS_SITE: &str = "https://renox.rs/docs/";

/// The explanation of one page (one named GET route).
///
/// Written as a `const`-friendly literal in the area's `explain.rs`:
///
/// ```
/// use bikeshop::explain::{Audience, Explanation, Feature, Flow};
///
/// let home = Explanation {
///     route: "home",
///     path: "/",
///     title: "Home",
///     purpose: "The shop's front door.",
///     who: "Anyone who opens the site.",
///     audience: &[Audience::Visitor],
///     flow: Flow::Buy,
///     features: &[Feature { api: "Routes::get", why: "One page, one handler." }],
///     under_hood: "Nothing is read from the database.",
///     docs: &["docs/routing.md#apps-modules-and-routes"],
///     sources: &["examples/bikeshop/src/app/home/mod.rs"],
/// };
/// assert_eq!(home.route, "home");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Explanation {
    /// The route's name (`Routes::name`), e.g. `catalog.show`.
    pub route: &'static str,
    /// The route's path pattern, as `route:list` shows it: `/products/{slug}`.
    /// The walker test checks it against the app's routes. The panel finds
    /// the page by it when the route's name isn't enough (see [`resolve`]).
    pub path: &'static str,
    /// The page's name, as a reader would call it.
    pub title: &'static str,
    /// What the page is for, and where it sits in the business.
    pub purpose: &'static str,
    /// Who uses it, in a sentence (the roles are in `audience`).
    pub who: &'static str,
    /// Who uses it, for the filter on `/about/pages`.
    pub audience: &'static [Audience],
    /// The part of the business the page belongs to.
    pub flow: Flow,
    /// The Renox features the page uses, and why each was the right choice.
    pub features: &'static [Feature],
    /// What happens when the page loads or a form on it is sent: queries,
    /// transactions, jobs, events, mails. Markdown (`code` is fine).
    pub under_hood: &'static str,
    /// Guide sections, as `docs/<file>.md#<heading anchor>`.
    pub docs: &'static [&'static str],
    /// Files in the repository (handler, template, test…), from its root.
    pub sources: &'static [&'static str],
}

/// One Renox feature a page uses.
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    /// The API, written the same way on every page so the filter on
    /// `/about/pages` groups them: `renox::grid`, `Valid<T>`, `Routes::throttle_by`…
    pub api: &'static str,
    /// What it does on this page, and why it was the right choice. Markdown.
    pub why: &'static str,
}

/// Who uses a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// Someone not logged in.
    Visitor,
    /// A logged-in customer.
    Customer,
    /// Anyone on a store's staff.
    Staff,
    /// Takes payments at a store's counter.
    Cashier,
    /// Works in a store's workshop.
    Mechanic,
    /// Runs one store.
    Manager,
    /// Runs all three stores.
    Owner,
    /// Someone reading the example to learn Renox.
    Developer,
}

impl Audience {
    /// Every audience, in the order the filter lists them.
    pub const ALL: [Audience; 8] = [
        Audience::Visitor,
        Audience::Customer,
        Audience::Staff,
        Audience::Cashier,
        Audience::Mechanic,
        Audience::Manager,
        Audience::Owner,
        Audience::Developer,
    ];

    /// The key used in URLs and translations (`about.audience.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Audience::Visitor => "visitor",
            Audience::Customer => "customer",
            Audience::Staff => "staff",
            Audience::Cashier => "cashier",
            Audience::Mechanic => "mechanic",
            Audience::Manager => "manager",
            Audience::Owner => "owner",
            Audience::Developer => "developer",
        }
    }

    /// The English label.
    pub fn label(self) -> &'static str {
        match self {
            Audience::Visitor => "Visitor",
            Audience::Customer => "Customer",
            Audience::Staff => "Staff",
            Audience::Cashier => "Cashier",
            Audience::Mechanic => "Mechanic",
            Audience::Manager => "Store manager",
            Audience::Owner => "Owner",
            Audience::Developer => "Developer",
        }
    }

    /// The audience with this key.
    pub fn from_key(key: &str) -> Option<Audience> {
        Audience::ALL.into_iter().find(|a| a.key() == key)
    }
}

/// The part of the business a page belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    /// Buying bikes and gear.
    Buy,
    /// Renting a bike.
    Rent,
    /// Services and service plans.
    Service,
    /// The customer's own account.
    Account,
    /// Running the stores: stock, staff, reports.
    BackOffice,
    /// Pages about the example itself.
    Learn,
}

impl Flow {
    /// The key used in translations (`about.flow.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Flow::Buy => "buy",
            Flow::Rent => "rent",
            Flow::Service => "service",
            Flow::Account => "account",
            Flow::BackOffice => "back_office",
            Flow::Learn => "learn",
        }
    }

    /// The English label.
    pub fn label(self) -> &'static str {
        match self {
            Flow::Buy => "Buy",
            Flow::Rent => "Rent",
            Flow::Service => "Service",
            Flow::Account => "Account",
            Flow::BackOffice => "Back office",
            Flow::Learn => "Learning Renox",
        }
    }
}

/// A GET route that is not a page (a JSON answer, a file, a stream), with
/// the reason it needs no "About this page". The walker test accepts these.
#[derive(Debug, Clone, Copy)]
pub struct NotAPage {
    /// The route's name.
    pub route: &'static str,
    /// Why it has no explanation, e.g. "a CSV download".
    pub reason: &'static str,
}

/// Every page's explanation, from every area.
pub fn all() -> Vec<Explanation> {
    crate::app::explanations()
}

/// GET routes that aren't pages, from every area.
pub fn not_pages() -> Vec<NotAPage> {
    crate::app::not_pages()
}

/// The explanation of the route named `route`.
pub fn for_route(route: &str) -> Option<Explanation> {
    all().into_iter().find(|e| e.route == route)
}

/// The explanation of the page being shown: by the current route's name
/// (`request.route`), else by its address (`request.path`) against the
/// explanations' path patterns.
///
/// Why the address too: Renox names the current route from its path alone,
/// so `GET /account` reports the alphabetically first name of every route
/// on `/account` (`account.destroy`, the `DELETE`), and `GET /products/{id}`
/// would report `products.destroy` rather than `products.show`. Only GET
/// pages are explained, so a name without an explanation falls back to the
/// GET page at that address. When paths overlap (`/products/new` and
/// `/products/{id}`), the pattern with more fixed segments wins, as in the
/// router.
pub fn resolve(route: Option<&str>, path: &str) -> Option<Explanation> {
    let all = all();
    if let Some(found) = route.and_then(|r| all.iter().find(|e| e.route == r)) {
        return Some(*found);
    }
    all.into_iter()
        .filter_map(|e| path_matches(e.path, path).map(|fixed| (fixed, e)))
        .max_by_key(|(fixed, _)| *fixed)
        .map(|(_, e)| e)
}

/// Whether `path` (`/products/12`) matches `pattern` (`/products/{id}`):
/// the number of fixed segments that matched, for choosing the most
/// specific pattern.
pub fn path_matches(pattern: &str, path: &str) -> Option<usize> {
    let mut fixed = 0;
    let mut want = pattern.trim_matches('/').split('/');
    let mut got = path.trim_matches('/').split('/');
    loop {
        match (want.next(), got.next()) {
            (None, None) => return Some(fixed),
            (Some(w), _) if w.starts_with("{*") => return Some(fixed),
            (Some(w), Some(g)) if w.starts_with('{') && !g.is_empty() => {}
            (Some(w), Some(g)) if w == g => fixed += 1,
            _ => return None,
        }
    }
}

/// Whether the "About this page" panels are shown: `BIKESHOP_EXPLAIN` unset,
/// or anything but `false`, `0`, `off` or `no`.
pub fn enabled(config: &renox::Config) -> bool {
    !matches!(
        config
            .var("BIKESHOP_EXPLAIN")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Some("false" | "0" | "off" | "no")
    )
}

/// An explanation as the templates show it, in one language.
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    /// The route's name.
    pub route: String,
    /// The route's path pattern.
    pub path: &'static str,
    /// The page's address when it has no parameters, for links.
    pub url: Option<String>,
    /// The page's name.
    pub title: String,
    /// What it is for.
    pub purpose: String,
    /// Who uses it.
    pub who: String,
    /// The audiences, with their labels.
    pub audience: Vec<Label>,
    /// The part of the business.
    pub flow: Label,
    /// The Renox features, with why.
    pub features: Vec<FeatureText>,
    /// What happens underneath (Markdown).
    pub under_hood: String,
    /// Links to the guides.
    pub docs: Vec<DocLink>,
    /// Links to the source files.
    pub sources: Vec<SourceLink>,
}

/// A key and its label in the page's language.
#[derive(Debug, Clone, Serialize)]
pub struct Label {
    /// The key (`cashier`, `back_office`).
    pub key: &'static str,
    /// The label (`Cashier`).
    pub label: String,
}

/// A feature in the page's language.
#[derive(Debug, Clone, Serialize)]
pub struct FeatureText {
    /// The API, never translated.
    pub api: &'static str,
    /// Why it is used here (Markdown).
    pub why: String,
}

/// A link to a guide section.
#[derive(Debug, Clone, Serialize)]
pub struct DocLink {
    /// As written in the explanation: `docs/ui.md#navigation-and-page-structure`.
    pub path: &'static str,
    /// The section on the documentation site.
    pub site: String,
    /// The file on GitHub.
    pub github: String,
}

/// A link to a file of the example.
#[derive(Debug, Clone, Serialize)]
pub struct SourceLink {
    /// From the repository's root.
    pub path: &'static str,
    /// The file on GitHub.
    pub github: String,
}

impl Explanation {
    /// The explanation in one language: `translate(key)` returns the text
    /// for a translation key, or `None` when the language has none (English
    /// is then used).
    pub fn localize(&self, translate: &dyn Fn(&str) -> Option<String>) -> Page {
        let text = |field: &str, english: &str| {
            translate(&format!("about_page.{}.{field}", self.route))
                .unwrap_or_else(|| english.to_owned())
        };
        let label = |kind: &str, key: &'static str, english: &str| Label {
            key,
            label: translate(&format!("about.{kind}.{key}")).unwrap_or_else(|| english.to_owned()),
        };
        Page {
            route: self.route.to_owned(),
            path: self.path,
            url: None,
            title: text("title", self.title),
            purpose: text("purpose", self.purpose),
            who: text("who", self.who),
            audience: self
                .audience
                .iter()
                .map(|a| label("audience", a.key(), a.label()))
                .collect(),
            flow: label("flow", self.flow.key(), self.flow.label()),
            features: self
                .features
                .iter()
                .enumerate()
                .map(|(n, f)| FeatureText {
                    api: f.api,
                    why: text(&format!("features.{n}"), f.why),
                })
                .collect(),
            under_hood: text("under_hood", self.under_hood),
            docs: self.docs.iter().map(|path| doc_link(path)).collect(),
            sources: self
                .sources
                .iter()
                .map(|path| SourceLink {
                    path,
                    github: format!("{REPOSITORY}{path}"),
                })
                .collect(),
        }
    }
}

/// `docs/ui.md#tabs` → the docs site's `/docs/ui#tabs` and the file on GitHub.
fn doc_link(path: &'static str) -> DocLink {
    let (file, anchor) = path.split_once('#').unwrap_or((path, ""));
    let slug = file
        .trim_start_matches("docs/")
        .trim_end_matches(".md")
        .to_owned();
    let anchor = if anchor.is_empty() {
        String::new()
    } else {
        format!("#{anchor}")
    };
    DocLink {
        path,
        site: format!("{DOCS_SITE}{slug}{anchor}"),
        github: format!("{REPOSITORY}{file}{anchor}"),
    }
}

/// A translator for [`Explanation::localize`] from the app's language
/// files (`Lang`, the request's language): the text, or `None` when that
/// language lacks the key.
pub fn translator(lang: renox::Lang) -> impl Fn(&str) -> Option<String> {
    move |key: &str| {
        if lang.locale == "en" {
            return None; // the explanations are written in English
        }
        let text = lang.t(key, &[]);
        (text != key).then_some(text)
    }
}

/// Registers the `about_page(request.route, request.path)` template
/// function the layouts' panel uses: the explanation of the page being
/// shown ([`resolve`]) in the page's language (through the page's own
/// `t()`), or `none`.
pub fn register(env: &mut minijinja::Environment<'static>) {
    env.add_function(
        "about_page",
        |state: &minijinja::State, route: Option<String>, path: Option<String>| -> Value {
            let Some(explanation) = resolve(route.as_deref(), path.as_deref().unwrap_or("")) else {
                return Value::from(());
            };
            let locale = state
                .lookup("app")
                .and_then(|app| app.get_attr("locale").ok())
                .map(|l| l.to_string())
                .unwrap_or_default();
            let t = state.lookup("t");
            let translate = |key: &str| -> Option<String> {
                if locale == "en" {
                    return None;
                }
                let text = t.as_ref()?.call(state, &[Value::from(key)]).ok()?;
                let text = text.to_string();
                (text != key).then_some(text)
            };
            Value::from_serialize(explanation.localize(&translate))
        },
    );
}

//! Renox's documentation site, built with Renox: the guides, the tutorial,
//! the cheat sheet and the changelog of this repository, compiled into one
//! binary and shown with the UI kit.
//!
//! Run it from this directory: `cargo run` (or `rnx serve`), then open
//! <http://127.0.0.1:3000>. Deploying: see README.md.

use renox::prelude::*;
use serde::{Deserialize, Serialize};

pub mod content;
pub mod highlight;
pub mod icons;
pub mod render;

use content::{PAGES, Section};
use render::{RENDERED, REPOSITORY};

/// The course.
pub const LEARN: &str = "https://learn.renox.rs";

/// The files the three sites (docs.renox.rs, renox.rs, learn.renox.rs) serve
/// at the same path, from `public/`: the design system's stylesheet
/// (`tokens.css`, the one place that defines colours and fonts, #545) and its
/// fonts.
pub const SHARED: &[(&str, &[u8])] = &[
    ("tokens.css", include_bytes!("../public/tokens.css")),
    (
        "fonts/inter-latin-wght-4.1.woff2",
        include_bytes!("../public/fonts/inter-latin-wght-4.1.woff2"),
    ),
    (
        "fonts/jetbrains-mono-latin-wght.woff2",
        include_bytes!("../public/fonts/jetbrains-mono-latin-wght.woff2"),
    ),
    (
        "fonts/OFL-Inter.txt",
        include_bytes!("../public/fonts/OFL-Inter.txt"),
    ),
    (
        "fonts/OFL-JetBrainsMono.txt",
        include_bytes!("../public/fonts/OFL-JetBrainsMono.txt"),
    ),
];

/// Routes for [`SHARED`]: `/tokens.css` and `/fonts/{file}`, for renox.rs and
/// learn.renox.rs. A release build also serves them as embedded public files
/// ([`shared`]); these answer in debug builds and tests too, which read
/// `public/` from the disk.
pub fn shared_routes() -> Routes {
    Routes::new()
        .get("/tokens.css", shared_tokens)
        .get("/fonts/{file}", shared_font)
}

fn shared_file(name: &str) -> Result<renox::axum::response::Response> {
    use renox::axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
    use renox::axum::response::IntoResponse;
    let (_, bytes) = SHARED
        .iter()
        .find(|(file, _)| *file == name)
        .ok_or(Error::NotFound)?;
    let kind = match name.rsplit_once('.').map(|(_, ext)| ext) {
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        _ => "text/plain; charset=utf-8",
    };
    Ok((
        [
            (CONTENT_TYPE, kind),
            (CACHE_CONTROL, "public, max-age=3600"),
        ],
        *bytes,
    )
        .into_response())
}

async fn shared_tokens() -> Result<renox::axum::response::Response> {
    shared_file("tokens.css")
}

async fn shared_font(Path(file): Path<String>) -> Result<renox::axum::response::Response> {
    shared_file(&format!("fonts/{file}"))
}

/// What in a site's stylesheet belongs in `tokens.css` instead: a colour
/// literal (`#hex`, `rgb()`, `hsl()`, `white`, `black`) in a declaration, a
/// `@font-face`, or a font family named outside a token (`font-family:
/// "Inter"`; `font-family: var(--mono)` is fine). One line per problem;
/// comments are ignored. The three sites' tests run it on their CSS.
pub fn css_problems(css: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut rest = css;
    while let Some((before, after)) = rest.split_once("/*") {
        plain.push_str(before);
        rest = after.split_once("*/").map_or("", |(_, tail)| tail);
    }
    plain.push_str(rest);
    let mut problems = Vec::new();
    if plain.contains("@font-face") {
        problems.push("@font-face (fonts are declared in tokens.css)".to_owned());
    }
    // A segment followed by `}` holds declarations; one followed by `{` is a selector.
    let mut start = 0;
    for (i, c) in plain.char_indices() {
        if c != '{' && c != '}' {
            continue;
        }
        let segment = &plain[start..i];
        start = i + 1;
        if c == '{' {
            continue;
        }
        for declaration in segment.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let (property, value) = (property.trim(), value.trim());
            let lower = value.to_ascii_lowercase();
            let hex = value.match_indices('#').any(|(j, _)| {
                let digits = value[j + 1..]
                    .chars()
                    .take_while(char::is_ascii_hexdigit)
                    .count();
                (3..=8).contains(&digits)
            });
            let named = lower
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .any(|word| word == "white" || word == "black");
            let function = ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "hwb("]
                .iter()
                .any(|f| lower.contains(f));
            if hex || named || function {
                problems.push(format!("a colour literal: {property}: {value}"));
            }
            let family =
                property == "font-family" && !(value.starts_with("var(") || value == "inherit");
            if family || (property == "font" && value.contains('"')) {
                problems.push(format!(
                    "a font named outside the tokens: {property}: {value}"
                ));
            }
        }
    }
    problems
}

/// `embedded` (an app's `renox::embedded!()`) plus [`SHARED`], so `/tokens.css`
/// and the fonts are served, with `asset()` versions, by renox.rs and
/// learn.renox.rs too. The app's own file wins where both have one.
pub fn shared(embedded: renox::Embedded) -> renox::Embedded {
    let mut public: Vec<(&'static str, &'static [u8])> = SHARED
        .iter()
        .filter(|(name, _)| !embedded.public.iter().any(|(own, _)| own == name))
        .copied()
        .collect();
    public.extend_from_slice(embedded.public);
    renox::Embedded {
        // Built once per app; the slice lives as long as the process.
        public: Box::leak(public.into_boxed_slice()),
        ..embedded
    }
}

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .templates(|env| {
            // `{{ icon("search") }}`: an inline SVG (icons.rs).
            env.add_function("icon", |name: String| {
                renox::minijinja::Value::from_safe_string(icons::svg(&name))
            });
        })
        .share("learn", |_ctx: renox::view::ViewContext| async move {
            Ok(LEARN)
        })
        .module(Docs)
}

struct Docs;

impl Module for Docs {
    fn name(&self) -> &'static str {
        "docs"
    }

    fn routes(&self) -> Routes {
        // The pages change only with a new binary: an ETag lets browsers and
        // proxies keep them, and a repeat visit gets a 304.
        let pages = Routes::new()
            .get("/", home)
            .name("home")
            .get("/docs/{slug}", show)
            .name("docs.show")
            .etag();
        Routes::new()
            .merge(pages)
            .redirect("/docs", "/docs/overview")
            .get("/search", search)
            .name("search")
            // Named `sitemap`: robots.txt points search engines at it.
            .get("/sitemap.xml", sitemap)
            .name("sitemap")
    }
}

/// The sidebar: each section with its pages.
#[derive(Serialize)]
struct NavSection {
    title: &'static str,
    icon: &'static str,
    pages: Vec<NavPage>,
}

#[derive(Serialize)]
struct NavPage {
    slug: &'static str,
    nav: &'static str,
    icon: &'static str,
    blurb: &'static str,
}

fn navigation() -> Vec<NavSection> {
    Section::ALL
        .iter()
        .map(|section| NavSection {
            title: section.title(),
            icon: section.icon(),
            pages: PAGES
                .iter()
                .filter(|page| page.section == *section)
                .map(|page| NavPage {
                    slug: page.slug,
                    nav: page.nav,
                    icon: page.icon,
                    blurb: page.blurb,
                })
                .collect(),
        })
        .collect()
}

/// The quick start on the home page, for the version this site documents
/// (the site shares the workspace's version).
fn quick_start() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        "# 1. Install `rnx`, Renox's command-line tool\n\
         cargo install renox-cli --version {version}\n\
         \n\
         # 2. Make a new app called \"blog\" and go into its folder\n\
         rnx new blog && cd blog\n\
         \n\
         # 3. Run it, then open http://127.0.0.1:3000\n\
         rnx serve\n"
    )
}

async fn home() -> View {
    view(
        "home.html",
        context! {
            nav => navigation(),
            current => "",
            repository => REPOSITORY,
            quick_start => render::code_panel("bash", &quick_start()),
        },
    )
}

async fn show(Path(slug): Path<String>) -> Result<View> {
    let index = PAGES
        .iter()
        .position(|page| page.slug == slug)
        .ok_or(Error::NotFound)?;
    let page = &PAGES[index];
    let rendered = &RENDERED[page.slug];
    let previous = index.checked_sub(1).map(|i| &PAGES[i]);
    let next = PAGES.get(index + 1);
    let link = |page: Option<&content::Page>| {
        page.map(|page| context! { slug => page.slug, nav => page.nav, icon => page.icon })
    };
    Ok(view(
        "page.html",
        context! {
            nav => navigation(),
            current => page.slug,
            title => &rendered.title,
            page_icon => page.icon,
            blurb => page.blurb,
            section => page.section.title(),
            minutes => rendered.minutes,
            html => &rendered.html,
            toc => &rendered.toc,
            source => format!("{REPOSITORY}/blob/main/{}", page.path),
            edit => format!("{REPOSITORY}/edit/main/{}", page.path),
            previous => link(previous),
            next => link(next),
            repository => REPOSITORY,
        },
    ))
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// A page that matched a search, with a line of its text around the match.
#[derive(Serialize)]
struct Hit {
    slug: &'static str,
    icon: &'static str,
    title: String,
    snippet: String,
}

async fn search(Query(query): Query<SearchQuery>) -> View {
    let q = query.q.trim().to_owned();
    let hits = if q.is_empty() { Vec::new() } else { find(&q) };
    view(
        "search.html",
        context! { nav => navigation(), current => "", q, hits, repository => REPOSITORY },
    )
}

/// Pages where every word of `q` appears, the ones with the words in their
/// title first, then by how often they appear.
fn find(q: &str) -> Vec<Hit> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(usize, Hit)> = PAGES
        .iter()
        .filter_map(|page| {
            let rendered = &RENDERED[page.slug];
            let text = rendered.text.to_lowercase();
            let title = rendered.title.to_lowercase();
            if !words
                .iter()
                .all(|w| text.contains(w.as_str()) || title.contains(w.as_str()))
            {
                return None;
            }
            let in_title = words.iter().filter(|w| title.contains(w.as_str())).count();
            let count: usize = words.iter().map(|w| text.matches(w.as_str()).count()).sum();
            Some((
                in_title * 1000 + count,
                Hit {
                    slug: page.slug,
                    icon: page.icon,
                    title: rendered.title.clone(),
                    snippet: snippet(&rendered.text, &words[0]),
                },
            ))
        })
        .collect();
    hits.sort_by_key(|hit| std::cmp::Reverse(hit.0));
    hits.into_iter().map(|(_, hit)| hit).collect()
}

/// About thirty words of `text` around the first that contains `word`.
fn snippet(text: &str, word: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let hit = words
        .iter()
        .position(|w| w.to_lowercase().contains(word))
        .unwrap_or(0);
    let start = hit.saturating_sub(10);
    let end = (hit + 20).min(words.len());
    let mut snippet = words[start..end].join(" ");
    if start > 0 {
        snippet.insert_str(0, "… ");
    }
    if end < words.len() {
        snippet.push_str(" …");
    }
    snippet
}

async fn sitemap(State(state): State<AppState>) -> Result<renox::seo::Sitemap> {
    let mut map = renox::seo::Sitemap::new(&state).route("home", &[], None)?;
    for page in PAGES {
        map = map.route("docs.show", &[&page.slug], None)?;
    }
    Ok(map)
}

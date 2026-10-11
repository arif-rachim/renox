//! learn.renox.rs: a course that builds one Renox app (a coffee shop) stage by stage, with screenshots, videos and challenges
//! solved in an editor. Built with Renox and compiled into one binary
//! (views and public files embedded).
//!
//! The plan is epic #538. This crate is its skeleton (#523): the layout, the
//! course's outline on the home page, the sitemap and `llms.txt`.
//!
//! Run it from this directory: `cargo run` (or `rnx serve`), then open
//! <http://127.0.0.1:3000>. Deploying: README.md.

use renox::axum::http::header::CONTENT_TYPE;
use renox::axum::response::{IntoResponse, Response};
use renox::prelude::*;
use serde_json::json;

pub mod challenge;
pub mod code;
pub mod content;
pub mod course;
pub mod design;
pub mod directive;
pub mod media;
pub mod render;

/// The site's own address (canonical links, JSON-LD) when `APP_URL` isn't
/// set to it.
pub const SITE: &str = "https://learn.renox.rs";
/// The documentation site.
pub const DOCS: &str = "https://docs.renox.rs";
/// The landing page.
pub const WWW: &str = "https://renox.rs";
/// The source.
pub const REPOSITORY: &str = "https://github.com/arif-rachim/renox";

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        // What every view gets: the links that appear in the layout.
        .share("links", |_ctx: renox::view::ViewContext| async move {
            Ok(json!({ "docs": DOCS, "www": WWW, "github": REPOSITORY }))
        })
        .module(Learn)
}

struct Learn;

impl Module for Learn {
    fn name(&self) -> &'static str {
        "learn"
    }

    fn routes(&self) -> Routes {
        // The pages change only with a new binary: ETags let browsers and
        // proxies keep them.
        let pages = Routes::new()
            .get("/", home)
            .name("home")
            .get("/design", design::page)
            .name("design")
            .get("/stages/{slug}", stage)
            .name("stage")
            .etag();
        Routes::new()
            .merge(pages)
            .get("/sitemap.xml", sitemap)
            .name("sitemap")
            .get("/llms.txt", llms)
            .name("llms")
    }
}

/// This site's base URL: `APP_URL` without a trailing slash.
fn base(state: &AppState) -> String {
    let url = state.config.url.trim_end_matches('/');
    if url.is_empty() {
        SITE.to_owned()
    } else {
        url.to_owned()
    }
}

/// A JSON-LD block, safe inside `<script>` (no `</`).
fn json_ld(value: serde_json::Value) -> String {
    value.to_string().replace("</", "<\\/")
}

async fn home(State(state): State<AppState>) -> Result<View> {
    let base = base(&state);
    let chapters = content::chapters();
    let workload: u32 = chapters.iter().map(|c| c.minutes).sum();
    let sections = course::STAGES
        .iter()
        .map(|s| {
            let mut section = json!({
                "@type": "Syllabus",
                "name": format!("Stage {}: {}", s.number, s.title),
                "description": s.summary,
            });
            if let Some(c) = chapters.iter().find(|c| c.slug == s.slug) {
                section["url"] = json!(format!("{base}/stages/{}", c.slug));
                section["timeRequired"] = json!(format!("PT{}M", c.minutes));
            }
            section
        })
        .collect::<Vec<_>>();
    let ld = json!({
        "@context": "https://schema.org",
        "@type": "Course",
        "name": "Learn Renox",
        "description": "Build one web app in Rust with Renox, a coffee shop, stage by stage: migrations, models, pages, components, forms, live components and tests.",
        "url": format!("{base}/"),
        "inLanguage": "en",
        "isAccessibleForFree": true,
        "educationalLevel": "Beginner",
        "coursePrerequisites": "Basic Rust: structs, enums, traits and async functions.",
        "offers": { "@type": "Offer", "category": "Free", "price": 0, "priceCurrency": "USD" },
        "hasCourseInstance": {
            "@type": "CourseInstance",
            "courseMode": "Online",
            "courseWorkload": format!("PT{workload}M"),
        },
        "provider": { "@type": "Organization", "name": "Renox", "url": WWW },
        "syllabusSections": sections,
    });
    Ok(view(
        "home.html",
        context! {
            stages => course::STAGES,
            chapters => content::chapters().iter().map(|c| c.slug).collect::<Vec<_>>(),
            json_ld => json_ld(ld),
        },
    ))
}

/// A neighbouring stage, for the pager.
#[derive(serde::Serialize)]
struct Neighbour {
    slug: &'static str,
    number: u8,
    title: &'static str,
}

/// `GET /stages/{slug}`: a chapter. An unknown slug, or a stage whose
/// chapter isn't written yet, is a 404.
async fn stage(State(state): State<AppState>, Path(slug): Path<String>) -> Result<Response> {
    // `/stages/{slug}.md`: the chapter as Markdown, for language models.
    if let Some(slug) = slug.strip_suffix(".md") {
        let chapter = content::find(slug).ok_or(Error::NotFound)?;
        let text = format!(
            "# Stage {}: {}\n\n> {}\n\n{}\n",
            chapter.number, chapter.title, chapter.summary, chapter.markdown
        );
        return Ok(([(CONTENT_TYPE, "text/markdown; charset=utf-8")], text).into_response());
    }
    let base = base(&state);
    let chapter = content::find(&slug).ok_or(Error::NotFound)?;
    let neighbour = |number: Option<u8>| {
        let stage = course::STAGES.get(usize::from(number?))?;
        Some(Neighbour {
            slug: stage.slug,
            number: stage.number,
            title: stage.title,
        })
    };
    let written = |number: Option<u8>| {
        neighbour(number).filter(|n| content::chapters().iter().any(|c| c.slug == n.slug))
    };
    let prev = written(chapter.number.checked_sub(1));
    let next_number = chapter.number.checked_add(1);
    let next = written(next_number);
    // The outline has a next stage but no chapter for it yet.
    let soon = if next.is_none() {
        neighbour(next_number)
    } else {
        None
    };
    let guide = course::STAGES
        .get(usize::from(chapter.number))
        .map(|s| s.guide);
    let name = format!("Stage {}: {}", chapter.number, chapter.title);
    let url = format!("{base}/stages/{}", chapter.slug);
    let topics = course::STAGES
        .get(usize::from(chapter.number))
        .map(|s| s.topics)
        .unwrap_or_default();
    let ld = json!([
        {
            "@context": "https://schema.org",
            "@type": "LearningResource",
            "name": name,
            "description": chapter.summary,
            "url": url,
            "learningResourceType": "Tutorial",
            "educationalLevel": "Beginner",
            "inLanguage": "en",
            "timeRequired": format!("PT{}M", chapter.minutes),
            "position": chapter.number,
            "teaches": topics,
            "isPartOf": { "@type": "Course", "name": "Learn Renox", "url": format!("{base}/") },
            "provider": { "@type": "Organization", "name": "Renox", "url": WWW },
        },
        {
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            "itemListElement": [
                { "@type": "ListItem", "position": 1, "name": "Course", "item": format!("{base}/") },
                { "@type": "ListItem", "position": 2, "name": name, "item": url },
            ],
        },
    ]);
    // The chapter's HTML names its videos' address by a placeholder, filled in
    // here so `LEARN_MEDIA_URL` need not be known when chapters are rendered.
    let mut chapter_json = serde_json::to_value(chapter)?;
    if let Some(html) = chapter_json["html"].as_str() {
        chapter_json["html"] = json!(html.replace(media::PLACEHOLDER, &media::media_url(&state)));
    }
    Ok(view(
        "stage.html",
        context! {
            chapter => chapter_json,
            page_title => format!("Stage {}: {} · Learn Renox", chapter.number, chapter.title),
            guide => guide,
            prev => prev,
            next => next,
            soon => soon,
            json_ld => json_ld(ld),
        },
    )
    .into_response())
}

/// Every page for search engines.
async fn sitemap(State(state): State<AppState>) -> Result<renox::seo::Sitemap> {
    let mut map = renox::seo::Sitemap::new(&state).route("home", &[], None)?;
    for c in content::chapters() {
        map = map.route("stage", &[&c.slug], None)?;
    }
    Ok(map)
}

/// `llms.txt` (llmstxt.org): what the course is and what each stage teaches.
async fn llms(State(state): State<AppState>) -> Response {
    let base = base(&state);
    let mut text = String::from(
        "# Learn Renox\n\n> A course that builds one web app in Rust with Renox (a coffee shop), stage by stage, from `rnx new` to tested live components.\n\n",
    );
    text.push_str(&format!(
        "- Course: {base}/\n- Docs: {DOCS}\n- Source: {REPOSITORY}\n\n## Stages\n\n"
    ));
    for s in course::STAGES {
        text.push_str(&format!(
            "- Stage {}: {}: {} Guide: {DOCS}/docs/{}",
            s.number, s.title, s.summary, s.guide
        ));
        if let Some(c) = content::chapters().iter().find(|c| c.slug == s.slug) {
            text.push_str(&format!(" Chapter: {base}/stages/{}.md", c.slug));
        }
        text.push('\n');
    }
    ([(CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response()
}

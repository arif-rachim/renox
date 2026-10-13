//! The design system's style guide: `GET /design` (noindex, not in the
//! sitemap) shows every component with sample content, in the reader's mode.

use renox::prelude::*;
use renox_site::render;

/// Every component of the design system, in `/design`'s order. An issue that
/// adds a component adds its name here and a `data-component` section to
/// `design.html`; the page's test fails until both agree. The browser test
/// (`tests/browser/learn-design.test.mjs`) keeps a copy of the count.
pub const COMPONENTS: &[&str] = &[
    "tokens",
    "type",
    "header",
    "footer",
    "theme-toggle",
    "eyebrow",
    "buttons",
    "hero",
    "section",
    "stage-card",
    "continue",
    "reset",
    "crumbs",
    "chapter-head",
    "chapter",
    "toc",
    "prose",
    "kbd",
    "code-panel",
    "code-file",
    "changes",
    "callouts",
    "shot",
    "video",
    "quiz",
    "challenge",
    "done-button",
    "going-deeper",
    "pager",
    "problem",
    "placeholder",
];

const RUST: &str = r#"/// One drink on the menu.
#[derive(Model, Serialize)]
pub struct MenuItem {
    pub id: i64,
    pub name: String,
    // The price in cents.
    pub price: i64,
    pub stock: Option<i64>,
}

impl MenuItem {
    pub fn is_cheap(&self) -> bool {
        self.price < 500
    }
}"#;

const BASH: &str = "# Create the app, then run it.\nrnx new coffee\ncd coffee\nrnx serve";

const TEXT: &str = "running 2 tests\ntest menu::cheap_items_are_listed ... ok\ntest menu::sold_out_items_are_hidden ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored";

/// The callouts, written as a chapter writes them: the page shows the
/// renderer's own output.
const CALLOUTS: &str = "> [!NOTE]\n> A drink is a `MenuItem` with a price in cents.\n\n> [!TIP]\n> Run `rnx serve` in a second terminal while you work.\n\n> [!IMPORTANT]\n> Migrations run in the order of their names.\n\n> [!WARNING]\n> A sold-out item still has a row.\n\n> [!CAUTION]\n> `migrate:fresh` drops every table.\n\n> A plain quote, for words that are someone else's.\n";

/// The samples of `code-file` (stage 1) and `changes` (stage 2), written as a
/// chapter writes them (the file name gives the stage).
const CODE_FILE: &str =
    "```learn:include\nfile: migrations/20261012090000_create_categories_table.up.sql\n```\n";
const CHANGES: &str = "```learn:changes\n```\n";
/// The sample of `challenge` (stage 2): the real `cheap-items` section.
const CHALLENGE: &str = "```learn:challenge\nid: cheap-items\n```\n";

/// A manifest entry for the second `video` sample: the directive's own figure
/// from a test entry. Its poster is a file the design page already has, so the
/// page makes no failing request (the sources load only on play).
const VIDEO_ENTRY: &str = r#"{"version":1,"videos":{"s00-design":{"stage":0,"title":"Sample","width":1280,"height":720,"duration":8.0,
  "poster":{"file":"screen.svg","bytes":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},
  "sources":[{"file":"design.webm","type":"video/webm","bytes":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},
             {"file":"design.mp4","type":"video/mp4","bytes":1,"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}],
  "scene_sha256":"x","recorded_at":"2026-10-11"}}}"#;

/// `GET /design`.
pub async fn page() -> Result<View> {
    let (callouts, ..) = crate::render::render(CALLOUTS, "design", 0);
    let chapter = "content/stages/01-design.md";
    let (code_file, ..) = crate::render::render(CODE_FILE, chapter, 0);
    // Stage 2 changes `mod app;` into `pub mod app;`: a removed line to show.
    let (changes, ..) = crate::render::render(CHANGES, "content/stages/02-design.md", 0);
    let (challenge, ..) = crate::render::render(CHALLENGE, "content/stages/02-design.md", 0);
    // The same section as a reader who marked it sees it (progress.js sets these).
    let passed = challenge
        .replace("s02/cheap-items", "design/passed")
        .replace("learn-challenge\"", "learn-challenge is-passed\"")
        .replace("aria-pressed=\"false\"", "aria-pressed=\"true\"")
        .replace("id=\"challenge-cheap-items\"", "id=\"challenge-passed\"");
    let challenge = challenge.replace("s02/cheap-items", "design/plain");
    let sample = crate::media::parse(VIDEO_ENTRY).expect("the sample entry is valid");
    let video = crate::media::figure(
        &sample.videos["s00-design"],
        "A recording of the menu page, drawn from a manifest entry",
        Some("The same figure the video directive draws, from a manifest entry"),
        "/design",
    );
    Ok(view(
        "design.html",
        context! {
            components => COMPONENTS,
            video => video,
            code_rust => render::code_panel("rust", RUST),
            code_bash => render::code_panel("bash", BASH),
            code_text => render::code_panel("text", TEXT),
            callouts => callouts,
            code_file => code_file,
            changes => changes,
            challenge => challenge,
            challenge_passed => passed,
        },
    ))
}

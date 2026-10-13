//! The content pipeline: chapters compile without problems, a bad directive
//! is a problem with its file and line, and every chapter renders on its page.

use renox::testing::TestApp;
use renox_learn::directive::Directive;
use renox_learn::{code, content, course};

async fn site() -> TestApp {
    TestApp::with_config(renox_learn::app(), |c| {
        c.url = "https://learn.renox.rs".into()
    })
    .await
}

/// The article of a stage page.
fn article(html: &str) -> &str {
    let start = html.find("<article class=\"prose\"").expect("an article");
    let rest = &html[start..];
    &rest[..rest.find("</article>").expect("the article ends")]
}

#[renox::test]
async fn the_content_has_no_problems() {
    let problems = content::problems();
    assert!(
        problems.is_empty(),
        "{}",
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[renox::test]
async fn a_bad_directive_is_a_problem_with_its_file_and_line() {
    const TEXT: &str = "---\ntitle: Demo\nstage: 99\nsummary: S\nminutes: 5\n---\nIntro.\n\n```learn:quiz\nid: q\nquestion: Q?\nchoice: a\nchoice: b\n```\n\nAfter.\n";
    let (chapter, problems) = content::parse("99-demo.md", TEXT);
    let chapter = chapter.expect("the front matter is fine");
    let quiz = problems
        .iter()
        .find(|p| p.line == 9)
        .unwrap_or_else(|| panic!("no problem on line 9: {problems:?}"));
    assert_eq!(quiz.file, "content/stages/99-demo.md");
    assert_eq!(
        quiz.to_string(),
        "content/stages/99-demo.md:9: learn:quiz needs exactly one `correct:` (found 0)"
    );
    assert!(chapter.html.contains("class=\"learn-problem\""));
    assert!(chapter.html.contains("role=\"alert\""));
    assert!(chapter.html.contains("After."), "the rest still renders");
    // The file name doesn't match the outline either.
    assert!(
        problems
            .iter()
            .any(|p| p.line == 1 && p.message.starts_with("the file name must be NN-slug.md"))
    );
}

#[renox::test]
async fn front_matter_and_headings_are_checked() {
    let (chapter, problems) = content::parse("00-setup.md", "no front matter");
    assert!(chapter.is_none());
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].message, "no front matter (start with ---)");

    let (chapter, problems) = content::parse(
        "00-setup.md",
        "---\ntitle: Other\nstage: 0\nsummary: Other.\nminutes: 5\n---\n# A title\n",
    );
    assert!(chapter.is_some());
    let messages: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        messages.iter().any(|m| m
            == "content/stages/00-setup.md:2: `title` is `Other`, the outline (course.rs) says `Set up and run an app`"),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m
            == "content/stages/00-setup.md:7: a chapter's headings start at ##: its title is the page's <h1>"),
        "{messages:?}"
    );
}

#[renox::test]
async fn kbd_passes_through_and_other_html_is_text() {
    let (html, ..) = renox_learn::render::render(
        "Press <kbd>Ctrl</kbd> and <script>alert(1)</script>.\n\n[docs](https://docs.renox.rs)\n",
        "content/stages/00-setup.md",
        0,
    );
    assert!(html.contains("<kbd>Ctrl</kbd>"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
    assert!(html.contains("rel=\"noopener\""), "{html}");
}

#[renox::test]
async fn the_chapters_are_numbered_from_0_without_gaps() {
    let chapters = content::chapters();
    assert!(!chapters.is_empty());
    for (i, chapter) in chapters.iter().enumerate() {
        assert_eq!(
            usize::from(chapter.number),
            i,
            "{} is out of order",
            chapter.file
        );
        let stage = &course::STAGES[i];
        assert_eq!(chapter.slug, stage.slug);
        assert_eq!(chapter.title, stage.title);
        assert_eq!(chapter.summary, stage.summary);
    }
}

#[renox::test]
async fn every_chapter_renders_in_the_layout() {
    let app = site().await;
    for chapter in content::chapters() {
        let path = format!("/stages/{}", chapter.slug);
        let page = app.get(&path).await;
        page.assert_ok()
            .assert_view("stage.html")
            .assert_see(&format!(
                "<title>Stage {}: {} · Learn Renox</title>",
                chapter.number, chapter.title
            ))
            .assert_see(&format!(
                "<link rel=\"canonical\" href=\"https://learn.renox.rs{path}\">"
            ));
        assert!(page.header("etag").is_some(), "{path} has no ETag");
        assert_eq!(page.text().matches("<h1").count(), 1, "{path}: one h1");
    }
}

#[renox::test]
async fn every_heading_has_an_id_and_the_toc_points_at_them() {
    let app = site().await;
    for chapter in content::chapters() {
        let html = app.get(&format!("/stages/{}", chapter.slug)).await.text();
        let body = article(&html);
        for tag in ["<h2", "<h3"] {
            for part in body.split(tag).skip(1) {
                let open = part.split('>').next().unwrap();
                assert!(
                    open.contains(" id=\""),
                    "{}: {tag} without an id",
                    chapter.slug
                );
            }
        }
        for heading in &chapter.toc {
            assert!(
                body.contains(&format!("id=\"{}\"", heading.id)),
                "{}: no heading with id {}",
                chapter.slug,
                heading.id
            );
            assert!(html.contains(&format!("<a href=\"#{}\">", heading.id)));
        }
    }
}

#[renox::test]
async fn the_pager_links_the_neighbouring_chapters() {
    let app = site().await;
    let chapters = content::chapters();
    for (i, chapter) in chapters.iter().enumerate() {
        let html = app.get(&format!("/stages/{}", chapter.slug)).await.text();
        let prev = html.contains("rel=\"prev\"");
        assert_eq!(prev, i > 0, "{}: previous link", chapter.slug);
        if let Some(next) = chapters.get(i + 1) {
            assert!(html.contains(&format!("rel=\"next\" href=\"/stages/{}\"", next.slug)));
            assert!(!html.contains("pager__soon"));
        } else if let Some(stage) = course::STAGES.get(i + 1) {
            assert!(!html.contains("rel=\"next\""));
            assert!(html.contains(&format!(
                "Next: Stage {}, {}, coming soon.",
                stage.number, stage.title
            )));
        }
        if i > 0 {
            assert!(html.contains(&format!(
                "rel=\"prev\" href=\"/stages/{}\"",
                chapters[i - 1].slug
            )));
        }
    }
}

#[renox::test]
async fn the_home_page_links_each_written_chapter() {
    let app = site().await;
    let html = app.get("/").await.text();
    for stage in course::STAGES {
        let link = format!("href=\"/stages/{}\"", stage.slug);
        let written = content::find(stage.slug).is_some();
        assert_eq!(html.contains(&link), written, "{}", stage.slug);
    }
    let unwritten = course::STAGES
        .iter()
        .any(|stage| content::find(stage.slug).is_none());
    assert_eq!(
        html.contains("Coming soon"),
        unwritten,
        "stages without a chapter still say so"
    );
}

#[renox::test]
async fn an_unknown_stage_is_a_404() {
    let app = site().await;
    app.get("/stages/no-such-stage").await.assert_not_found();
    // A stage in the outline without a chapter yet.
    let missing = course::STAGES
        .iter()
        .find(|s| content::find(s.slug).is_none());
    if let Some(stage) = missing {
        app.get(&format!("/stages/{}", stage.slug))
            .await
            .assert_not_found();
    }
}

#[renox::test]
async fn the_quiz_is_rendered_with_its_correct_choice_marked() {
    let app = site().await;
    let html = app.get("/stages/setup").await.text();
    let body = article(&html);
    assert!(body.contains("data-quiz=\"where-it-runs\""));
    assert!(body.contains("id=\"quiz-where-it-runs\""));
    assert_eq!(body.matches("data-correct").count(), 1);
    assert!(body.contains("value=\"1\" data-correct> <span>http://127.0.0.1:3000</span>"));
    assert!(body.contains("learn-quiz__check\" disabled>Check</button>"));
    assert!(
        body.contains("<code>rnx serve</code>"),
        "the question is Markdown"
    );
}

#[renox::test]
async fn the_design_page_uses_the_renderers_markup() {
    const SAMPLE: &str = "---\ntitle: Demo\nstage: 2\nsummary: S\nminutes: 5\n---\n## A heading\n\n> [!TIP]\n> A tip.\n\n```rust\nlet x = 1;\n```\n\n```learn:screenshot\nfile: menu.webp\npage: /menu\nalt: The menu\n```\n\n```learn:quiz\nid: q\nquestion: Q?\nchoice: a\ncorrect: b\nexplain: E\n```\n\n```learn:bad\n```\n\n```learn:challenge\nid: cheap-items\n```\n";
    let (chapter, _) = content::parse("02-models.md", SAMPLE);
    let rendered = chapter.expect("a chapter").html;
    let design = site().await.get("/design").await.text();
    for class in [
        "site-anchor",
        "site-callout site-callout--tip",
        "site-callout__title",
        "site-code__bar",
        "site-code__lang",
        "site-code__copy-label",
        "learn-shot",
        "learn-quiz",
        "learn-quiz__question",
        "learn-quiz__choice",
        "learn-quiz__result",
        "learn-quiz__explain",
        "learn-quiz__check",
        "learn-problem",
        "learn-challenge",
    ] {
        assert!(
            rendered.contains(class),
            "the renderer no longer writes {class}"
        );
    }
    for class in [
        "site-callout site-callout--tip",
        "site-callout__title",
        "site-code__bar",
        "site-code__lang",
        "site-code__copy-label",
        "learn-shot",
        "learn-quiz",
        "learn-quiz__question",
        "learn-quiz__choice",
        "learn-quiz__result",
        "learn-quiz__explain",
        "learn-quiz__check",
        "learn-problem",
        "learn-challenge",
    ] {
        assert!(design.contains(class), "/design doesn't show {class}");
    }
}

#[test]
fn regions_have_no_problems() {
    assert!(code::PROBLEMS.is_empty(), "{}", code::PROBLEMS.join("\n"));
}

/// The (stage, region) and (stage, file) of every include in the chapters.
/// A stage and a name or path.
type Found = Vec<(u8, String)>;

fn includes() -> (Found, Found) {
    let (mut regions, mut files) = (Vec::new(), Vec::new());
    for chapter in content::chapters() {
        for d in &chapter.directives {
            if let Directive::Include(i) = d {
                let stage = i.stage.unwrap_or(chapter.number);
                if let Some(r) = &i.region {
                    regions.push((stage, r.clone()));
                }
                if let Some(f) = &i.file {
                    files.push((stage, f.clone()));
                }
            }
        }
    }
    (regions, files)
}

#[test]
fn every_include_resolves() {
    let (regions, files) = includes();
    assert!(!regions.is_empty() && !files.is_empty());
    for (stage, name) in &regions {
        assert!(
            code::region(*stage, name).is_some(),
            "stage {stage}: region {name}"
        );
    }
    for (stage, path) in &files {
        assert!(
            code::file(*stage, path).is_some(),
            "stage {stage}: file {path}"
        );
    }
}

#[test]
fn every_region_is_included_by_a_chapter() {
    let (regions, _) = includes();
    let written: Vec<u8> = content::chapters().iter().map(|c| c.number).collect();
    for region in code::REGIONS.iter().filter(|r| written.contains(&r.stage)) {
        assert!(
            regions.contains(&(region.stage, region.name.to_owned())),
            "no chapter includes stage {}'s region {}",
            region.stage,
            region.name
        );
    }
}

#[test]
fn regions_are_cut_without_their_markers() {
    let r = code::region(1, "menu.item-model").expect("a region");
    assert_eq!(r.path, "src/app/menu/model.rs");
    assert_eq!(r.language, "rust");
    assert!(
        r.text.starts_with("/// One thing on the menu"),
        "{}",
        r.text
    );
    assert!(r.text.ends_with('}'));
    assert!(!r.text.contains("learn:"));
    assert!(r.first_line < r.last_line);
    for file in code::FILES {
        assert!(!file.text.contains("[learn:"), "{}", file.path);
    }
    // Stage 0 is `rnx new` untouched.
    assert!(code::REGIONS.iter().all(|r| r.stage != 0));
}

#[test]
fn changes_list_the_new_migrations() {
    let added: Vec<&str> = code::changes(1)
        .filter(|c| c.kind == "added")
        .map(|c| c.path)
        .collect();
    for path in [
        "migrations/20261012090000_create_categories_table.up.sql",
        "migrations/20261012090100_create_menu_items_table.up.sql",
    ] {
        assert!(added.contains(&path), "{path} is not listed in {added:?}");
    }
    assert_eq!(
        code::changes(0).count(),
        0,
        "stage 0 has no stage before it"
    );
    let lib = code::changes(1)
        .find(|c| c.path == "src/lib.rs")
        .expect("lib.rs changed");
    assert_eq!(lib.kind, "changed");
    assert!(
        lib.diff.contains("+        .module(app::menu::Menu)"),
        "{}",
        lib.diff
    );
    assert!(!lib.diff.contains("learn:"), "marker lines don't show");
}

#[renox::test]
async fn the_migrations_chapter_shows_what_changed() {
    let html = site().await.get("/stages/migrations").await.text();
    assert!(html.contains("class=\"learn-code\""), "included code");
    assert!(html.contains("class=\"learn-change\""), "the changes");
    assert!(html.contains("learn-change__kind rx-badge rx-badge--success\">added</span>"));
    assert!(html.contains("<span class=\"add\">+"));
}

#[test]
fn an_include_that_does_not_resolve_is_a_problem() {
    const TEXT: &str = "---\ntitle: Migrations\nstage: 1\nsummary: Create the categories table with plain SQL, then let Renox write the menu items table from the model.\nminutes: 20\n---\n## A\n\n```learn:include\nregion: no.such\n```\n\n```learn:include\nfile: src/nothing.rs\nstage: 2\n```\n";
    let (chapter, problems) = content::parse("01-migrations.md", TEXT);
    let messages: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert_eq!(
        messages,
        [
            "content/stages/01-migrations.md:9: learn:include: stage 1 has no region `no.such`",
            "content/stages/01-migrations.md:13: learn:include: stage 2 has no file `src/nothing.rs`",
        ]
    );
    assert!(chapter.unwrap().html.contains("class=\"learn-problem\""));
}

#[test]
fn changes_in_stage_0_is_a_problem() {
    const TEXT: &str = "---\ntitle: Set up and run an app\nstage: 0\nsummary: Install rnx, create the coffee app and see it answer in the browser.\nminutes: 10\n---\n```learn:changes\n```\n";
    let (_, problems) = content::parse("00-setup.md", TEXT);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(
        problems[0].message,
        "learn:changes: stage 0 has no stage before it"
    );
}

#[test]
fn chapters_have_no_pasted_code() {
    for chapter in content::chapters() {
        for line in chapter.markdown.lines() {
            let fence = line.trim_start().strip_prefix("```").unwrap_or("");
            let language = fence.split([',', ' ']).next().unwrap_or("");
            assert!(
                !matches!(language, "rust" | "rs" | "html" | "sql" | "toml"),
                "{}: Rust, HTML, SQL and TOML come from the course app (`learn:include`): {line}",
                chapter.file
            );
        }
    }
}

#[test]
fn chapters_are_the_right_length() {
    for chapter in content::chapters() {
        // Prose only: code, directives and fenced output don't count.
        let mut in_fence = false;
        let mut words = 0;
        for line in chapter.markdown.lines() {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
            } else if !in_fence {
                words += line.split_whitespace().count();
            }
        }
        assert!(
            (900..=1800).contains(&words),
            "{} has {words} words of prose, between 900 and 1,800 is the rule",
            chapter.file
        );
    }
}

/// Every screenshot directive with its chapter's stage number.
fn screenshots() -> Vec<(u8, renox_learn::directive::Screenshot)> {
    let mut found = Vec::new();
    for chapter in content::chapters() {
        for d in &chapter.directives {
            if let Directive::Screenshot(s) = d {
                found.push((chapter.number, s.clone()));
            }
        }
    }
    found
}

fn images_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("public/img")
}

#[test]
fn every_screenshot_directive_has_its_image() {
    let shots = screenshots();
    assert!(!shots.is_empty(), "the chapters have no screenshots");
    for (stage, shot) in shots {
        let path = images_dir().join(format!("s{stage:02}")).join(&shot.file);
        let bytes = std::fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing: run node learn/media/screens.mjs {stage}",
                path.display()
            )
        });
        assert!(
            bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
            "{} is not a webp file",
            path.display()
        );
        assert!(
            bytes.len() <= 300 * 1024,
            "{} is {} KB, over the 300 KB budget",
            path.display(),
            bytes.len() / 1024
        );
    }
}

#[test]
fn every_image_is_used() {
    let used: Vec<String> = screenshots()
        .into_iter()
        .map(|(stage, shot)| format!("s{stage:02}/{}", shot.file))
        .collect();
    let mut found = 0;
    for dir in std::fs::read_dir(images_dir()).expect("learn/public/img") {
        let dir = dir.expect("an entry").path();
        for file in std::fs::read_dir(&dir).expect("a stage folder") {
            let file = file.expect("an entry").path();
            if file.extension().is_none_or(|e| e != "webp") {
                continue;
            }
            found += 1;
            let name = format!(
                "{}/{}",
                dir.file_name().unwrap().to_string_lossy(),
                file.file_name().unwrap().to_string_lossy()
            );
            assert!(used.contains(&name), "{name} is named by no directive");
        }
    }
    assert!(found > 0, "no images under learn/public/img");
}

#[test]
fn a_screenshot_without_alt_is_a_problem() {
    let (_, problems) = content::parse(
        "00-setup.md",
        "---\ntitle: Demo\nstage: 0\nsummary: S\nminutes: 5\n---\n```learn:screenshot\nfile: a.webp\npage: /\n```\n",
    );
    assert!(
        problems
            .iter()
            .any(|p| p.line == 7 && p.message == "learn:screenshot needs `alt`"),
        "{problems:?}"
    );
}

#[test]
fn a_screenshot_with_a_bad_page_or_viewport_is_a_problem() {
    for (body, message) in [
        ("page: menu\nalt: A", "page `menu`"),
        ("page: /\nviewport: tablet\nalt: A", "viewport `tablet`"),
        ("alt: A", "needs `page`"),
    ] {
        let text = format!(
            "---\ntitle: Demo\nstage: 0\nsummary: S\nminutes: 5\n---\n```learn:screenshot\nfile: a.webp\n{body}\n```\n"
        );
        let (_, problems) = content::parse("00-setup.md", &text);
        assert!(
            problems.iter().any(|p| p.message.contains(message)),
            "{message}: {problems:?}"
        );
    }
}

#[test]
fn the_figure_has_width_and_height() {
    const TEXT: &str = "---\ntitle: Demo\nstage: 0\nsummary: S\nminutes: 5\n---\n```learn:screenshot\nfile: a.webp\npage: /menu\nalt: \"A <b>menu</b>\"\ncaption: The `menu`.\n```\n\n```learn:screenshot\nfile: b.webp\npage: /menu\nviewport: phone\nalt: Phone\n```\n";
    let (chapter, problems) = content::parse("00-setup.md", TEXT);
    assert!(
        !problems.iter().any(|p| p.message.contains("screenshot")),
        "{problems:?}"
    );
    let html = chapter.expect("a chapter").html;
    assert!(
        html.contains("<figure class=\"learn-shot\"><img src=\"/img/s00/a.webp\" alt=\"A &lt;b&gt;menu&lt;/b&gt;\" width=\"1280\" height=\"800\" loading=\"lazy\" decoding=\"async\"><figcaption>The <code>menu</code>.</figcaption></figure>"),
        "{html}"
    );
    assert!(html.contains("<figure class=\"learn-shot learn-shot--phone\"><img src=\"/img/s00/b.webp\" alt=\"Phone\" width=\"390\" height=\"844\""));
}

/// Every `id` of a `learn:video` in the chapters.
fn video_ids() -> std::collections::BTreeSet<String> {
    content::chapters()
        .iter()
        .flat_map(|c| &c.directives)
        .filter_map(|d| match d {
            Directive::Video(v) => Some(v.id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_video_directive_has_a_manifest_entry() {
    // A missing entry is also a problem on the page (`the_content_has_no_problems`).
    let manifest = renox_learn::media::manifest();
    for id in video_ids() {
        assert!(
            manifest.videos.contains_key(&id),
            "learn:video `{id}` is not in content/media.json"
        );
    }
}

#[test]
fn every_manifest_entry_is_used_and_has_a_scene() {
    let used = video_ids();
    let scenes = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("media/scenes");
    for (id, video) in &renox_learn::media::manifest().videos {
        assert!(
            used.contains(id),
            "`{id}` is in media.json but no chapter uses it"
        );
        assert!(
            scenes.join(format!("{id}.json")).is_file(),
            "`{id}` has no scene file media/scenes/{id}.json"
        );
        assert!(
            id.starts_with(&format!("s{:02}-", video.stage)),
            "`{id}` doesn't start with its stage ({})",
            video.stage
        );
    }
}

#[test]
fn manifest_files_are_named_after_their_id() {
    for (id, video) in &renox_learn::media::manifest().videos {
        assert_eq!(video.poster.file, format!("{id}.webp"));
        assert!(video.poster.mime.is_none(), "{id}: a poster has no type");
        let files: Vec<_> = video
            .sources
            .iter()
            .map(|s| (s.file.as_str(), s.mime.as_deref()))
            .collect();
        assert_eq!(
            files,
            [
                (format!("{id}.webm").as_str(), Some("video/webm")),
                (format!("{id}.mp4").as_str(), Some("video/mp4"))
            ],
            "{id}: sources are webm then mp4"
        );
        for f in std::iter::once(&video.poster).chain(&video.sources) {
            assert_eq!(f.sha256.len(), 64, "{}: a SHA-256 in hex", f.file);
            assert!(f.bytes > 0, "{}: no bytes", f.file);
        }
    }
}

#[test]
fn an_unknown_video_id_is_a_problem_where_it_is() {
    const TEXT: &str = "---\ntitle: Demo\nstage: 0\nsummary: S\nminutes: 5\n---\n```learn:video\nid: s00-nothing\ntitle: T\n```\n";
    let (_, problems) = content::parse("00-setup.md", TEXT);
    assert!(
        problems.iter().any(|p| p.line == 7
            && p.message
                .contains("`s00-nothing` is not in content/media.json")),
        "{problems:?}"
    );
}

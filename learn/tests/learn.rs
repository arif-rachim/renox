//! learn.renox.rs: every route answers, the SEO tags and the structured data
//! are there, the course's outline is in the HTML, and every link to
//! docs.renox.rs names a page that exists.

use renox::testing::TestApp;

async fn site() -> TestApp {
    TestApp::with_config(renox_learn::app(), |c| {
        c.url = "https://learn.renox.rs".into()
    })
    .await
}

/// Every `https://docs.renox.rs/docs/{slug}` in `html`.
fn doc_links(html: &str) -> Vec<String> {
    html.split("https://docs.renox.rs/docs/")
        .skip(1)
        .map(|rest| {
            rest.split(['"', '#', '?', ')', ' ', '<', '\n'])
                .next()
                .unwrap_or("")
                .to_owned()
        })
        .collect()
}

#[renox::test]
async fn the_home_page_lists_every_stage_with_its_tags_and_structured_data() {
    let app = site().await;
    let page = app.get("/").await;
    page.assert_ok()
        .assert_see("<title>Learn Renox: build a web app in Rust, stage by stage</title>")
        .assert_see("<meta name=\"description\" content=\"A course that builds one Renox app")
        .assert_see("<link rel=\"canonical\" href=\"https://learn.renox.rs/\">")
        .assert_see("\"@type\":\"Course\"")
        .assert_see("\"@type\":\"Syllabus\"")
        .assert_see("<a href=\"/\" aria-current=\"page\">Course</a>");
    for stage in renox_learn::course::STAGES {
        page.assert_see(&format!("id=\"{}\"", stage.slug));
    }
    let html = page.text();
    assert_eq!(html.matches("<h1").count(), 1, "one h1");
    // The JSON-LD can't close its <script> early.
    let ld = html.split("application/ld+json\">").nth(1).unwrap();
    assert!(!ld.split("</script>").next().unwrap().contains("</"));
}

#[renox::test]
async fn the_stages_are_numbered_in_order_with_unique_slugs() {
    let stages = renox_learn::course::STAGES;
    for (i, stage) in stages.iter().enumerate() {
        assert_eq!(
            usize::from(stage.number),
            i,
            "{} is out of order",
            stage.slug
        );
        assert!(!stage.topics.is_empty(), "{} has no topics", stage.slug);
    }
    let mut slugs: Vec<_> = stages.iter().map(|s| s.slug).collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), stages.len(), "a slug is used twice");
}

#[renox::test]
async fn every_docs_link_names_a_page_of_the_docs_site() {
    let app = site().await;
    let slugs: Vec<&str> = renox_site::content::PAGES.iter().map(|p| p.slug).collect();
    let mut checked = 0;
    let mut pages = vec!["/".to_owned(), "/llms.txt".into(), "/no-such-page".into()];
    pages.extend(
        renox_learn::content::chapters()
            .iter()
            .map(|c| format!("/stages/{}", c.slug)),
    );
    for path in &pages {
        let html = app.get(path).await.text();
        for link in doc_links(&html) {
            assert!(
                slugs.contains(&link.as_str()),
                "{path} links to docs.renox.rs/docs/{link}, which doesn't exist"
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "checked {checked} links");
}

#[renox::test]
async fn sitemap_robots_and_llms_txt_are_served() {
    let app = site().await;
    app.get("/sitemap.xml")
        .await
        .assert_ok()
        .assert_see("<loc>https://learn.renox.rs/</loc>");
    app.get("/robots.txt")
        .await
        .assert_ok()
        .assert_see("Sitemap: https://learn.renox.rs/sitemap.xml");
    let llms = app.get("/llms.txt").await;
    llms.assert_ok()
        .assert_header("content-type", "text/plain; charset=utf-8")
        .assert_see("# Learn Renox")
        .assert_see("- Stage 7: A live component on a model:");
}

#[renox::test]
async fn an_unknown_page_is_a_404_in_the_layout_and_not_indexed() {
    let app = site().await;
    app.get("/no-such-page")
        .await
        .assert_status(404)
        .assert_see("This page doesn&#x27;t exist.")
        .assert_see("<meta name=\"robots\" content=\"noindex\">")
        .assert_see("learn renox");
}

#[renox::test]
async fn the_home_page_has_an_etag_and_answers_304_to_it() {
    let app = site().await;
    let first = app.get("/").await;
    let etag = first.header("etag").expect("an ETag").to_owned();
    app.request()
        .header("if-none-match", &etag)
        .get("/")
        .await
        .assert_status(304);
}

#[renox::test]
async fn the_views_compile_and_are_written_with_tags_only() {
    let app = site().await;
    app.assert_views_compile();
    // The course teaches the 1.1 way, so its own pages are written that way:
    // tags and rx-if / rx-for, no {% %} statements.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/views");
    let mut stack = vec![dir];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !text.contains("{%"),
                    "{} has a {{% %}} statement",
                    path.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 3, "checked {checked} views");
}

#[renox::test]
async fn the_design_page_shows_every_component() {
    let app = site().await;
    let page = app.get("/design").await;
    page.assert_ok()
        .assert_see("<meta name=\"robots\" content=\"noindex\">");
    let html = page.text();
    for name in renox_learn::design::COMPONENTS {
        assert_eq!(
            html.matches(&format!("data-component=\"{name}\"")).count(),
            1,
            "{name} has one section"
        );
        assert_eq!(
            html.matches(&format!("href=\"#c-{name}\"")).count(),
            1,
            "{name} has one link in the index"
        );
    }
    assert_eq!(
        html.matches("data-component=").count(),
        renox_learn::design::COMPONENTS.len(),
        "a section is not listed in COMPONENTS"
    );
    for path in ["/sitemap.xml", "/llms.txt"] {
        let text = app.get(path).await.text();
        assert!(!text.contains("/design"), "{path} mentions /design");
    }
}

/// `css` without its comments.
fn without_comments(css: &str) -> String {
    let mut plain = String::new();
    let mut rest = css;
    while let Some((before, after)) = rest.split_once("/*") {
        plain.push_str(before);
        rest = after.split_once("*/").map_or("", |(_, tail)| tail);
    }
    plain.push_str(rest);
    plain
}

#[renox::test]
async fn every_page_links_the_shared_tokens_before_learn_css() {
    let app = site().await;
    for path in ["/", "/stages/setup", "/design", "/editor", "/no-such-page"] {
        let html = app.get(path).await.text();
        let tokens = html
            .find("href=\"/tokens.css")
            .unwrap_or_else(|| panic!("{path} links /tokens.css"));
        let own = html
            .find("href=\"/learn.css")
            .unwrap_or_else(|| panic!("{path} links /learn.css"));
        assert!(tokens < own, "{path}: tokens.css comes first");
        assert!(
            html.contains("<body class=\"rx-page learn rs\">"),
            "{path}: the roles' class"
        );
    }
    // Served from renox-site's copy, with the fonts.
    let css = app.get("/tokens.css").await;
    css.assert_ok();
    assert_eq!(css.text(), include_str!("../../site/public/tokens.css"));
    app.get("/fonts/inter-latin-wght-4.1.woff2")
        .await
        .assert_ok();
    app.get("/fonts/jetbrains-mono-latin-wght.woff2")
        .await
        .assert_ok();
}

#[test]
fn the_stylesheet_keeps_colours_and_fonts_in_the_tokens() {
    let css = without_comments(include_str!("../public/learn.css"));
    // Roles it may point at other tokens (the dark frame) and the callouts' colour.
    let allowed = [
        "--callout",
        "--rs-focus",
        "--rs-link",
        "--rs-text-2",
        "--rs-number",
    ];
    for line in css.lines() {
        let hex = line.match_indices('#').any(|(i, _)| {
            let digits = line[i + 1..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .count();
            (3..=8).contains(&digits)
        });
        for bad in ["rgb(", "rgba(", "hsl(", "font-family", "@font-face"] {
            assert!(!line.contains(bad), "learn.css: {line}");
        }
        assert!(!hex, "a colour literal in learn.css: {line}");
        // A custom property defined here (`--name:` not inside var()).
        let mut rest = line;
        while let Some(i) = rest.find("--") {
            let tail = &rest[i..];
            let name: String = tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            let starts_name = !rest[..i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '-');
            let defines = starts_name
                && tail[name.len()..].trim_start().starts_with(':')
                && !rest[..i].ends_with("var(");
            assert!(
                !defines || allowed.contains(&name.as_str()),
                "{name} is defined in learn.css (tokens go in site/public/tokens.css): {line}"
            );
            rest = &tail[2..];
        }
    }
}

#[renox::test]
async fn the_sitemap_lists_every_written_stage() {
    let app = site().await;
    let xml = app.get("/sitemap.xml").await.text();
    for c in renox_learn::content::chapters() {
        assert!(
            xml.contains(&format!(
                "<loc>https://learn.renox.rs/stages/{}</loc>",
                c.slug
            )),
            "{} is not in the sitemap",
            c.slug
        );
    }
    assert!(!xml.contains("/design"));
}

#[renox::test]
async fn the_course_json_ld_is_free_online_and_timed() {
    let app = site().await;
    let html = app.get("/").await.text();
    let sum: u32 = renox_learn::content::chapters()
        .iter()
        .map(|c| c.minutes)
        .sum();
    for needle in [
        "\"@type\":\"CourseInstance\"".to_owned(),
        format!("\"courseWorkload\":\"PT{sum}M\""),
        "\"category\":\"Free\"".into(),
        "\"url\":\"https://learn.renox.rs/stages/setup\"".into(),
    ] {
        assert!(html.contains(&needle), "the home page lacks {needle}");
    }
}

#[renox::test]
async fn each_stage_page_is_a_learning_resource_with_a_breadcrumb() {
    let app = site().await;
    for c in renox_learn::content::chapters() {
        let html = app.get(&format!("/stages/{}", c.slug)).await.text();
        for needle in [
            "\"@type\":\"LearningResource\"".to_owned(),
            format!("\"position\":{}", c.number),
            "\"@type\":\"BreadcrumbList\"".into(),
            format!("\"item\":\"https://learn.renox.rs/stages/{}\"", c.slug),
        ] {
            assert!(html.contains(&needle), "{} lacks {needle}", c.slug);
        }
        let ld = html.split("application/ld+json\">").nth(1).unwrap();
        assert!(!ld.split("</script>").next().unwrap().contains("</"));
    }
}

#[renox::test]
async fn a_stage_is_also_markdown() {
    let app = site().await;
    app.get("/stages/setup.md")
        .await
        .assert_ok()
        .assert_header("content-type", "text/markdown; charset=utf-8");
    let body = app.get("/stages/setup.md").await.text();
    assert!(
        body.starts_with("# Stage 0: Set up and run an app"),
        "{body}"
    );
    app.get("/stages/nope.md").await.assert_not_found();
}

#[renox::test]
async fn llms_txt_links_each_written_stage() {
    let app = site().await;
    let llms = app.get("/llms.txt").await.text();
    for c in renox_learn::content::chapters() {
        assert!(llms.contains(&format!(
            "Chapter: https://learn.renox.rs/stages/{}.md",
            c.slug
        )));
    }
}

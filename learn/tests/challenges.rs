//! Challenges: every folder loads, a bad one is a problem, and a chapter shows
//! the challenge it names. (That each start fails and each solution passes is
//! `challenges/check.sh`, run by CI's `learn-challenges` job.)

use renox::testing::TestApp;
use renox_learn::challenge::{self, Raw};
use renox_learn::content;
use renox_learn::directive::Directive;

const IDS: &[(u8, &str)] = &[
    (1, "add-allergens"),
    (2, "cheap-items"),
    (3, "category-page"),
    (4, "category-chip"),
    (5, "name-min-length"),
    (7, "sell-many"),
    (8, "test-the-menu"),
];

async fn site() -> TestApp {
    TestApp::with_config(renox_learn::app(), |c| {
        c.url = "https://learn.renox.rs".into()
    })
    .await
}

/// A challenge folder that is fine, which a test then breaks in one place.
fn fixture() -> Raw {
    Raw {
        dir: "s02/demo",
        markdown: Some(
            "---\ntitle: Demo\nstage: 2\nfiles: src/a.rs\ntest: challenge_demo\nminutes: 5\nsummary: A demo.\n---\nDo it.\n",
        ),
        start: &[("src/a.rs", "start")],
        solution: &[("src/a.rs", "solution")],
        tests: &[("challenge_demo.rs", "test")],
        failing: None,
        passing: None,
    }
}

fn messages(raw: &Raw) -> Vec<String> {
    let (challenge, problems) = challenge::parse(raw);
    assert!(challenge.is_none() || problems.is_empty());
    problems.into_iter().map(|p| p.message).collect()
}

#[test]
fn every_challenge_loads_without_problems() {
    let problems = challenge::problems();
    assert!(
        problems.is_empty(),
        "{}",
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    for (stage, id) in IDS {
        let c = challenge::find(*stage, id).unwrap_or_else(|| panic!("no challenge {stage}/{id}"));
        assert!(!c.files.is_empty() && c.files.len() <= 4, "{id}");
        assert!(c.test_path.starts_with("tests/challenge_"), "{id}");
        assert!(c.test.contains("#["), "{id}: the test is there");
        assert!(!c.instructions_html.is_empty(), "{id}");
        assert!(c.minutes > 0, "{id}");
        assert!(
            c.failing.is_some_and(|f| !f.trim().is_empty()),
            "{id}: record it with RECORD=1 challenges/check.sh"
        );
        assert!(
            c.passing.is_some_and(|f| f.contains("test result: ok")),
            "{id}"
        );
        for f in &c.files {
            assert_ne!(f.start, f.solution, "{id}: {} is the same solved", f.path);
        }
    }
    assert_eq!(
        challenge::all().len(),
        IDS.len(),
        "a challenge folder is not in IDS"
    );
}

#[test]
fn every_challenge_is_shown_by_the_chapter_of_its_stage() {
    for (stage, id) in IDS {
        let chapter = content::chapters()
            .iter()
            .find(|c| c.number == *stage)
            .expect("the stage has a chapter");
        let shown = chapter
            .directives
            .iter()
            .filter(|d| matches!(d, Directive::Challenge(c) if c.id == *id))
            .count();
        assert_eq!(
            shown, 1,
            "{id} should be in the chapter of stage {stage} once"
        );
    }
}

#[test]
fn a_good_fixture_loads() {
    let (challenge, problems) = challenge::parse(&fixture());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(challenge.unwrap().test_path, "tests/challenge_demo.rs");
}

#[test]
fn a_bad_front_matter_is_a_problem() {
    // `test` missing.
    let mut raw = fixture();
    raw.markdown = Some(
        "---\ntitle: Demo\nstage: 2\nfiles: src/a.rs\nminutes: 5\nsummary: A demo.\n---\nDo it.\n",
    );
    assert_eq!(messages(&raw), ["`test` is missing"]);

    // A file of `files` that `solution/` lacks.
    let mut raw = fixture();
    raw.solution = &[];
    assert_eq!(messages(&raw), ["`src/a.rs` is missing from solution/"]);

    // A file of `files` that `start/` lacks, and a start file `files` omits.
    let mut raw = fixture();
    raw.start = &[("src/b.rs", "x")];
    let found = messages(&raw);
    assert!(
        found.contains(&"`src/a.rs` is missing from start/".to_owned()),
        "{found:?}"
    );
    assert!(
        found.contains(&"start/src/b.rs is not listed in `files`".to_owned()),
        "{found:?}"
    );

    // Files the student never changes.
    for path in [
        "Cargo.toml",
        "migrations/20261012_x.up.sql",
        "tests/challenge_demo.rs",
    ] {
        let mut raw = fixture();
        let md: &'static str = Box::leak(
            format!("---\ntitle: Demo\nstage: 2\nfiles: {path}\ntest: challenge_demo\nminutes: 5\nsummary: A demo.\n---\nDo it.\n").into_boxed_str(),
        );
        raw.markdown = Some(md);
        assert_eq!(
            messages(&raw)[0],
            format!("`{path}` can't be a file the student changes")
        );
    }

    // A stage that does not exist, and a folder that does not match the stage.
    let mut raw = fixture();
    raw.dir = "s99/demo";
    raw.markdown = Some(
        "---\ntitle: Demo\nstage: 99\nfiles: src/a.rs\ntest: challenge_demo\nminutes: 5\nsummary: A demo.\n---\nDo it.\n",
    );
    assert_eq!(messages(&raw), ["stage 99 has no app in course/"]);
    let mut raw = fixture();
    raw.dir = "s03/demo";
    assert_eq!(messages(&raw), ["stage 2 does not match the folder s03"]);

    // An id the folder name can't be, a test file that isn't there.
    let mut raw = fixture();
    raw.dir = "s02/Cheap_Items";
    assert!(messages(&raw)[0].starts_with("id `Cheap_Items` must match"));
    let mut raw = fixture();
    raw.tests = &[];
    assert_eq!(
        messages(&raw),
        ["tests/challenge_demo.rs is missing from the challenge folder"]
    );

    // Five files.
    let mut raw = fixture();
    raw.markdown = Some(
        "---\ntitle: Demo\nstage: 2\nfiles: a, b, c, d, e\ntest: challenge_demo\nminutes: 5\nsummary: A demo.\n---\nDo it.\n",
    );
    assert!(messages(&raw)[0].starts_with("`files` lists 1 to 4 paths"));
}

#[test]
fn an_unknown_challenge_id_is_a_problem() {
    const TEXT: &str = "---\ntitle: Demo\nstage: 2\nsummary: S\nminutes: 5\n---\nIntro.\n\n```learn:challenge\nid: no-such-thing\n```\n";
    let (chapter, problems) = content::parse("02-models.md", TEXT);
    assert!(chapter.is_some());
    let found = problems
        .iter()
        .find(|p| p.line == 9)
        .unwrap_or_else(|| panic!("no problem on line 9: {problems:?}"));
    assert_eq!(
        found.message,
        "learn:challenge: stage 2 has no challenge `no-such-thing`"
    );
    // The same id in another stage's chapter is unknown too.
    let (_, problems) =
        content::parse("03-pages.md", &TEXT.replace("no-such-thing", "cheap-items"));
    assert!(
        problems
            .iter()
            .any(|p| p.message == "learn:challenge: stage 3 has no challenge `cheap-items`")
    );
}

/// The text of some HTML: tags removed.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[renox::test]
async fn the_stage_page_shows_the_challenge() {
    let page = site().await.get("/stages/models").await;
    page.assert_ok();
    let html = page.text();
    let c = challenge::find(2, "cheap-items").unwrap();
    for needle in [
        "<section class=\"learn-challenge\" id=\"challenge-cheap-items\" data-challenge=\"s02/cheap-items\">",
        "<p class=\"eyebrow\"><b>02</b> Challenge</p>",
        "<h2 id=\"challenge-cheap-items-title\">Only the cheap items</h2>",
        "About 15 minutes",
        "Start from your stage 2 app",
        "tests/challenge_cheap_items.rs",
        "What failing looks like",
        "What passing looks like",
        "<details class=\"learn-challenge__solution\"><summary>Show the solution</summary>",
        "data-challenge-done aria-pressed=\"false\" hidden>I passed it</button>",
    ] {
        assert!(html.contains(needle), "the page lacks {needle}");
    }
    // Code is highlighted: its words sit in spans.
    let text = plain(&html);
    for needle in [
        "git clone --depth 1 https://github.com/arif-rachim/renox",
        "cd renox/learn/course/stage-02",
        "cargo test --test challenge_cheap_items",
    ] {
        assert!(text.contains(needle), "the page's text lacks {needle}");
    }
    // The start of each file is shown before the solution's.
    let solution_at = text.find("Show the solution").unwrap();
    let start_at = text
        .find("Write the query here")
        .expect("the start is shown");
    assert!(start_at < solution_at, "the start comes first");
    assert!(
        text[solution_at..].contains(".where_(MenuItem::PRICE.lt(300))"),
        "the solution is inside the details"
    );
    // The test is shown (escaped), with a copy button on each panel.
    assert!(html.contains("cheap_items_are_under_three_dollars_cheapest_first"));
    assert!(html.matches("data-copy").count() >= 6);
    assert_eq!(c.files.len(), 1);
    // The script is loaded from a file, not inline.
    assert!(html.contains("challenge.js"));
}

#[renox::test]
async fn every_stage_page_with_a_challenge_renders_it() {
    let app = site().await;
    for (stage, id) in IDS {
        let slug = renox_learn::course::STAGES
            .iter()
            .find(|s| s.number == *stage)
            .unwrap()
            .slug;
        let html = app.get(&format!("/stages/{slug}")).await.text();
        let marker = format!("data-challenge=\"s{stage:02}/{id}\"");
        assert_eq!(html.matches(&marker).count(), 1, "{id} on /stages/{slug}");
        assert!(
            !html.contains("learn-problem"),
            "/stages/{slug} shows a problem"
        );
    }
}

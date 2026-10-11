//! The course's chapters: Markdown files in `content/stages/`, compiled in
//! by build.rs.
//!
//! A chapter is named `NN-slug.md` (`NN` is its stage number, `slug` the
//! stage's slug in [`course::STAGES`](crate::course::STAGES)) and starts with
//! a front matter block between `---` lines, one `key: value` per line, all
//! four required:
//!
//! ```text
//! ---
//! title: Set up and run an app
//! stage: 0
//! summary: Install rnx, create the coffee app and see it answer in the browser.
//! minutes: 10
//! ```
//!
//! `title` and `summary` must equal the outline's, so the Markdown reads well
//! on GitHub and can't drift from the home page. The title is the page's
//! `<h1>`: a chapter's own headings start at `##`. The directives a chapter
//! can use are in [`directive`](crate::directive).
//!
//! Mistakes are collected as [`Problem`]s (a test fails on any), and a bad
//! directive is shown where it is.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use renox_site::render::Heading;
use serde::Serialize;

use crate::course::STAGES;
use crate::directive::Directive;
use crate::render::render;

include!(concat!(env!("OUT_DIR"), "/stages.rs"));

/// A mistake in a chapter, with where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// Relative to `learn/`, like `content/stages/01-migrations.md`.
    pub file: String,
    /// From 1, counted in the whole file, front matter included.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

/// One chapter, parsed and rendered.
#[derive(Debug, Clone, Serialize)]
pub struct Chapter {
    /// Its file name, like `01-migrations.md`.
    pub file: &'static str,
    /// The stage's number.
    pub number: u8,
    /// The stage's slug: `/stages/{slug}`.
    pub slug: &'static str,
    pub title: String,
    pub summary: String,
    /// About how long the stage takes.
    pub minutes: u32,
    /// The body as Markdown (without the front matter).
    pub markdown: String,
    /// The body as HTML.
    pub html: String,
    /// The `##` and `###` headings, for the table of contents.
    pub toc: Vec<Heading>,
    /// The directives of the body, in order.
    pub directives: Vec<Directive>,
}

/// Every chapter and every problem found in them.
#[derive(Debug)]
pub struct Content {
    /// By stage number.
    pub chapters: Vec<Chapter>,
    pub problems: Vec<Problem>,
}

/// The whole course's content, parsed at first use.
pub static CONTENT: LazyLock<Content> = LazyLock::new(load);

/// Every chapter, by stage number.
pub fn chapters() -> &'static [Chapter] {
    &CONTENT.chapters
}

/// The chapter of the stage whose slug is `slug`.
pub fn find(slug: &str) -> Option<&'static Chapter> {
    chapters().iter().find(|c| c.slug == slug)
}

/// Every problem in every chapter.
pub fn problems() -> &'static [Problem] {
    &CONTENT.problems
}

fn load() -> Content {
    let mut chapters = Vec::new();
    let mut problems = Vec::new();
    // Each quiz id and the file that has it.
    let mut quizzes: HashMap<String, String> = HashMap::new();
    for (file, text) in STAGE_FILES {
        let (chapter, found) = parse(file, text);
        problems.extend(found);
        let Some(chapter) = chapter else { continue };
        let path = format!("content/stages/{file}");
        for (id, line) in quiz_ids(text) {
            match quizzes.get(&id) {
                Some(other) => problems.push(Problem {
                    file: path.clone(),
                    line,
                    message: format!("quiz id `{id}` is also used in {other}"),
                }),
                None => {
                    quizzes.insert(id, path.clone());
                }
            }
        }
        chapters.push(chapter);
    }
    chapters.sort_by_key(|c: &Chapter| c.number);
    Content { chapters, problems }
}

/// The ids of the quizzes in `text`, with their lines.
fn quiz_ids(text: &str) -> Vec<(String, usize)> {
    let mut ids = Vec::new();
    let mut in_quiz = false;
    for (i, line) in text.replace("\r\n", "\n").lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_quiz = !in_quiz && trimmed.starts_with("```learn:quiz");
        } else if in_quiz && let Some(id) = trimmed.strip_prefix("id:") {
            ids.push((id.trim().trim_matches('"').to_owned(), i + 1));
        }
    }
    ids
}

/// A chapter from its file: the front matter, then the Markdown. A file
/// whose front matter fails gives no chapter and one problem; the other
/// mistakes are collected and the chapter is still built.
pub fn parse(file: &'static str, text: &str) -> (Option<Chapter>, Vec<Problem>) {
    let path = format!("content/stages/{file}");
    let problem = |line: usize, message: String| Problem {
        file: path.clone(),
        line,
        message,
    };
    let text = text.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return (
            None,
            vec![problem(1, "no front matter (start with ---)".into())],
        );
    };
    let Some((head, body)) = rest.split_once("\n---\n") else {
        return (
            None,
            vec![problem(1, "the front matter has no closing ---".into())],
        );
    };
    // The lines the front matter takes: both `---` lines and its own.
    let front_lines = head.lines().count() + 2;
    let mut get: HashMap<String, (String, usize)> = HashMap::new();
    for (i, line) in head.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = i + 2;
        let Some((k, v)) = line.split_once(':') else {
            return (
                None,
                vec![problem(at, format!("`{line}` is not `key: value`"))],
            );
        };
        // A value may be quoted (`title: "A: b"`), as YAML allows.
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(v);
        get.insert(k.trim().to_owned(), (v.to_owned(), at));
    }
    let mut found = Vec::new();
    for key in ["title", "stage", "summary", "minutes"] {
        if get.get(key).is_none_or(|(v, _)| v.is_empty()) {
            return (None, vec![problem(1, format!("`{key}` is missing"))]);
        }
    }
    let value = |key: &str| get[key].0.clone();
    let at = |key: &str| get[key].1;
    let number = match value("stage").parse::<u8>() {
        Ok(n) if n <= 99 => n,
        _ => {
            return (
                None,
                vec![problem(
                    at("stage"),
                    format!(
                        "stage `{}` is not a whole number from 0 to 99",
                        value("stage")
                    ),
                )],
            );
        }
    };
    let minutes = match value("minutes").parse::<u32>() {
        Ok(m) if (1..=120).contains(&m) => m,
        _ => {
            return (
                None,
                vec![problem(
                    at("minutes"),
                    format!(
                        "minutes `{}` is not a whole number from 1 to 120",
                        value("minutes")
                    ),
                )],
            );
        }
    };
    let (title, summary) = (value("title"), value("summary"));

    // The file name and the front matter agree with the outline.
    let stem = file.trim_end_matches(".md");
    let named = stem
        .split_once('-')
        .filter(|(n, _)| n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()));
    let outline = STAGES.get(usize::from(number));
    let slug = outline.map_or("", |s| s.slug);
    let name_ok = named.is_some_and(|(n, s)| {
        n.parse::<u8>().ok() == Some(number) && outline.is_some_and(|o| o.slug == s)
    });
    if !name_ok {
        found.push(problem(
            1,
            "the file name must be NN-slug.md with NN = stage and slug = course::STAGES[NN].slug"
                .into(),
        ));
    }
    if let Some(o) = outline {
        for (key, shown, expected) in [("title", &title, o.title), ("summary", &summary, o.summary)]
        {
            if shown != expected {
                found.push(problem(
                    at(key),
                    format!("`{key}` is `{shown}`, the outline (course.rs) says `{expected}`"),
                ));
            }
        }
    }

    let markdown = body.trim().to_owned();
    // `body` starts right after the closing `---` line; `trim` may drop
    // leading blank lines, so render the untrimmed body for true lines.
    let (html, toc, directives, problems) = render(body, &path, front_lines);
    found.extend(problems);
    (
        Some(Chapter {
            file,
            number,
            slug,
            title,
            summary,
            minutes,
            markdown,
            html,
            toc,
            directives,
        }),
        found,
    )
}

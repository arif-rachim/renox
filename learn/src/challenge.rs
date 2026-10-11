//! Challenges: tasks a student solves in their own copy of the course app and
//! checks with `cargo test`. The site never runs the student's code.
//!
//! A challenge is a folder, `challenges/sNN/<id>/` (`NN` is the stage whose
//! end state it starts from, `<id>` matches `[a-z0-9][a-z0-9-]{0,39}`):
//!
//! ```text
//! challenge.md                    front matter + the instructions
//! start/<path>                    the files the student changes, as they start
//! solution/<path>                 the same paths, solved (it may add new files)
//! tests/challenge_<name>.rs       the test the student adds to the app's tests/
//! output-failing.txt              recorded by `challenges/check.sh` with RECORD=1
//! output-passing.txt
//! ```
//!
//! The front matter is `key: value` lines between `---` lines:
//!
//! ```text
//! ---
//! title: Only the cheap items
//! stage: 2
//! files: src/app/menu/model.rs
//! test: challenge_cheap_items
//! minutes: 15
//! summary: Write MenuItem::cheap, the items under $3.00, cheapest first.
//! ---
//! Add `MenuItem::cheap(db)` ... (the instructions, Markdown)
//! ```
//!
//! `files` lists 1 to 4 paths (relative to the stage app) that exist in
//! `start/` and `solution/`, and nothing else is in `start/`. They are never
//! `Cargo.toml`, `Cargo.lock`, `build.rs`, `migrations/...` or the challenge's
//! own test. Anything wrong is a [`Problem`]; `tests/challenges.rs` asserts
//! there are none. A chapter shows a challenge with `learn:challenge`
//! ([`directive`](crate::directive)).

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Serialize;

use crate::code;
use crate::content::Problem;
use crate::render::render;

include!(concat!(env!("OUT_DIR"), "/challenges.rs"));

/// A file the student changes: how it starts and how it ends.
#[derive(Debug, Clone, Serialize)]
pub struct ChallengeFile {
    /// Relative to the stage app.
    pub path: String,
    /// The file as the student has it.
    pub start: &'static str,
    /// The file solved.
    pub solution: &'static str,
}

/// A file the solution adds.
#[derive(Debug, Clone, Serialize)]
pub struct NewFile {
    /// Relative to the stage app.
    pub path: String,
    /// Its text.
    pub solution: &'static str,
}

/// One challenge, parsed.
#[derive(Debug, Clone, Serialize)]
pub struct Challenge {
    /// The stage whose end state it starts from.
    pub stage: u8,
    /// Its folder name.
    pub id: &'static str,
    /// Heading of its section.
    pub title: String,
    /// One sentence: what the student writes.
    pub summary: String,
    /// About how long it takes.
    pub minutes: u16,
    /// The files the student changes.
    pub files: Vec<ChallengeFile>,
    /// Files only the solution has (the student creates them).
    pub new_files: Vec<NewFile>,
    /// Where the test goes: `tests/challenge_cheap_items.rs`.
    pub test_path: String,
    /// The test's text.
    pub test: &'static str,
    /// The instructions as HTML.
    pub instructions_html: String,
    /// `cargo test` failing, as recorded.
    pub failing: Option<&'static str>,
    /// `cargo test` passing, as recorded.
    pub passing: Option<&'static str>,
}

struct Loaded {
    challenges: Vec<Challenge>,
    problems: Vec<Problem>,
}

static LOADED: LazyLock<Loaded> = LazyLock::new(|| {
    let mut challenges = Vec::new();
    let mut problems = Vec::new();
    for raw in RAW {
        let (challenge, found) = parse(raw);
        challenges.extend(challenge);
        problems.extend(found);
    }
    Loaded {
        challenges,
        problems,
    }
});

/// Every challenge that loaded, by stage and folder name.
pub fn all() -> &'static [Challenge] {
    &LOADED.challenges
}

/// The challenge `id` of `stage`.
pub fn find(stage: u8, id: &str) -> Option<&'static Challenge> {
    all().iter().find(|c| c.stage == stage && c.id == id)
}

/// Everything wrong in the challenge folders.
pub fn problems() -> &'static [Problem] {
    &LOADED.problems
}

/// Whether `id` is a valid challenge id: `[a-z0-9][a-z0-9-]{0,39}`.
pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    id.len() <= 40
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Files a challenge may not ask the student to change.
fn forbidden(path: &str) -> bool {
    matches!(path, "Cargo.toml" | "Cargo.lock" | "build.rs") || path.starts_with("migrations/")
}

fn bad(problems: &mut Vec<Problem>, file: &str, line: usize, message: String) {
    problems.push(Problem {
        file: file.to_owned(),
        line,
        message,
    });
}

/// Parses one folder. The challenge is `None` when anything is wrong.
pub fn parse(raw: &Raw) -> (Option<Challenge>, Vec<Problem>) {
    let file = format!("challenges/{}/challenge.md", raw.dir);
    let mut problems = Vec::new();
    let (stage_dir, id) = raw.dir.split_once('/').unwrap_or((raw.dir, ""));
    let dir_stage = stage_dir
        .strip_prefix('s')
        .filter(|n| n.len() == 2)
        .and_then(|n| n.parse::<u8>().ok());
    if dir_stage.is_none() {
        bad(
            &mut problems,
            &file,
            1,
            format!("`{stage_dir}` is not a stage folder (sNN)"),
        );
    }
    if !valid_id(id) {
        bad(
            &mut problems,
            &file,
            1,
            format!("id `{id}` must match [a-z0-9][a-z0-9-]{{0,39}}"),
        );
    }
    let Some(markdown) = raw.markdown else {
        bad(&mut problems, &file, 1, "challenge.md is missing".into());
        return (None, problems);
    };
    let text = markdown.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        bad(
            &mut problems,
            &file,
            1,
            "no front matter (start with ---)".into(),
        );
        return (None, problems);
    };
    let Some((head, body)) = rest.split_once("\n---\n") else {
        bad(
            &mut problems,
            &file,
            1,
            "the front matter has no closing ---".into(),
        );
        return (None, problems);
    };
    let front_lines = head.matches('\n').count() + 3;
    let mut get: HashMap<&str, (usize, String)> = HashMap::new();
    for (n, line) in head.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            bad(
                &mut problems,
                &file,
                n + 2,
                format!("`{line}` is not `key: value`"),
            );
            continue;
        };
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(v);
        get.insert(k.trim(), (n + 2, v.to_owned()));
    }
    let mut need = |key: &str| -> Option<(usize, String)> {
        match get.get(key).filter(|(_, v)| !v.is_empty()) {
            Some(found) => Some(found.clone()),
            None => {
                bad(&mut problems, &file, 1, format!("`{key}` is missing"));
                None
            }
        }
    };
    let title = need("title");
    let stage = need("stage");
    let files_key = need("files");
    let test_key = need("test");
    let minutes = need("minutes");
    let summary = need("summary");

    if let Some((line, value)) = &stage {
        match value.parse::<u8>() {
            Ok(n) if Some(n) == dir_stage => {
                if code::file(n, "Cargo.toml").is_none() {
                    bad(
                        &mut problems,
                        &file,
                        *line,
                        format!("stage {n} has no app in course/"),
                    );
                }
            }
            Ok(n) => bad(
                &mut problems,
                &file,
                *line,
                format!("stage {n} does not match the folder {stage_dir}"),
            ),
            Err(_) => bad(
                &mut problems,
                &file,
                *line,
                format!("stage `{value}` is not a number"),
            ),
        }
    }
    let minutes_n = minutes
        .as_ref()
        .and_then(|(line, v)| match v.parse::<u16>() {
            Ok(n) if n > 0 => Some(n),
            _ => {
                bad(
                    &mut problems,
                    &file,
                    *line,
                    format!("minutes `{v}` is not a positive number"),
                );
                None
            }
        });

    let test_path = test_key.as_ref().and_then(|(line, stem)| {
        let ok = !stem.is_empty()
            && stem
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !ok {
            bad(
                &mut problems,
                &file,
                *line,
                format!("test `{stem}` must be a file stem like challenge_cheap_items"),
            );
            return None;
        }
        let name = format!("{stem}.rs");
        if raw.tests.iter().any(|(n, _)| *n == name) {
            Some(format!("tests/{name}"))
        } else {
            bad(
                &mut problems,
                &file,
                *line,
                format!("tests/{name} is missing from the challenge folder"),
            );
            None
        }
    });
    let test_text = test_path.as_ref().and_then(|p| {
        let name = p.trim_start_matches("tests/");
        raw.tests.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
    });

    let mut files = Vec::new();
    if let Some((line, value)) = &files_key {
        let paths: Vec<&str> = value.split(',').map(str::trim).collect();
        if paths.is_empty() || paths.len() > 4 || paths.iter().any(|p| p.is_empty()) {
            bad(
                &mut problems,
                &file,
                *line,
                "`files` lists 1 to 4 paths, separated by commas".into(),
            );
        }
        for path in paths.iter().filter(|p| !p.is_empty()) {
            let own_test = test_path.as_deref() == Some(*path);
            if forbidden(path) || own_test || path.starts_with('/') || path.contains("..") {
                bad(
                    &mut problems,
                    &file,
                    *line,
                    format!("`{path}` can't be a file the student changes"),
                );
                continue;
            }
            let start = raw.start.iter().find(|(p, _)| p == path).map(|(_, t)| *t);
            let solution = raw
                .solution
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, t)| *t);
            match (start, solution) {
                (Some(start), Some(solution)) => files.push(ChallengeFile {
                    path: (*path).to_owned(),
                    start,
                    solution,
                }),
                (None, _) => bad(
                    &mut problems,
                    &file,
                    *line,
                    format!("`{path}` is missing from start/"),
                ),
                (_, None) => bad(
                    &mut problems,
                    &file,
                    *line,
                    format!("`{path}` is missing from solution/"),
                ),
            }
        }
        for (path, _) in raw.start {
            if !paths.contains(path) {
                bad(
                    &mut problems,
                    &file,
                    *line,
                    format!("start/{path} is not listed in `files`"),
                );
            }
        }
    }
    let new_files: Vec<NewFile> = raw
        .solution
        .iter()
        .filter(|(p, _)| !raw.start.iter().any(|(s, _)| s == p))
        .map(|(p, t)| NewFile {
            path: (*p).to_owned(),
            solution: t,
        })
        .collect();

    if body.contains("```learn:") {
        bad(
            &mut problems,
            &file,
            front_lines + 1,
            "the instructions can't contain directives".into(),
        );
    }
    let instructions_html = if body.contains("```learn:") {
        String::new()
    } else {
        let (html, _, _, found) = render(body, &file, front_lines);
        for p in found {
            problems.push(p);
        }
        html
    };

    let (Some((_, title)), Some((_, summary)), Some(minutes), Some(test_path), Some(test)) =
        (title, summary, minutes_n, test_path, test_text)
    else {
        return (None, problems);
    };
    if !problems.is_empty() || files.is_empty() {
        return (None, problems);
    }
    let challenge = Challenge {
        stage: dir_stage.unwrap_or(0),
        id: raw.dir.split_once('/').map_or(raw.dir, |(_, id)| id),
        title,
        summary,
        minutes,
        files,
        new_files,
        test_path,
        test,
        instructions_html,
        failing: raw.failing,
        passing: raw.passing,
    };
    (Some(challenge), problems)
}

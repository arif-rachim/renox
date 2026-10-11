//! Reads the course app's stages (`course/stage-NN/`) and writes
//! `$OUT_DIR/course_code.rs`, which `src/code.rs` includes: the named regions
//! chapters include, every file's text, and what changed in each stage.
//!
//! A region is marked in a stage's file by two comment lines, `[learn:<name>]`
//! and `[/learn:<name>]`, in the comment syntax of the file:
//!
//! ```text
//! // [learn:menu.item-model]            Rust, JavaScript
//! {# [learn:menu.index] #}              templates (`{#- … #}` works too)
//! -- [learn:menu.unique-name]           SQL
//! /* [learn:menu.css] */                CSS
//! # [learn:menu.deps]                   TOML, .env.example
//! ```
//!
//! The lines between become the region's text, without the marker lines and
//! without their common indentation. A name opened again later in the same
//! file continues the region (the parts are joined by a `…` line). The key is
//! (stage, name): every stage may have its own `menu.item-model`. Mistakes
//! (never closed, closed without an open, one name in two files of a stage,
//! an empty region) never fail the build: they go into `PROBLEMS`, which
//! `tests/content.rs` reports. The marker lines are removed from the text of
//! every file, so `file` includes and the diffs never show them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The files and directories of a stage that are scanned.
const SCANNED: &[&str] = &[
    "src",
    "resources",
    "migrations",
    "tests",
    "public/app.css",
    "Cargo.toml",
    "build.rs",
    ".env.example",
];

/// A file longer than this is not diffed.
const MAX_LINES: usize = 2000;

/// Lines of context around a change.
const CONTEXT: usize = 3;

pub fn generate() {
    let course = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("course");
    let mut stages: Vec<(u8, PathBuf)> = std::fs::read_dir(&course)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let number = name.strip_prefix("stage-")?.parse::<u8>().ok()?;
                    (name.len() == 8 && e.path().is_dir()).then(|| (number, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    stages.sort();

    let mut regions: BTreeMap<(u8, String), Region> = BTreeMap::new();
    let mut problems = Vec::new();
    // Per stage: path → text without marker lines.
    let mut texts: BTreeMap<u8, BTreeMap<String, String>> = BTreeMap::new();

    for (number, dir) in &stages {
        // Never `course` itself: it holds the shared `target/`.
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files = Vec::new();
        for item in SCANNED {
            let path = dir.join(item);
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, &mut files);
        }
        files.sort();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let path = file
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let shown = format!("learn/course/stage-{number:02}/{path}");
            let clean = scan(*number, &path, &shown, &text, &mut regions, &mut problems);
            texts.entry(*number).or_default().insert(path, clean);
        }
    }

    let mut out = String::new();
    out.push_str("pub static REGIONS: &[Region] = &[\n");
    for ((stage, name), region) in &regions {
        let text = region.text();
        writeln!(
            out,
            "    Region {{ stage: {stage}, name: {name:?}, path: {:?}, language: {:?}, first_line: {}, last_line: {}, text: {text:?} }},",
            region.path, region.language, region.first_line, region.last_line,
        )
        .unwrap();
    }
    out.push_str("];\n\npub static FILES: &[File] = &[\n");
    for (stage, files) in &texts {
        for (path, text) in files {
            writeln!(
                out,
                "    File {{ stage: {stage}, path: {path:?}, text: {text:?} }},"
            )
            .unwrap();
        }
    }
    out.push_str("];\n\npub static CHANGES: &[Change] = &[\n");
    for (stage, files) in &texts {
        let Some(before) = stage.checked_sub(1).and_then(|n| texts.get(&n)) else {
            continue;
        };
        let mut paths: Vec<&String> = files.keys().chain(before.keys()).collect();
        paths.sort();
        paths.dedup();
        for path in paths {
            let (kind, old, new) = match (before.get(path), files.get(path)) {
                (None, Some(new)) => ("added", "", new.as_str()),
                (Some(old), None) => ("removed", old.as_str(), ""),
                (Some(old), Some(new)) if old != new => ("changed", old.as_str(), new.as_str()),
                _ => continue,
            };
            let diff = diff(old, new);
            writeln!(
                out,
                "    Change {{ stage: {stage}, path: {path:?}, kind: {kind:?}, diff: {diff:?} }},"
            )
            .unwrap();
        }
    }
    problems.sort();
    out.push_str("];\n\npub static PROBLEMS: &[&str] = &");
    writeln!(out, "{problems:?};").unwrap();

    let target = Path::new(&std::env::var("OUT_DIR").unwrap()).join("course_code.rs");
    std::fs::write(target, out).unwrap();
}

/// Every text file under `path` (or `path` itself) a stage is made of.
fn collect(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect(&path, files);
        }
    } else if path.is_file() && is_text(path) {
        files.push(path.to_path_buf());
    }
}

/// Whether a file is read as text (images, fonts and the like are skipped).
fn is_text(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == ".env.example"
        || matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("rs" | "html" | "sql" | "css" | "js" | "toml" | "json" | "md" | "txt")
        )
}

/// The language a file's code is highlighted as.
fn language(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "html" => "html",
        "sql" => "sql",
        "css" => "css",
        "toml" => "toml",
        "js" | "mjs" => "js",
        _ => "text",
    }
}

/// One region as it is being collected.
struct Region {
    path: String,
    /// The file's path shown in problems.
    shown: String,
    language: &'static str,
    /// Each part: the line number of its first line and its lines.
    parts: Vec<(usize, Vec<String>)>,
    first_line: usize,
    last_line: usize,
}

impl Region {
    /// The parts, each without its common indentation, joined by a `…`
    /// line in the language's comment syntax.
    fn text(&self) -> String {
        let mut out = Vec::new();
        for (n, (_, lines)) in self.parts.iter().enumerate() {
            if n > 0 {
                out.push(
                    match self.language {
                        "html" => "{# … #}",
                        "sql" => "-- …",
                        "css" => "/* … */",
                        "toml" | "text" => "# …",
                        _ => "// …",
                    }
                    .to_owned(),
                );
            }
            let indent = lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            for line in lines {
                let line = line.trim_end();
                out.push(line.get(indent..).unwrap_or(line.trim_start()).to_owned());
            }
        }
        while out.first().is_some_and(|l| l.is_empty()) {
            out.remove(0);
        }
        while out.last().is_some_and(|l| l.is_empty()) {
            out.pop();
        }
        out.join("\n")
    }
}

/// `[learn:name]` → (true, name), `[/learn:name]` → (false, name), when
/// the line holds nothing but the marker in a comment.
fn marker(line: &str) -> Option<(bool, &str)> {
    let mut rest = line.trim();
    for open in ["{#-", "{#", "//", "--", "/*", "#"] {
        if let Some(r) = rest.strip_prefix(open) {
            rest = r;
            break;
        }
    }
    for close in ["-#}", "#}", "*/"] {
        if let Some(r) = rest.strip_suffix(close) {
            rest = r;
            break;
        }
    }
    let rest = rest.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (open, name) = match rest.strip_prefix('/') {
        Some(name) => (false, name),
        None => (true, rest),
    };
    let name = name.strip_prefix("learn:")?;
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'));
    valid.then_some((open, name))
}

/// Collects the regions of one file; returns its text without marker lines.
fn scan(
    stage: u8,
    path: &str,
    shown: &str,
    text: &str,
    regions: &mut BTreeMap<(u8, String), Region>,
    problems: &mut Vec<String>,
) -> String {
    let language = language(path);
    let mut clean = String::new();
    // Open regions: name → (first line number, lines so far).
    let mut open: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let number = n + 1;
        match marker(line) {
            Some((true, name)) => {
                if open.contains_key(name) {
                    problems.push(format!(
                        "{shown}:{number}: `[learn:{name}]` is opened again before it was closed"
                    ));
                } else {
                    open.insert(name.to_owned(), (number + 1, Vec::new()));
                }
            }
            Some((false, name)) => {
                let Some((first, lines)) = open.remove(name) else {
                    problems.push(format!(
                        "{shown}:{number}: `[/learn:{name}]` closes a region that isn't open"
                    ));
                    continue;
                };
                if lines.iter().all(|l| l.trim().is_empty()) {
                    problems.push(format!("{shown}:{number}: `{name}` is empty"));
                    continue;
                }
                let last = number - 1;
                match regions.get_mut(&(stage, name.to_owned())) {
                    Some(region) if region.path == path => {
                        region.parts.push((first, lines));
                        region.last_line = last;
                    }
                    Some(region) => problems.push(format!(
                        "{shown}:{number}: `{name}` is a region of {} already",
                        region.shown
                    )),
                    None => {
                        regions.insert(
                            (stage, name.to_owned()),
                            Region {
                                path: path.to_owned(),
                                shown: shown.to_owned(),
                                language,
                                parts: vec![(first, lines)],
                                first_line: first,
                                last_line: last,
                            },
                        );
                    }
                }
                continue;
            }
            None => {
                for (_, lines) in open.values_mut() {
                    lines.push(line.to_owned());
                }
            }
        }
        if marker(line).is_none() {
            clean.push_str(line);
            clean.push('\n');
        }
    }
    for (name, (first, _)) in open {
        problems.push(format!(
            "{shown}:{}: `[learn:{name}]` is never closed",
            first - 1
        ));
    }
    clean
}

/// A line diff of `old` against `new`: lines starting with `+`, `-` or a
/// space, three lines of context around each change, `…` between hunks.
fn diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len() > MAX_LINES || b.len() > MAX_LINES {
        return "(too long to show)".to_owned();
    }
    // Trim what both share at either end, then LCS the middle.
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let (n, m) = (am.len(), bm.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if am[i] == bm[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // (kind, line): ' ' keeps, '-' removes, '+' adds.
    let mut ops: Vec<(char, &str)> = a[..prefix].iter().map(|l| (' ', *l)).collect();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && am[i] == bm[j] {
            ops.push((' ', am[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            ops.push(('+', bm[j]));
            j += 1;
        } else {
            ops.push(('-', am[i]));
            i += 1;
        }
    }
    ops.extend(a[a.len() - suffix..].iter().map(|l| (' ', *l)));

    // Keep the changes and CONTEXT lines around them.
    let mut keep = vec![false; ops.len()];
    for (at, (kind, _)) in ops.iter().enumerate() {
        if *kind != ' ' {
            let from = at.saturating_sub(CONTEXT);
            let to = (at + CONTEXT + 1).min(ops.len());
            keep[from..to].iter_mut().for_each(|k| *k = true);
        }
    }
    let mut lines = Vec::new();
    let mut skipped = false;
    for (at, (kind, line)) in ops.iter().enumerate() {
        if keep[at] {
            if skipped && !lines.is_empty() {
                lines.push(" …".to_owned());
            }
            skipped = false;
            lines.push(format!("{kind}{line}"));
        } else {
            skipped = true;
        }
    }
    lines.join("\n")
}

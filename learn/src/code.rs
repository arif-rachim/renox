//! The course app's code, read by build.rs from `course/stage-NN/`: the named
//! regions chapters include (`[learn:name]` marker comments in the stage's
//! files), the whole files, and what changed in each stage against the one
//! before. See `build_course.rs` for the marker rules.

/// A named part of a file of one stage.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// The stage it belongs to.
    pub stage: u8,
    /// Its name, `[a-z0-9._-]+`.
    pub name: &'static str,
    /// The file, relative to the stage's folder.
    pub path: &'static str,
    /// What the code is highlighted as.
    pub language: &'static str,
    /// The first line of its text in the file (from 1).
    pub first_line: usize,
    /// The last line of its text in the file.
    pub last_line: usize,
    /// The text, without marker lines and common indentation.
    pub text: &'static str,
}

/// A file of one stage, without marker lines.
#[derive(Debug, Clone, Copy)]
pub struct File {
    /// The stage it belongs to.
    pub stage: u8,
    /// Relative to the stage's folder.
    pub path: &'static str,
    /// Its text.
    pub text: &'static str,
}

/// One file that differs from the stage before.
#[derive(Debug, Clone, Copy)]
pub struct Change {
    /// The stage that made the change.
    pub stage: u8,
    /// Relative to the stage's folder.
    pub path: &'static str,
    /// `added`, `changed` or `removed`.
    pub kind: &'static str,
    /// Lines starting with `+`, `-` or a space.
    pub diff: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/course_code.rs"));

/// The region `name` of `stage`.
pub fn region(stage: u8, name: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.stage == stage && r.name == name)
}

/// The file `path` of `stage`.
pub fn file(stage: u8, path: &str) -> Option<&'static File> {
    FILES.iter().find(|f| f.stage == stage && f.path == path)
}

/// What `stage` changed against the stage before it, by path.
pub fn changes(stage: u8) -> impl Iterator<Item = &'static Change> {
    CHANGES.iter().filter(move |c| c.stage == stage)
}

/// The language a file is highlighted as, from its extension.
pub fn language_of(path: &str) -> &'static str {
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

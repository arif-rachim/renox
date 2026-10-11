//! Reads `challenges/sNN/<id>/` and writes `$OUT_DIR/challenges.rs`, which
//! `src/challenge.rs` includes: for every challenge folder its `challenge.md`,
//! every file under `start/` and `solution/`, its `tests/*.rs` and the two
//! recorded outputs when they exist. Mistakes in the folders are reported by
//! `challenge::problems()`, not here.

use std::fmt::Write;
use std::path::{Path, PathBuf};

pub fn generate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("challenges");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut out = String::from(
        "/// A challenge folder as build.rs read it.\npub struct Raw {\n    /// `s02/cheap-items`.\n    pub dir: &'static str,\n    /// `challenge.md`, if there is one.\n    pub markdown: Option<&'static str>,\n    /// Files under `start/`, by relative path.\n    pub start: &'static [(&'static str, &'static str)],\n    /// Files under `solution/`, by relative path.\n    pub solution: &'static [(&'static str, &'static str)],\n    /// Files under `tests/`, by file name.\n    pub tests: &'static [(&'static str, &'static str)],\n    /// `output-failing.txt`.\n    pub failing: Option<&'static str>,\n    /// `output-passing.txt`.\n    pub passing: Option<&'static str>,\n}\n\n/// Every challenge folder (build.rs).\npub static RAW: &[Raw] = &[\n",
    );
    for stage in dirs(&root) {
        let stage_name = name(&stage);
        if !stage_name.starts_with('s') {
            continue;
        }
        for dir in dirs(&stage) {
            writeln!(out, "    Raw {{").unwrap();
            writeln!(
                out,
                "        dir: {:?},",
                format!("{stage_name}/{}", name(&dir))
            )
            .unwrap();
            writeln!(out, "        markdown: {},", opt(&dir.join("challenge.md"))).unwrap();
            for part in ["start", "solution", "tests"] {
                writeln!(out, "        {part}: &[").unwrap();
                let base = dir.join(part);
                for file in files(&base) {
                    let rel = file
                        .strip_prefix(&base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    println!("cargo:rerun-if-changed={}", file.display());
                    writeln!(
                        out,
                        "            ({rel:?}, include_str!({:?})),",
                        file.display().to_string()
                    )
                    .unwrap();
                }
                writeln!(out, "        ],").unwrap();
            }
            writeln!(
                out,
                "        failing: {},",
                opt(&dir.join("output-failing.txt"))
            )
            .unwrap();
            writeln!(
                out,
                "        passing: {},",
                opt(&dir.join("output-passing.txt"))
            )
            .unwrap();
            writeln!(out, "    }},").unwrap();
        }
    }
    out.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR").unwrap()).join("challenges.rs");
    std::fs::write(target, out).unwrap();
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// `Some(include_str!(…))` when the file exists, else `None`.
fn opt(p: &Path) -> String {
    if p.is_file() {
        println!("cargo:rerun-if-changed={}", p.display());
        format!("Some(include_str!({:?}))", p.display().to_string())
    } else {
        "None".into()
    }
}

/// The subdirectories of `dir`, sorted.
fn dirs(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .map(|e| {
            e.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Every file under `dir`, recursively, sorted.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if path.is_dir() {
                todo.push(path);
            } else if std::fs::read_to_string(&path).is_ok() {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

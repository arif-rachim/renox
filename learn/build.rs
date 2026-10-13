//! Also scans the stages of the course app (`course/stage-NN/`) for the code
//! the chapters include (`build_course.rs`).
//!
//! Compiles the chapters in: every `content/stages/*.md` becomes an entry of
//! `STAGE_FILES` (its file name and its text), so adding a chapter is adding
//! a file. The views and public files are compiled in too (`renox::embedded!()`):
//! rebuild when they change.

use std::fmt::Write;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("content/stages");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::from(
        "/// Every chapter's file name and Markdown (build.rs).\npub static STAGE_FILES: &[(&str, &str)] = &[\n",
    );
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path.file_name().unwrap().to_string_lossy();
        writeln!(
            out,
            "    ({name:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let target = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("stages.rs");
    std::fs::write(target, out).unwrap();
    for watched in ["resources", "public"] {
        println!("cargo:rerun-if-changed={watched}");
    }
    // The videos' manifest, compiled in (src/media.rs).
    let media = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("content/media.json");
    println!("cargo:rerun-if-changed={}", media.display());
    let target = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("media_json.rs");
    std::fs::write(
        target,
        format!(
            "/// `content/media.json` (build.rs).\npub static MEDIA_JSON: &str = include_str!({:?});\n",
            media.display().to_string()
        ),
    )
    .unwrap();
    course::generate();
    challenges::generate();
}

#[path = "build_course.rs"]
mod course;

#[path = "build_challenges.rs"]
mod challenges;

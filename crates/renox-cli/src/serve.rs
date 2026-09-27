use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
use serde_json::Value;

/// Paths whose changes need a rebuild. Views reload inside the running app.
const WATCH: &[&str] = &["src", "migrations", "build.rs", "Cargo.toml", ".env"];

pub fn run(cargo_args: &[String]) -> Result<()> {
    if !Path::new("Cargo.toml").is_file() {
        bail!("no Cargo.toml here; run `rnx serve` from your app's directory");
    }

    let (tx, rx) = mpsc::channel::<DebounceEventResult>();
    let mut debouncer = new_debouncer(Duration::from_millis(300), tx)?;
    for path in WATCH.iter().map(Path::new).filter(|p| p.exists()) {
        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)
            .with_context(|| format!("could not watch {}", path.display()))?;
    }

    let mut snapshot = fingerprint();
    let mut app: Option<Child> = None;
    loop {
        match build(cargo_args)? {
            Some(exe) if migrate(&exe)? => {
                if let Some(mut old) = app.take() {
                    stop(&mut old);
                }
                app = Some(start(&exe)?);
            }
            Some(_) if app.is_some() => {
                eprintln!("\nrnx: migrations failed; the previous version keeps running.")
            }
            Some(_) => eprintln!("\nrnx: migrations failed; waiting for changes…"),
            None if app.is_some() => {
                eprintln!("\nrnx: build failed; the previous version keeps running.")
            }
            None => eprintln!("\nrnx: build failed; waiting for changes…"),
        }

        snapshot = wait_for_change(&rx, snapshot);
        eprintln!("\nrnx: change detected, rebuilding…");
    }
}

/// Runs `cargo build` and returns the path of the app's binary.
pub(crate) fn build(cargo_args: &[String]) -> Result<Option<PathBuf>> {
    let output = Command::new("cargo")
        .args(["build", "--message-format=json-render-diagnostics"])
        .args(cargo_args)
        .stderr(Stdio::inherit())
        .output()
        .context("could not run cargo")?;
    if !output.status.success() {
        return Ok(None);
    }

    let exe = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|msg| msg["reason"] == "compiler-artifact")
        .filter_map(|msg| msg["executable"].as_str().map(PathBuf::from))
        .next_back();
    match exe {
        Some(exe) => Ok(Some(exe)),
        None => bail!("the build produced no binary; is this a Renox app?"),
    }
}

/// Runs pending migrations with the new binary. Prints its output unless
/// there was nothing to do.
fn migrate(exe: &Path) -> Result<bool> {
    let output = Command::new(exe)
        .arg("migrate")
        .output()
        .with_context(|| format!("could not run {} migrate", exe.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim() != "Nothing to do." {
        print!("{stdout}");
    }
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(output.status.success())
}

fn start(exe: &Path) -> Result<Child> {
    Command::new(exe)
        .spawn()
        .with_context(|| format!("could not start {}", exe.display()))
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Blocks until a watched file's contents change and returns the new fingerprint.
///
/// The watcher also reports files being opened (e.g. by `cargo build` reading
/// `src/`), so events only wake us up; the fingerprint decides.
fn wait_for_change(
    rx: &mpsc::Receiver<DebounceEventResult>,
    previous: Vec<(PathBuf, SystemTime, u64)>,
) -> Vec<(PathBuf, SystemTime, u64)> {
    for result in rx {
        if let Err(err) = result {
            eprintln!("rnx: watch error: {err}");
            continue;
        }
        let current = fingerprint();
        if current != previous {
            return current;
        }
    }
    previous
}

/// Modification time and size of every watched file, sorted by path.
fn fingerprint() -> Vec<(PathBuf, SystemTime, u64)> {
    let mut files = Vec::new();
    for path in WATCH {
        collect(Path::new(path), &mut files);
    }
    files.sort();
    files
}

fn collect(path: &Path, files: &mut Vec<(PathBuf, SystemTime, u64)>) {
    let Ok(meta) = path.metadata() else { return };
    if meta.is_dir() {
        let Ok(entries) = path.read_dir() else { return };
        for entry in entries.flatten() {
            collect(&entry.path(), files);
        }
    } else if !is_ignored(path) {
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        files.push((path.to_path_buf(), modified, meta.len()));
    }
}

/// Editor swap and backup files.
fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| (n.starts_with('.') && n != ".env") || n.ends_with('~'))
}

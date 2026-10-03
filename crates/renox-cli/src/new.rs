use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::Database;

/// Files of a new app: (path, contents). `.stub` files are templated.
const STUBS: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../stubs/Cargo.toml.stub")),
    ("AGENTS.md", include_str!("../stubs/AGENTS.md.stub")),
    // Claude Code reads CLAUDE.md; it imports AGENTS.md so there's one text.
    ("CLAUDE.md", include_str!("../stubs/CLAUDE.md.stub")),
    (".env", include_str!("../stubs/env.stub")),
    (".env.example", include_str!("../stubs/env.stub")),
    (".gitignore", include_str!("../stubs/gitignore.stub")),
    ("build.rs", include_str!("../stubs/build.rs")),
    ("migrations/.gitkeep", ""),
    ("src/lib.rs", include_str!("../stubs/src/lib.rs")),
    ("src/main.rs", include_str!("../stubs/src/main.rs")),
    ("tests/home.rs", include_str!("../stubs/tests/home.rs")),
    ("src/app/mod.rs", include_str!("../stubs/src/app/mod.rs")),
    (
        "src/app/home/mod.rs",
        include_str!("../stubs/src/app/home/mod.rs"),
    ),
    (
        "resources/views/layouts/app.html",
        include_str!("../stubs/resources/views/layouts/app.html"),
    ),
    (
        "resources/views/home/index.html",
        include_str!("../stubs/resources/views/home/index.html"),
    ),
    (
        "resources/views/errors/default.html",
        include_str!("../stubs/resources/views/errors/default.html"),
    ),
    ("public/app.css", include_str!("../stubs/public/app.css")),
    (
        "resources/lang/en.json",
        include_str!("../stubs/resources/lang/en.json"),
    ),
];

const RENOX_GIT: &str = "https://github.com/arif-rachim/renox";

/// Renox from GitHub, pinned to the commit this `rnx` was built from, so
/// `cargo update` doesn't move the app to an API it wasn't written for.
fn git_dependency(rev: Option<&str>) -> String {
    match rev {
        Some(rev) => format!("renox = {{ git = \"{RENOX_GIT}\", rev = \"{rev}\""),
        // Built without git: follow main (move to a `rev` when you can).
        None => format!("renox = {{ git = \"{RENOX_GIT}\", branch = \"main\""),
    }
}

/// The Renox repository at `rev` (or `main`), for links in AGENTS.md.
fn docs_url(rev: Option<&str>) -> String {
    format!("{RENOX_GIT}/blob/{}", rev.unwrap_or("main"))
}

/// Renox from crates.io, at this `rnx`'s own version (`cargo install
/// renox-cli` builds from the registry, without git): a caret requirement,
/// since a minor release doesn't break apps.
fn registry_dependency(version: &str) -> String {
    let major_minor: Vec<&str> = version.split('.').take(2).collect();
    format!("renox = {{ version = \"{}\"", major_minor.join("."))
}

pub fn run(
    name: &str,
    renox_path: Option<&Path>,
    database: Database,
    tailwind: bool,
) -> Result<()> {
    run_in(Path::new("."), name, renox_path, database, tailwind)
}

/// `run`, making the app in `parent` (tests use a temporary directory).
fn run_in(
    parent: &Path,
    name: &str,
    renox_path: Option<&Path>,
    database: Database,
    tailwind: bool,
) -> Result<()> {
    validate_name(name)?;
    let root = &parent.join(name);
    if root.exists() {
        bail!("`{name}` already exists");
    }

    let rev = option_env!("RENOX_GIT_REV");
    // Where AGENTS.md sends coding agents: the docs of the Renox this app
    // compiles against, not `main`, which may have APIs it doesn't.
    let (dependency, docs, source) = match renox_path {
        Some(path) => {
            let crate_dir = path.join("crates/renox");
            let crate_dir = crate_dir
                .canonicalize()
                .with_context(|| format!("{} is not a Renox checkout", path.display()))?;
            let checkout = crate_dir
                .parent()
                .and_then(Path::parent)
                .unwrap_or(&crate_dir)
                .display()
                .to_string();
            (
                format!("renox = {{ path = {:?}", crate_dir.display().to_string()),
                docs_url(None),
                format!("the local Renox checkout this app uses: `{checkout}`"),
            )
        }
        None if rev.is_none() && option_env!("RENOX_FROM_CRATES_IO").is_some() => {
            let version = env!("CARGO_PKG_VERSION");
            (
                registry_dependency(version),
                format!("{RENOX_GIT}/blob/v{version}"),
                format!(
                    "the crates Cargo downloaded: `~/.cargo/registry/src/*/renox-core-{version}/`"
                ),
            )
        }
        None => (
            git_dependency(rev),
            docs_url(rev),
            match rev {
                Some(rev) => format!(
                    "the checkout Cargo downloaded: `~/.cargo/git/checkouts/renox-*/{}/`",
                    &rev[..7.min(rev.len())]
                ),
                None => "the checkout Cargo downloaded: `~/.cargo/git/checkouts/renox-*/*/`".into(),
            },
        ),
    };
    let dependency = match database {
        Database::Sqlite => format!("{dependency} }}"),
        Database::Postgres => format!("{dependency}, features = [\"postgres\"] }}"),
    };
    let crate_name = name.replace('-', "_");
    let postgres = format!("postgres://postgres:postgres@localhost:5432/{crate_name}");
    let (database_url, test_database_url) = match database {
        Database::Sqlite => (
            "sqlite://storage/app.db".to_owned(),
            format!("# TEST_DATABASE_URL={postgres}_test"),
        ),
        Database::Postgres => (
            postgres.clone(),
            format!("TEST_DATABASE_URL={postgres}_test"),
        ),
    };
    let key = crate::generate_key();

    for (file, contents) in STUBS {
        let contents = contents
            .replace("{{name}}", name)
            .replace("{{title}}", &title(name))
            .replace("{{renox_dependency}}", &dependency)
            .replace("{{renox_docs}}", &docs)
            .replace("{{renox_source}}", &source)
            .replace("{{crate_name}}", &crate_name)
            .replace("{{database_url}}", &database_url)
            .replace("{{test_database_url}}", &test_database_url);
        // Only the real .env gets a key; .env.example stays shareable.
        let contents = match *file {
            ".env" => contents.replace("{{app_key}}", &key),
            _ => contents.replace("{{app_key}}", ""),
        };
        let path = root.join(file);
        fs::create_dir_all(path.parent().expect("stub paths have a parent"))?;
        fs::write(&path, contents)
            .with_context(|| format!("could not write {}", path.display()))?;
    }

    if tailwind {
        use_tailwind(root)?;
    }

    match database {
        Database::Sqlite => println!("Created {name}. Next:\n\n    cd {name}\n    rnx serve\n"),
        Database::Postgres => println!(
            "Created {name}. Next: create the `{crate_name}` and `{crate_name}_test` databases \
             (or edit DATABASE_URL and TEST_DATABASE_URL in .env), then\n\n    cd {name}\n    rnx serve\n"
        ),
    }
    Ok(())
}

/// `--tailwind`: the app's styles move into Tailwind's input, the layout
/// links the built file, and the CSS is built once if the CLI can be had.
fn use_tailwind(root: &Path) -> Result<()> {
    let own = fs::read_to_string(root.join("public/app.css"))?;
    fs::remove_file(root.join("public/app.css"))?;
    let input = root.join(crate::tailwind::INPUT);
    fs::create_dir_all(input.parent().expect("the input has a parent"))?;
    // The app's own rules, without the stub's header comment.
    let rules = own
        .split_once("*/")
        .map_or(own.as_str(), |(_, rest)| rest.trim_start());
    fs::write(
        &input,
        format!(
            "{}
{rules}",
            crate::tailwind::INPUT_STUB
        ),
    )?;

    let layout = root.join("resources/views/layouts/app.html");
    let html = fs::read_to_string(&layout)?.replace(
        "<link rel=\"stylesheet\" href=\"{{ asset('app.css') }}\">",
        "{#- Built by Tailwind from resources/css/app.css (`rnx serve`, `rnx build`). -#}\n  \
         <link rel=\"stylesheet\" href=\"{{ asset('css/app.css') }}\">",
    );
    fs::write(&layout, html)?;
    let home = root.join("resources/views/home/index.html");
    let html = fs::read_to_string(&home)?.replace(
        "    <p class=\"rx-subtitle\">{{ t('home.edit') }}</p>",
        "    <p class=\"rx-subtitle\">{{ t('home.edit') }}</p>\n    \
         <p class=\"mt-2 text-sm font-medium text-emerald-700 dark:text-emerald-400\">\
         Tailwind is on: edit resources/css/app.css, or use its classes in any view.</p>",
    );
    fs::write(&home, html)?;

    if let Err(err) = crate::tailwind::build(root, false) {
        eprintln!("rnx: Tailwind isn't built yet ({err:#}); `rnx serve` will try again.");
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    let valid = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        bail!(
            "`{name}` is not a valid name: use lowercase letters, digits, `-` and `_`, starting with a letter"
        );
    }
    let crate_name = name.replace('-', "_");
    if crate::generate::is_reserved(&crate_name) {
        bail!(
            "`{name}` can't be an app's name: `{crate_name}` is a Rust keyword or a crate the app uses"
        );
    }
    Ok(())
}

/// `coffee-shop` -> `Coffee Shop`
fn title(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        assert!(validate_name("shop").is_ok());
        assert!(validate_name("coffee-shop_2").is_ok());
        assert!(validate_name("Shop").is_err());
        assert!(validate_name("2shop").is_err());
        assert!(validate_name("../shop").is_err());
        for reserved in ["renox", "fn", "self", "type"] {
            assert!(validate_name(reserved).is_err(), "{reserved}");
        }
    }

    #[test]
    fn pins_the_git_dependency() {
        assert_eq!(registry_dependency("1.2.3"), "renox = { version = \"1.2\"");
        let rev = "0123456789abcdef0123456789abcdef01234567";
        assert!(git_dependency(Some(rev)).ends_with(&format!("rev = \"{rev}\"")));
        assert!(git_dependency(None).ends_with("branch = \"main\""));
        // AGENTS.md links the docs of that same commit.
        assert_eq!(
            docs_url(Some(rev)),
            format!("https://github.com/arif-rachim/renox/blob/{rev}")
        );
        assert!(docs_url(None).ends_with("/blob/main"));
    }

    /// A written file with `\n` line ends (Windows checkouts give the stubs `\r\n`).
    fn read_lf(path: std::path::PathBuf) -> String {
        fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    }

    #[test]
    fn makes_an_app_with_every_placeholder_filled() {
        let dir = tempfile::tempdir().unwrap();
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        run_in(
            dir.path(),
            "coffee-shop",
            Some(&checkout),
            Database::Sqlite,
            false,
        )
        .unwrap();
        let root = dir.path().join("coffee-shop");
        for (file, _) in STUBS {
            let text = fs::read_to_string(root.join(file)).unwrap();
            // `{{name}}`-style placeholders (templates' `{{ x }}` has spaces).
            let left = text.split("{{").skip(1).any(|rest| {
                rest.split_once("}}").is_some_and(|(inner, _)| {
                    !inner.is_empty() && inner.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                })
            });
            assert!(!left, "{file} still has a placeholder");
        }
        let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"coffee-shop\""), "{cargo}");
        assert!(cargo.contains("renox = { path = "), "{cargo}");
        // The real .env has a key; the example doesn't.
        let env = read_lf(root.join(".env"));
        assert!(env.contains("APP_KEY=base64:"), "{env}");
        let example = read_lf(root.join(".env.example"));
        assert!(example.contains("APP_KEY=\n"), "{example}");
        assert!(env.contains("DATABASE_URL=sqlite://storage/app.db"));
        // AGENTS.md names the crate in its test example.
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("coffee_shop::app()"));
        // Not twice, and not over a bad name or a checkout that isn't Renox.
        assert!(run_in(dir.path(), "coffee-shop", None, Database::Sqlite, false).is_err());
        assert!(run_in(dir.path(), "Shop", None, Database::Sqlite, false).is_err());
        assert!(
            run_in(
                dir.path(),
                "other",
                Some(dir.path()),
                Database::Sqlite,
                false
            )
            .is_err()
        );
    }

    #[test]
    fn postgres_apps_point_at_their_databases() {
        let dir = tempfile::tempdir().unwrap();
        run_in(dir.path(), "kasir", None, Database::Postgres, false).unwrap();
        let root = dir.path().join("kasir");
        let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("features = [\"postgres\"]"), "{cargo}");
        let env = read_lf(root.join(".env"));
        assert!(env.contains("DATABASE_URL=postgres://postgres:postgres@localhost:5432/kasir\n"));
        assert!(env.contains(
            "\nTEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/kasir_test"
        ));
    }

    #[test]
    fn titles_names() {
        assert_eq!(title("coffee-shop_downtown"), "Coffee Shop Downtown");
    }
}

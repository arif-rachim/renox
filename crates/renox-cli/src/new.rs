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
    ("public/app.css", include_str!("../stubs/public/app.css")),
    (
        "resources/lang/en.json",
        include_str!("../stubs/resources/lang/en.json"),
    ),
    (
        "resources/lang/id.json",
        include_str!("../stubs/resources/lang/id.json"),
    ),
];

const RENOX_GIT: &str = "https://github.com/arif-rachim/renox";

pub fn run(name: &str, renox_path: Option<&Path>, database: Database) -> Result<()> {
    validate_name(name)?;
    let root = Path::new(name);
    if root.exists() {
        bail!("`{name}` already exists");
    }

    let dependency = match renox_path {
        Some(path) => {
            let crate_dir = path.join("crates/renox");
            let crate_dir = crate_dir
                .canonicalize()
                .with_context(|| format!("{} is not a Renox checkout", path.display()))?;
            format!("renox = {{ path = {:?}", crate_dir.display().to_string())
        }
        None => format!("renox = {{ git = \"{RENOX_GIT}\""),
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

    match database {
        Database::Sqlite => println!("Created {name}. Next:\n\n    cd {name}\n    rnx serve\n"),
        Database::Postgres => println!(
            "Created {name}. Next: create the `{crate_name}` and `{crate_name}_test` databases \
             (or edit DATABASE_URL and TEST_DATABASE_URL in .env), then\n\n    cd {name}\n    rnx serve\n"
        ),
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

/// `toko-kopi` -> `Toko Kopi`
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
        assert!(validate_name("toko").is_ok());
        assert!(validate_name("toko-kopi_2").is_ok());
        assert!(validate_name("Toko").is_err());
        assert!(validate_name("2toko").is_err());
        assert!(validate_name("../toko").is_err());
        for reserved in ["renox", "fn", "self", "type"] {
            assert!(validate_name(reserved).is_err(), "{reserved}");
        }
    }

    #[test]
    fn titles_names() {
        assert_eq!(title("toko-kopi_nusantara"), "Toko Kopi Nusantara");
    }
}

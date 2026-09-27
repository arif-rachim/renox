use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// Files of a new app: (path, contents). `.stub` files are templated.
const STUBS: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../stubs/Cargo.toml.stub")),
    (".env", include_str!("../stubs/env.stub")),
    (".env.example", include_str!("../stubs/env.stub")),
    (".gitignore", include_str!("../stubs/gitignore.stub")),
    ("build.rs", include_str!("../stubs/build.rs")),
    ("migrations/.gitkeep", ""),
    ("src/main.rs", include_str!("../stubs/src/main.rs")),
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

pub fn run(name: &str, renox_path: Option<&Path>) -> Result<()> {
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
            format!("renox = {{ path = {:?} }}", crate_dir.display().to_string())
        }
        None => format!("renox = {{ git = \"{RENOX_GIT}\" }}"),
    };
    let key = crate::generate_key();

    for (file, contents) in STUBS {
        let contents = contents
            .replace("{{name}}", name)
            .replace("{{title}}", &title(name))
            .replace("{{renox_dependency}}", &dependency);
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

    println!("Created {name}. Next:\n\n    cd {name}\n    rnx serve\n");
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
    }

    #[test]
    fn titles_names() {
        assert_eq!(title("toko-kopi_nusantara"), "Toko Kopi Nusantara");
    }
}

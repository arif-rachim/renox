use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// Creates `<timestamp>_<name>.up.sql` and `.down.sql`.
pub fn migration(name: &str, dir: &Path) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        bail!("`{name}` is not a valid migration name: use snake_case, e.g. create_produk_table");
    }

    fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    let stem = format!("{}_{name}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
    let (up_sql, down_sql) = template(name);
    for (suffix, contents) in [("up", up_sql), ("down", down_sql)] {
        let path = dir.join(format!("{stem}.{suffix}.sql"));
        if path.exists() {
            bail!("{} already exists", path.display());
        }
        fs::write(&path, contents)
            .with_context(|| format!("could not write {}", path.display()))?;
        println!("Created {}", path.display());
    }
    Ok(())
}

/// A starting point: a table for `create_<table>_table`, otherwise comments.
fn template(name: &str) -> (String, String) {
    match name
        .strip_prefix("create_")
        .and_then(|rest| rest.strip_suffix("_table"))
    {
        Some(table) => (
            format!(
                "CREATE TABLE {table} (\n    \
                 id INTEGER PRIMARY KEY AUTOINCREMENT,\n    \
                 created_at TEXT,\n    \
                 updated_at TEXT\n);\n"
            ),
            format!("DROP TABLE {table};\n"),
        ),
        None => (format!("-- {name}\n"), format!("-- Undo {name}\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_table_templates() {
        let (up, down) = template("create_produk_table");
        assert!(up.starts_with("CREATE TABLE produk ("));
        assert_eq!(down, "DROP TABLE produk;\n");
        assert!(template("add_stok_to_produk").0.starts_with("-- "));
    }

    #[test]
    fn writes_both_files() {
        let dir = tempfile_dir();
        migration("create_produk_table", &dir).unwrap();
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("_create_produk_table.down.sql"));
        assert!(files[1].ends_with("_create_produk_table.up.sql"));
        assert!(migration("Bad Name", &dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("renox-make-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }
}

//! `rnx make:*` generators. Each writes new files into the app (never over
//! existing ones) and wires them up where that's safe to do automatically.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use heck::{ToKebabCase, ToSnakeCase, ToTitleCase, ToUpperCamelCase};

/// `path` relative to the app root, for messages.
fn shown(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn write_new(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        bail!("{} already exists", shown(path));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("could not write {}", path.display()))?;
    println!("Created {}", shown(path));
    Ok(())
}

fn check_name(name: &str) -> Result<()> {
    let ok = name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !ok {
        bail!(
            "`{name}` is not a valid name: use letters, digits, `_` or `-`, starting with a letter"
        );
    }
    let snake = name.to_snake_case();
    if is_reserved(&snake) {
        bail!("`{name}` can't be used: `{snake}` is a Rust keyword or a crate the app uses");
    }
    Ok(())
}

/// Rust keywords (current and reserved) and crate names an app's modules
/// and crate must not take.
pub(crate) fn is_reserved(word: &str) -> bool {
    const RESERVED: &[&str] = &[
        "as",
        "async",
        "await",
        "break",
        "const",
        "continue",
        "crate",
        "dyn",
        "else",
        "enum",
        "extern",
        "false",
        "fn",
        "for",
        "gen",
        "if",
        "impl",
        "in",
        "let",
        "loop",
        "match",
        "mod",
        "move",
        "mut",
        "pub",
        "ref",
        "return",
        "self",
        "static",
        "struct",
        "super",
        "trait",
        "true",
        "type",
        "unsafe",
        "use",
        "where",
        "while",
        "abstract",
        "become",
        "box",
        "do",
        "final",
        "macro",
        "override",
        "priv",
        "try",
        "typeof",
        "unsized",
        "virtual",
        "yield",
        "std",
        "core",
        "alloc",
        "renox",
        "renox_core",
        "renox_macros",
        "serde",
        "tokio",
        "test",
    ];
    RESERVED.contains(&word)
}

/// Adds `pub mod {name};` to a `mod.rs`, after its other module lines.
fn add_mod(mod_rs: &Path, name: &str) -> Result<()> {
    let source = fs::read_to_string(mod_rs).unwrap_or_default();
    let line = format!("pub mod {name};");
    if source
        .lines()
        .any(|l| l.trim() == line || l.trim() == format!("mod {name};"))
    {
        return Ok(());
    }
    let mut lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .rposition(|l| l.starts_with("pub mod ") || l.starts_with("mod "))
        .map_or(0, |i| i + 1);
    lines.insert(at, &line);
    let mut out = lines.join("\n");
    out.push('\n');
    if let Some(dir) = mod_rs.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(mod_rs, out)?;
    println!("Updated {}", shown(mod_rs));
    Ok(())
}

fn module_dir(root: &Path, module: &str) -> Result<PathBuf> {
    let dir = root.join("src/app").join(module);
    if !dir.join("mod.rs").is_file() {
        bail!("there is no module `{module}`; create it with `rnx make:module {module}`");
    }
    Ok(dir)
}

/// `rnx make:module stok_barang`: a module with an index route and view.
pub fn module(root: &Path, name: &str) -> Result<()> {
    check_name(name)?;
    let snake = name.to_snake_case();
    let pascal = name.to_upper_camel_case();
    let kebab = name.to_kebab_case();
    let title = name.to_title_case();

    write_new(
        &root.join("src/app").join(&snake).join("mod.rs"),
        &format!(
            r#"use renox::prelude::*;

pub struct {pascal};

impl Module for {pascal} {{
    fn name(&self) -> &'static str {{
        "{snake}"
    }}

    fn routes(&self) -> Routes {{
        Routes::new().get("/{kebab}", index).name("{snake}.index")
    }}
}}

async fn index() -> View {{
    view("{snake}/index.html", context! {{}})
}}
"#
        ),
    )?;
    write_new(
        &root.join("resources/views").join(&snake).join("index.html"),
        &format!(
            "{{% extends \"layouts/app.html\" %}}\n\n{{% block content %}}\n<h1>{title}</h1>\n{{% endblock %}}\n"
        ),
    )?;
    add_mod(&root.join("src/app/mod.rs"), &snake)?;
    // Apps from `rnx new` build the App in src/lib.rs; older ones in main.rs.
    let lib = root.join("src/lib.rs");
    let target = if lib.is_file() {
        lib
    } else {
        root.join("src/main.rs")
    };
    register_in_main(&target, &format!("app::{snake}::{pascal}"))
}

/// Adds `.module(path)` after the last `.module(` call in `lib.rs`/`main.rs`.
fn register_in_main(main_rs: &Path, path: &str) -> Result<()> {
    let hint = || println!("Register it where the App is built: .module({path})");
    let Ok(source) = fs::read_to_string(main_rs) else {
        hint();
        return Ok(());
    };
    if source.contains(&format!(".module({path})")) {
        return Ok(());
    }
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let Some(last) = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with(".module("))
    else {
        hint();
        return Ok(());
    };
    let indent: String = lines[last]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    lines.insert(last + 1, format!("{indent}.module({path})"));
    fs::write(main_rs, lines.join("\n") + "\n")?;
    println!("Updated {} (.module({path}))", shown(main_rs));
    Ok(())
}

/// `rnx make:model Produk [--module produk] [--migration]`.
pub fn model(root: &Path, name: &str, module: Option<&str>, migration: bool) -> Result<()> {
    check_name(name)?;
    let pascal = name.to_upper_camel_case();
    let table = name.to_snake_case();
    let module = module.map_or_else(|| table.clone(), |m| m.to_snake_case());
    let dir = module_dir(root, &module)?;
    let file = if dir.join("model.rs").exists() {
        table.clone()
    } else {
        "model".into()
    };

    write_new(
        &dir.join(format!("{file}.rs")),
        &format!(
            r#"use renox::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "{table}")]
pub struct {pascal} {{
    pub id: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}}
"#
        ),
    )?;
    add_mod(&dir.join("mod.rs"), &file)?;
    if migration {
        crate::make::migration(&format!("create_{table}_table"), &root.join("migrations"))?;
    }
    Ok(())
}

/// `rnx make:job KirimStruk --module pesanan`.
pub fn job(root: &Path, name: &str, module: &str) -> Result<()> {
    check_name(name)?;
    let pascal = name.to_upper_camel_case();
    let snake = name.to_snake_case();
    let module = module.to_snake_case();
    let dir = module_dir(root, &module)?;
    write_new(
        &dir.join(format!("{snake}.rs")),
        &format!(
            r#"use renox::prelude::*;
use serde::{{Deserialize, Serialize}};

#[derive(Serialize, Deserialize)]
pub struct {pascal} {{}}

impl Job for {pascal} {{
    const NAME: &'static str = "{kebab}";

    async fn handle(self, ctx: JobContext) -> Result {{
        let _ = ctx;
        Ok(())
    }}
}}
"#,
            kebab = name.to_kebab_case()
        ),
    )?;
    add_mod(&dir.join("mod.rs"), &snake)?;
    println!("Register it in the module's `register`: app.job::<{snake}::{pascal}>();");
    Ok(())
}

/// `rnx make:command admin:create --module users`: an app command in the
/// module, run as `my-app admin:create …`.
pub fn command(root: &Path, name: &str, module: &str) -> Result<()> {
    let valid = name.split(':').all(|part| {
        part.starts_with(|c: char| c.is_ascii_lowercase())
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    });
    if !valid {
        bail!(
            "`{name}` is not a valid command name: use lowercase words separated by `:`, e.g. admin:create"
        );
    }
    let snake = name.replace([':', '-'], "_");
    if is_reserved(&snake) {
        bail!("`{name}` can't be used: `{snake}` is a Rust keyword or a crate the app uses");
    }
    let module = module.to_snake_case();
    let dir = module_dir(root, &module)?;
    write_new(
        &dir.join(format!("{snake}.rs")),
        &format!(
            r#"use renox::command::Args;
use renox::prelude::*;

/// `my-app {name} …`
pub async fn run(state: AppState, args: Args) -> Result {{
    let _ = (state, args);
    println!("{name}: done");
    Ok(())
}}
"#
        ),
    )?;
    add_mod(&dir.join("mod.rs"), &snake)?;
    println!(
        "Register it in the module's `register`: app.command(\"{name}\", \"What it does\", {snake}::run);"
    );
    Ok(())
}

/// `rnx make:policy Produk --module produk`: `impl Policy` for a model.
pub fn policy(root: &Path, model: &str, module: &str) -> Result<()> {
    check_name(model)?;
    let pascal = model.to_upper_camel_case();
    let module = module.to_snake_case();
    let dir = module_dir(root, &module)?;
    write_new(
        &dir.join("policy.rs"),
        &format!(
            r#"use renox::prelude::*;

use super::model::{pascal};

impl Policy for {pascal} {{
    fn allows(&self, user: &User, ability: &str) -> bool {{
        let _ = user;
        match ability {{
            "view" => true,
            // e.g. "update" | "delete" => self.user_id == user.id,
            _ => false,
        }}
    }}
}}
"#
        ),
    )?;
    add_mod(&dir.join("mod.rs"), "policy")
}

/// `rnx make:mail pesanan_dikirim`: an HTML and a text template.
pub fn mail(root: &Path, name: &str) -> Result<()> {
    check_name(name)?;
    let snake = name.to_snake_case();
    let dir = root.join("resources/views/mail");
    write_new(
        &dir.join(format!("{snake}.html")),
        r#"{% extends "renox/mail/layout.html" %}
{% from "renox/mail/button.html" import button %}
{% block content %}
<p>Hello!</p>
{{ button(app.url, "Open " ~ app.name) }}
{% endblock %}
"#,
    )?;
    write_new(
        &dir.join(format!("{snake}.txt")),
        "Hello!\n\n{{ app.url }}\n",
    )?;
    println!("Send it with: state.mail_view(to, subject, \"mail/{snake}\", context! {{}})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/app/home")).unwrap();
        fs::write(dir.path().join("src/app/mod.rs"), "pub mod home;\n").unwrap();
        fs::write(
            dir.path().join("src/app/home/mod.rs"),
            "use renox::prelude::*;\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/main.rs"),
            "mod app;\n\nfn main() -> renox::Result {\n    renox::App::new()\n        .migrations(renox::migrations!())\n        .module(app::home::Home)\n        .run()\n}\n",
        )
        .unwrap();
        dir
    }

    fn read(dir: &tempfile::TempDir, path: &str) -> String {
        fs::read_to_string(dir.path().join(path)).unwrap()
    }

    #[test]
    fn modules_are_created_and_registered() {
        let dir = app();
        module(dir.path(), "stok_barang").unwrap();
        let code = read(&dir, "src/app/stok_barang/mod.rs");
        assert!(code.contains("pub struct StokBarang;"));
        assert!(
            code.contains(r#"Routes::new().get("/stok-barang", index).name("stok_barang.index")"#)
        );
        assert!(
            read(&dir, "resources/views/stok_barang/index.html").contains("<h1>Stok Barang</h1>")
        );
        assert_eq!(
            read(&dir, "src/app/mod.rs"),
            "pub mod home;\npub mod stok_barang;\n"
        );
        assert!(read(&dir, "src/main.rs").contains(
            "        .module(app::home::Home)\n        .module(app::stok_barang::StokBarang)\n        .run()"
        ));
        assert!(
            module(dir.path(), "stok_barang").is_err(),
            "never overwrites"
        );
        assert!(module(dir.path(), "../evil").is_err());
    }

    #[test]
    fn models_jobs_policies_and_mails() {
        let dir = app();
        module(dir.path(), "produk").unwrap();
        model(dir.path(), "Produk", None, true).unwrap();
        let code = read(&dir, "src/app/produk/model.rs");
        assert!(
            code.contains(r#"#[model(table = "produk")]"#) && code.contains("pub struct Produk {")
        );
        let migrations: Vec<_> = fs::read_dir(dir.path().join("migrations"))
            .unwrap()
            .collect();
        assert_eq!(migrations.len(), 2, "up and down");

        model(dir.path(), "Kategori", Some("produk"), false).unwrap();
        assert!(
            dir.path().join("src/app/produk/kategori.rs").exists(),
            "model.rs is taken"
        );

        job(dir.path(), "KirimStruk", "produk").unwrap();
        assert!(
            read(&dir, "src/app/produk/kirim_struk.rs")
                .contains(r#"const NAME: &'static str = "kirim-struk";"#)
        );
        command(dir.path(), "stok:import", "produk").unwrap();
        assert!(read(&dir, "src/app/produk/stok_import.rs").contains("pub async fn run("));
        assert!(command(dir.path(), "Bad Name", "produk").is_err());
        assert!(command(dir.path(), "self", "produk").is_err());
        policy(dir.path(), "Produk", "produk").unwrap();
        assert!(read(&dir, "src/app/produk/policy.rs").contains("impl Policy for Produk"));

        let mods = read(&dir, "src/app/produk/mod.rs");
        for m in [
            "pub mod model;",
            "pub mod kategori;",
            "pub mod kirim_struk;",
            "pub mod stok_import;",
            "pub mod policy;",
        ] {
            assert!(mods.contains(m), "{mods}");
        }
        assert!(
            model(dir.path(), "Order", Some("nope"), false).is_err(),
            "unknown module"
        );

        mail(dir.path(), "pesanan_dikirim").unwrap();
        assert!(
            read(&dir, "resources/views/mail/pesanan_dikirim.html")
                .contains("renox/mail/layout.html")
        );
        assert!(
            dir.path()
                .join("resources/views/mail/pesanan_dikirim.txt")
                .exists()
        );
    }
}

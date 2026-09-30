//! `rnx`: the command-line tool for the Renox web framework.

mod deploy;
mod generate;
mod make;
mod new;
mod scaffold;
mod serve;
mod tailwind;

use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "rnx",
    version,
    about = "rnx: the Renox web framework CLI",
    after_help = "Any other command runs in your app (cargo run -- <command>), e.g.\n  rnx queue:work, rnx queue:failed, rnx schedule:list"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new Renox application.
    New {
        /// Directory and package name, e.g. `toko`.
        name: String,
        /// Use a local checkout of Renox instead of the Git repository.
        #[arg(long, value_name = "DIR")]
        renox_path: Option<PathBuf>,
        /// The database the app starts with.
        #[arg(long, value_enum, default_value_t = Database::Sqlite)]
        database: Database,
        /// Style pages with Tailwind CSS (its standalone CLI, no Node) next to the UI kit.
        #[arg(long)]
        tailwind: bool,
    },
    /// Run the app, rebuilding and restarting it when source files change.
    Serve {
        /// Extra arguments for `cargo build`, e.g. `--release`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cargo_args: Vec<String>,
    },
    /// Create a migration in `migrations/`, e.g. `create_produk_table`.
    #[command(name = "make:migration")]
    MakeMigration {
        /// Snake-case description, e.g. `create_produk_table`.
        name: String,
        /// Directory for the migration files.
        #[arg(long, default_value = "migrations")]
        path: PathBuf,
    },
    /// Create a module: routes, an index view, and its registration.
    #[command(name = "make:module")]
    MakeModule {
        /// e.g. `produk` or `stok_barang`.
        name: String,
        /// A whole resource: model, migration, factory, form, the seven
        /// handlers (`Routes::resource`), views on the UI kit, and tests.
        #[arg(long)]
        resource: bool,
        /// With --resource: the fields, e.g. "name:string price:money
        /// notes:text active:bool due:date" (string, text, int, money, float,
        /// bool, date). Default: name:string.
        #[arg(long)]
        fields: Option<String>,
        /// With --resource: the model's name (default: the module's in the
        /// singular, `products` → `Product`).
        #[arg(long)]
        model: Option<String>,
    },
    /// Fake records for a model (`impl Factory`).
    #[command(name = "make:factory")]
    MakeFactory {
        model: String,
        #[arg(long)]
        module: String,
    },
    /// A seeder for `db:seed`, registered on the App.
    #[command(name = "make:seeder")]
    MakeSeeder { name: String },
    /// An integration test file in tests/.
    #[command(name = "make:test")]
    MakeTest { name: String },
    /// A notification (mail and database) in a module.
    #[command(name = "make:notification")]
    MakeNotification {
        name: String,
        #[arg(long)]
        module: String,
    },
    /// An event and a listener registered in its module.
    #[command(name = "make:event")]
    MakeEvent {
        name: String,
        #[arg(long)]
        module: String,
    },
    /// A validation rule (`impl Rule`) in a module.
    #[command(name = "make:rule")]
    MakeRule {
        name: String,
        #[arg(long)]
        module: String,
    },
    /// A middleware on every route of the app, registered with App::layer.
    #[command(name = "make:middleware")]
    MakeMiddleware { name: String },
    /// Create a model (and with --migration, its table's migration).
    #[command(name = "make:model")]
    MakeModel {
        /// e.g. `Produk`.
        name: String,
        /// Module to put it in; defaults to the model's name in snake_case.
        #[arg(long)]
        module: Option<String>,
        /// Also create a `create_<table>_table` migration.
        #[arg(short, long)]
        migration: bool,
        /// The `id` type: a database-numbered integer, a ULID or UUID made
        /// on insert, or a string the app sets.
        #[arg(long, value_enum, default_value = "integer")]
        key: KeyType,
    },
    /// Create a queued job in a module.
    #[command(name = "make:job")]
    MakeJob {
        /// e.g. `KirimStruk`.
        name: String,
        #[arg(long)]
        module: String,
    },
    /// Create an app command in a module (`my-app <name>`).
    #[command(name = "make:command")]
    MakeCmd {
        /// e.g. `admin:create`.
        name: String,
        #[arg(long)]
        module: String,
    },
    /// Create a policy for a module's model.
    #[command(name = "make:policy")]
    MakePolicy {
        /// The model, e.g. `Produk`.
        model: String,
        #[arg(long)]
        module: String,
    },
    /// Create a view component (a macro that can use `old`, `error`, `t`…),
    /// or with `--ui` copy Renox's UI kit into the app to change it.
    #[command(name = "make:component")]
    MakeComponent {
        /// e.g. `price_tag`: resources/views/components/price_tag.html.
        name: Option<String>,
        /// Copy renox/ui.html and its CSS into the app (`my-app ui:publish`).
        #[arg(long)]
        ui: bool,
        /// With --ui: replace files already there.
        #[arg(long)]
        force: bool,
    },
    /// Create an HTML and a text mail template.
    #[command(name = "make:mail")]
    MakeMail {
        /// e.g. `pesanan_dikirim`.
        name: String,
    },
    /// Run pending migrations.
    Migrate {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        args: Vec<String>,
    },
    /// Undo the last batch of migrations (`--step N` for more).
    #[command(name = "migrate:rollback")]
    MigrateRollback {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Drop all tables and run every migration (`--seed` to seed too).
    #[command(name = "migrate:fresh")]
    MigrateFresh {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// List migrations and whether they have run.
    #[command(name = "migrate:status")]
    MigrateStatus,
    /// Run the app's seeders.
    #[command(name = "db:seed")]
    DbSeed,
    /// Build a release binary into dist/ (and, with Tailwind, minified CSS first).
    Build,
    /// Build public/css/app.css with Tailwind (`rnx serve` and `rnx build` do it for you).
    Tailwind {
        /// Rebuild whenever a view or the input changes.
        #[arg(long)]
        watch: bool,
        /// Optimize and minify, as `rnx build` does.
        #[arg(long)]
        minify: bool,
    },
    /// Download the pinned Tailwind CSS standalone CLI (checked by SHA-256) and print its path.
    #[command(name = "tailwind:install")]
    TailwindInstall,
    /// Create a Dockerfile, a systemd unit, a Litestream config and a deploy guide.
    #[command(name = "make:deploy")]
    MakeDeploy,
    /// Generate an APP_KEY and write it to `.env`.
    #[command(name = "key:generate")]
    KeyGenerate {
        /// Print the key instead of writing it to `.env`.
        #[arg(long)]
        show: bool,
    },
    /// Any other command is run by the app itself.
    #[command(external_subcommand)]
    App(Vec<String>),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New {
            name,
            renox_path,
            database,
            tailwind,
        } => new::run(&name, renox_path.as_deref(), database, tailwind),
        Command::Tailwind { watch, minify } => tailwind::run(&app_root()?, watch, minify),
        Command::TailwindInstall => {
            println!("{}", tailwind::binary()?.display());
            Ok(())
        }
        Command::Serve { cargo_args } => serve::run(&cargo_args),
        Command::KeyGenerate { show } => key_generate(show),
        Command::Build => deploy::build(&app_root()?),
        Command::MakeDeploy => deploy::make_deploy(&app_root()?),
        Command::MakeMigration { name, path } => make::migration(&name, &path),
        Command::MakeModule {
            name,
            resource: true,
            fields,
            model,
        } => scaffold::resource(&app_root()?, &name, model.as_deref(), fields.as_deref()),
        Command::MakeModule { name, .. } => generate::module(&app_root()?, &name),
        Command::MakeFactory { model, module } => generate::factory(&app_root()?, &model, &module),
        Command::MakeSeeder { name } => generate::seeder(&app_root()?, &name),
        Command::MakeTest { name } => generate::test(&app_root()?, &name),
        Command::MakeNotification { name, module } => {
            generate::notification(&app_root()?, &name, &module)
        }
        Command::MakeEvent { name, module } => generate::event(&app_root()?, &name, &module),
        Command::MakeRule { name, module } => generate::rule(&app_root()?, &name, &module),
        Command::MakeMiddleware { name } => generate::middleware(&app_root()?, &name),
        Command::MakeModel {
            name,
            module,
            migration,
            key,
        } => generate::model(&app_root()?, &name, module.as_deref(), migration, key),
        Command::MakeJob { name, module } => generate::job(&app_root()?, &name, &module),
        Command::MakeCmd { name, module } => generate::command(&app_root()?, &name, &module),
        Command::MakePolicy { model, module } => generate::policy(&app_root()?, &model, &module),
        Command::MakeMail { name } => generate::mail(&app_root()?, &name),
        Command::MakeComponent {
            ui: true, force, ..
        } => {
            let args: Vec<String> = if force {
                vec!["--force".into()]
            } else {
                Vec::new()
            };
            app_command("ui:publish", &args)
        }
        Command::MakeComponent {
            name: Some(name), ..
        } => generate::component(&app_root()?, &name),
        Command::MakeComponent { .. } => Err(anyhow::anyhow!("give a name, or --ui")),
        Command::Migrate { args } => app_command("migrate", &args),
        Command::MigrateRollback { args } => app_command("migrate:rollback", &args),
        Command::MigrateFresh { args } => app_command("migrate:fresh", &args),
        Command::MigrateStatus => app_command("migrate:status", &[]),
        Command::DbSeed => app_command("db:seed", &[]),
        Command::App(args) => match args.split_first() {
            Some((command, rest)) => app_command(command, rest),
            None => Ok(()),
        },
    }
}

/// Runs a command built into the app binary (`cargo run -- <command>`), since
/// migrations and seeders are compiled into the app.
fn app_command(command: &str, args: &[String]) -> Result<()> {
    if !std::path::Path::new("Cargo.toml").is_file() {
        anyhow::bail!("no Cargo.toml here; run `rnx {command}` from your app's directory");
    }
    let status = std::process::Command::new("cargo")
        .args(["run", "--quiet", "--", command])
        .args(args)
        .status()
        .context("could not run cargo")?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// The current directory, if it looks like a Renox app.
/// The database engine an app uses.
/// A model's key type (`make:model --key`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum KeyType {
    Integer,
    Ulid,
    Uuid,
    String,
}

impl KeyType {
    /// The `id` field's Rust type.
    pub(crate) fn rust_type(self) -> &'static str {
        match self {
            KeyType::Integer => "i64",
            KeyType::Ulid => "Ulid",
            KeyType::Uuid => "Uuid",
            KeyType::String => "String",
        }
    }

    /// The `id` column in a `CREATE TABLE`.
    pub(crate) fn column(self, database: Database) -> &'static str {
        match (self, database) {
            (KeyType::Integer, Database::Sqlite) => "id INTEGER PRIMARY KEY AUTOINCREMENT",
            (KeyType::Integer, Database::Postgres) => {
                "id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY"
            }
            (KeyType::Uuid, Database::Sqlite) => "id BLOB PRIMARY KEY",
            (KeyType::Uuid, Database::Postgres) => "id UUID PRIMARY KEY",
            (KeyType::Ulid | KeyType::String, _) => "id TEXT PRIMARY KEY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Database {
    Sqlite,
    Postgres,
}

impl Database {
    /// The app's engine, from `DATABASE_URL` in the environment or in `.env`
    /// in the current directory; SQLite when neither says otherwise.
    pub(crate) fn of_current_app() -> Self {
        let url = std::env::var("DATABASE_URL").ok().or_else(|| {
            std::fs::read_to_string(".env").ok().and_then(|env| {
                env.lines()
                    .find_map(|line| line.trim().strip_prefix("DATABASE_URL="))
                    .map(|url| url.trim().trim_matches('"').to_owned())
            })
        });
        Self::of_url(url.as_deref().unwrap_or_default())
    }

    pub(crate) fn of_url(url: &str) -> Self {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Database::Postgres
        } else {
            Database::Sqlite
        }
    }
}

fn app_root() -> Result<PathBuf> {
    let root = std::env::current_dir()?;
    if !root.join("Cargo.toml").is_file() || !root.join("src").is_dir() {
        anyhow::bail!("run this from your app's directory (the one with Cargo.toml and src/)");
    }
    Ok(root)
}

pub(crate) fn generate_key() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    format!("base64:{}", STANDARD.encode(bytes))
}

fn key_generate(show: bool) -> Result<()> {
    let key = generate_key();
    if show {
        println!("{key}");
        return Ok(());
    }

    // No .env yet (a fresh clone): start one from .env.example, or empty.
    let env = match std::fs::read_to_string(".env") {
        Ok(env) => env,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if !std::path::Path::new("Cargo.toml").is_file() {
                anyhow::bail!("no .env or Cargo.toml here; run from your app or pass --show");
            }
            std::fs::read_to_string(".env.example").unwrap_or_default()
        }
        Err(err) => return Err(err).context("could not read .env"),
    };
    let mut replaced = false;
    let mut lines: Vec<String> = env
        .lines()
        .map(|line| {
            // `export APP_KEY=…` (a .env also sourced by a shell) keeps its `export`.
            let export = line.starts_with("export ");
            let rest = line.strip_prefix("export ").unwrap_or(line).trim_start();
            if rest.starts_with("APP_KEY=") && !replaced {
                replaced = true;
                format!("{}APP_KEY={key}", if export { "export " } else { "" })
            } else if rest.starts_with("APP_KEY=") {
                // A second definition would override the new key.
                format!("# {line}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !replaced {
        lines.push(format!("APP_KEY={key}"));
    }
    std::fs::write(".env", lines.join("\n") + "\n").context("could not write .env")?;
    println!("APP_KEY written to .env. Existing sessions are now invalid.");
    Ok(())
}

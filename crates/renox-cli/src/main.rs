//! `rnx`: the command-line tool for the Renox web framework.

mod deploy;
mod generate;
mod make;
mod new;
mod serve;

use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};

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
    },
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
    },
    /// Create a queued job in a module.
    #[command(name = "make:job")]
    MakeJob {
        /// e.g. `KirimStruk`.
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
    /// Build a release binary into dist/.
    Build,
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
        Command::New { name, renox_path } => new::run(&name, renox_path.as_deref()),
        Command::Serve { cargo_args } => serve::run(&cargo_args),
        Command::KeyGenerate { show } => key_generate(show),
        Command::Build => deploy::build(&app_root()?),
        Command::MakeDeploy => deploy::make_deploy(&app_root()?),
        Command::MakeMigration { name, path } => make::migration(&name, &path),
        Command::MakeModule { name } => generate::module(&app_root()?, &name),
        Command::MakeModel {
            name,
            module,
            migration,
        } => generate::model(&app_root()?, &name, module.as_deref(), migration),
        Command::MakeJob { name, module } => generate::job(&app_root()?, &name, &module),
        Command::MakePolicy { model, module } => generate::policy(&app_root()?, &model, &module),
        Command::MakeMail { name } => generate::mail(&app_root()?, &name),
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

    let env = std::fs::read_to_string(".env")
        .context("no .env in this directory; run from your app or pass --show")?;
    let mut replaced = false;
    let mut lines: Vec<String> = env
        .lines()
        .map(|line| {
            if line.starts_with("APP_KEY=") {
                replaced = true;
                format!("APP_KEY={key}")
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

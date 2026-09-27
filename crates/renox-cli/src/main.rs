//! `renox`: the command-line tool for the Renox web framework.

mod make;
mod new;
mod serve;

use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "renox", version, about = "The Renox web framework CLI")]
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
    /// Generate an APP_KEY and write it to `.env`.
    #[command(name = "key:generate")]
    KeyGenerate {
        /// Print the key instead of writing it to `.env`.
        #[arg(long)]
        show: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New { name, renox_path } => new::run(&name, renox_path.as_deref()),
        Command::Serve { cargo_args } => serve::run(&cargo_args),
        Command::KeyGenerate { show } => key_generate(show),
        Command::MakeMigration { name, path } => make::migration(&name, &path),
        Command::Migrate { args } => app_command("migrate", &args),
        Command::MigrateRollback { args } => app_command("migrate:rollback", &args),
        Command::MigrateFresh { args } => app_command("migrate:fresh", &args),
        Command::MigrateStatus => app_command("migrate:status", &[]),
        Command::DbSeed => app_command("db:seed", &[]),
    }
}

/// Runs a command built into the app binary (`cargo run -- <command>`), since
/// migrations and seeders are compiled into the app.
fn app_command(command: &str, args: &[String]) -> Result<()> {
    if !std::path::Path::new("Cargo.toml").is_file() {
        anyhow::bail!("no Cargo.toml here; run `renox {command}` from your app's directory");
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

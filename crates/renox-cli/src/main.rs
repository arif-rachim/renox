//! `renox`: the command-line tool for the Renox web framework.

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
    }
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

//! `products:import`: products from a CSV file, each line in a savepoint.
//! Made with `rnx make:command products:import --module products`.

use renox::clap;
use renox::command::AppCommand;
use renox::prelude::*;

use super::model::Product;

/// Import products from a CSV file of `name,price` lines.
///
/// Everything goes in one transaction; each line runs in a savepoint, so a
/// bad line (a price that isn't a number, a name the `saving` hook refuses,
/// a database error) undoes only its own writes and the import goes on. On
/// PostgreSQL that matters twice: a failed statement stops a whole
/// transaction, unless it failed inside a savepoint.
#[derive(clap::Parser)]
#[command(name = "products:import")]
pub struct ImportProducts {
    /// The CSV file (`name,price` per line; a `name,price` header is skipped).
    pub file: std::path::PathBuf,
    /// The owner's email.
    #[arg(long)]
    pub owner: String,
}

impl AppCommand for ImportProducts {
    async fn run(self, state: AppState) -> Result {
        let text = std::fs::read_to_string(&self.file)
            .map_err(|err| Error::BadRequest(format!("{}: {err}", self.file.display())))?;
        let owner = User::find_by_email(&state.db, &self.owner)
            .await?
            .ok_or_else(|| Error::BadRequest(format!("no user {}", self.owner)))?;
        let report = import(&state.db, &owner, &text).await?;
        println!(
            "Imported {}, skipped {}.",
            report.imported,
            report.skipped.len()
        );
        for (line, reason) in &report.skipped {
            println!("  line {line}: {reason}");
        }
        Ok(())
    }
}

/// What an import did.
#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    /// Line numbers (from 1) and why they were left out.
    pub skipped: Vec<(usize, String)>,
}

/// Imports `csv` for `owner`: one transaction, a savepoint per line.
pub async fn import(db: &Db, owner: &User, csv: &str) -> Result<ImportReport> {
    let mut report = ImportReport::default();
    let mut tx = db.begin().await?;
    for (index, line) in csv.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || (number == 1 && line.trim() == "name,price") {
            continue;
        }
        let row = parse(line);
        let owner_id = owner.id;
        let outcome = tx
            .savepoint(|tx| {
                Box::pin(async move {
                    let (name, price) = row?;
                    let product = Product {
                        user_id: owner_id,
                        name,
                        price,
                        ..Default::default()
                    };
                    Product::create(&mut *tx, product).await?; // the hooks run
                    Ok(())
                })
            })
            .await;
        match outcome {
            Ok(()) => report.imported += 1,
            Err(err) => report.skipped.push((number, reason(&err))),
        }
    }
    tx.commit().await?;
    Ok(report)
}

fn parse(line: &str) -> Result<(String, i64)> {
    let (name, price) = line
        .rsplit_once(',')
        .ok_or_else(|| Error::BadRequest("expected `name,price`".into()))?;
    let price = price
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|price| *price >= 0)
        .ok_or_else(|| Error::BadRequest(format!("`{}` isn't a price", price.trim())))?;
    Ok((name.trim().to_owned(), price))
}

/// A short reason for the report: a validation error's first message.
fn reason(err: &Error) -> String {
    match err {
        Error::BadRequest(message) => message.clone(),
        Error::Validation(invalid) => invalid
            .errors
            .iter()
            .next()
            .and_then(|(_, messages)| messages.first().cloned())
            .unwrap_or_else(|| "invalid".into()),
        other => format!("{other:?}"),
    }
}

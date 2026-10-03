//! Products from a CSV file chosen in the browser (the "Import" sheet, a
//! multipart form sent with htmx). Lines are `sku,name,price,stock`; a new
//! SKU adds a product, a known one updates its name and price; `stock` is
//! received into the ledger. Everything runs in one transaction with a
//! savepoint per line, so a bad line is skipped and reported, and the rest
//! still land.

use renox::Toast;
use renox::prelude::*;
use serde::Deserialize;

use super::{Product, stock};

#[derive(Deserialize, Validate)]
pub struct ImportForm {
    #[validate(required, mimes(&["csv", "txt"]), max = 1024)]
    pub file: Option<Upload>,
}

/// What an import did.
#[derive(Debug, Default)]
pub struct Report {
    pub added: usize,
    pub updated: usize,
    /// Line numbers (from 1) and why they were skipped.
    pub skipped: Vec<(usize, String)>,
}

pub(crate) async fn upload(
    State(db): State<Db>,
    user: AuthUser,
    Valid(form): Valid<ImportForm>,
) -> Result<(Toast, HxRefresh)> {
    let Some(file) = form.file else {
        return Err(Error::BadRequest("No file was sent.".into()));
    };
    let text = String::from_utf8_lossy(&file.bytes);
    let report = import(&db, &text, &user.name).await?;
    let mut toast = Toast::success(format!(
        "{} added, {} updated.",
        report.added, report.updated
    ));
    if !report.skipped.is_empty() {
        let lines: Vec<String> = report
            .skipped
            .iter()
            .take(5)
            .map(|(line, why)| format!("line {line}: {why}"))
            .collect();
        toast = Toast::warning(format!(
            "{} added, {} updated, {} skipped.",
            report.added,
            report.updated,
            report.skipped.len()
        ))
        .body(lines.join("; "));
    }
    Ok((toast, HxRefresh))
}

/// Imports `csv`; `user` is named in the ledger.
pub async fn import(db: &Db, csv: &str, user: &str) -> Result<Report> {
    let mut report = Report::default();
    let mut tx = db.begin().await?;
    for (index, line) in csv.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || (number == 1 && line.to_lowercase().starts_with("sku,")) {
            continue;
        }
        let row = parse(line);
        let user = user.to_owned();
        let outcome = tx
            .savepoint(|tx| {
                Box::pin(async move {
                    let (sku, name, price, quantity) = row?;
                    let existing = Product::where_eq("sku", &sku).first(&mut *tx).await?;
                    let added = existing.is_none();
                    let product = match existing {
                        Some(mut product) => {
                            product.name = name;
                            product.price = price;
                            product.save_only(&mut *tx, &["name", "price"]).await?;
                            product
                        }
                        None => {
                            Product::create(
                                &mut *tx,
                                Product {
                                    sku,
                                    name,
                                    price,
                                    active: true,
                                    ..Default::default()
                                },
                            )
                            .await?
                        }
                    };
                    if quantity > 0 {
                        stock::change(
                            &mut *tx,
                            stock::Change {
                                product_id: product.id,
                                quantity,
                                reason: "imported",
                                note: "CSV import",
                                invoice_id: None,
                                user_name: &user,
                            },
                        )
                        .await?;
                    }
                    Ok(added)
                })
            })
            .await;
        match outcome {
            Ok(true) => report.added += 1,
            Ok(false) => report.updated += 1,
            Err(Error::BadRequest(why)) => report.skipped.push((number, why)),
            Err(err) => {
                tracing::warn!(line = number, ?err, "an import line failed");
                report
                    .skipped
                    .push((number, "it could not be saved".into()));
            }
        }
    }
    tx.commit().await?;
    Ok(report)
}

/// `sku,name,price,stock`; the name may contain commas when quoted.
fn parse(line: &str) -> Result<(String, String, i64, i64)> {
    let fields = split(line);
    let [sku, name, price, quantity] = fields.as_slice() else {
        return Err(Error::BadRequest("expected sku,name,price,stock".into()));
    };
    let sku = sku.trim().to_uppercase();
    let valid_sku = !sku.is_empty()
        && sku.len() <= 30
        && sku
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_sku {
        return Err(Error::BadRequest(format!("`{sku}` isn't a SKU")));
    }
    let name = name.trim().to_owned();
    if name.is_empty() || name.len() > 100 {
        return Err(Error::BadRequest("the name is empty or too long".into()));
    }
    let number = |text: &str, what: &str| {
        text.trim()
            .parse::<i64>()
            .ok()
            .filter(|n| *n >= 0)
            .ok_or_else(|| Error::BadRequest(format!("`{}` isn't a {what}", text.trim())))
    };
    Ok((
        sku,
        name,
        number(price, "price")?,
        number(quantity, "quantity")?,
    ))
}

/// Splits a CSV line, with `"…"` around fields that hold commas.
fn split(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                if let Some(field) = fields.last_mut() {
                    field.push('"');
                }
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            c => {
                if let Some(field) = fields.last_mut() {
                    field.push(c);
                }
            }
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quoted_names() {
        assert_eq!(
            parse(r#"cf-01,"Coffee, robusta",25000,10"#).unwrap(),
            ("CF-01".into(), "Coffee, robusta".into(), 25000, 10)
        );
        assert!(parse("CF-01,Coffee,pricey,1").is_err());
        assert!(parse("CF 01,Coffee,1,1").is_err());
    }
}

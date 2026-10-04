//! Products from a CSV file chosen in the browser (the kit's
//! `import_action` sheet, a multipart form sent with htmx), read by
//! `renox::import`. Columns are `sku,name,price,stock`; each row is checked
//! with `ProductRow`'s rules, like a form. A new SKU adds a product, a
//! known one updates its name and price; `stock` is received into the
//! ledger. The good rows are written in one transaction, a savepoint each;
//! the refused ones are listed in the sheet with their row numbers.

use renox::import::{Import, ImportReport};
use renox::prelude::*;
use renox::validation::ValidateHooks;
use serde::Deserialize;

use super::{Product, stock};

#[derive(Deserialize, Validate)]
pub struct ImportForm {
    #[validate(required, mimes(&["csv", "txt"]), max = 1024)]
    pub file: Option<Upload>,
}

/// One row of the file: the same rules as the "New product" form, except
/// that a known SKU is allowed (it updates that product).
#[derive(Deserialize, Validate)]
#[validate(hooks)]
pub struct ProductRow {
    #[validate(required, max = 30, alpha_dash)]
    pub sku: String,
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, min = 0)]
    pub price: i64,
    #[validate(min = 0, max = 100000)]
    pub stock: Option<i64>,
}

impl ValidateHooks for ProductRow {
    fn prepare(&mut self) {
        self.sku = self.sku.trim().to_uppercase();
        self.name = self.name.trim().to_owned();
    }
}

/// The columns, for the sheet and the template.
pub const COLUMNS: [&str; 4] = ["sku", "name", "price", "stock"];

pub(crate) async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    lang: Lang,
    Valid(form): Valid<ImportForm>,
) -> Result<ImportReport> {
    let Some(file) = form.file else {
        return Err(Error::BadRequest("No file was sent.".into()));
    };
    import(&state, Import::csv(file.bytes()).lang(&lang), &user.name).await
}

/// An empty file with the columns: "Download a template" in the sheet.
pub(crate) async fn template() -> renox::Download {
    renox::import::template("products.csv", &COLUMNS)
}

/// Runs `import`; `user` is named in the ledger.
pub async fn import(state: &AppState, import: Import, user: &str) -> Result<ImportReport> {
    let user = user.to_owned();
    import
        .run(state, move |tx, row: ProductRow| {
            let user = user.clone();
            Box::pin(async move {
                let existing = Product::where_eq("sku", &row.sku).first(&mut *tx).await?;
                let product = match existing {
                    Some(mut product) => {
                        product.name = row.name;
                        product.price = row.price;
                        product.save_only(&mut *tx, &["name", "price"]).await?;
                        product
                    }
                    None => {
                        Product::create(
                            &mut *tx,
                            Product {
                                sku: row.sku,
                                name: row.name,
                                price: row.price,
                                active: true,
                                ..Default::default()
                            },
                        )
                        .await?
                    }
                };
                if let Some(quantity) = row.stock.filter(|q| *q > 0) {
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
                Ok(())
            })
        })
        .await
}

//! The stock ledger: `products.stock` changes only through `change`, which
//! writes a `stock_movements` row in the same transaction. Taking stock is
//! one conditional `UPDATE … WHERE stock >= ?`, so two cashiers selling the
//! last unit can't both succeed, on SQLite or PostgreSQL, without reading
//! the row first.

use renox::Toast;
use renox::db::Transaction;
use renox::grid::{Column, Grid};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::Product;

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "stock_movements")]
pub struct StockMovement {
    pub id: i64,
    pub product_id: i64,
    /// Positive in, negative out.
    pub quantity: i64,
    /// `received`, `sold`, `returned`, `counted`, `damaged` or `imported`.
    pub reason: String,
    pub note: String,
    pub invoice_id: Option<i64>,
    pub user_name: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// A change to one product's stock.
pub struct Change<'a> {
    pub product_id: i64,
    /// Positive in, negative out.
    pub quantity: i64,
    pub reason: &'a str,
    pub note: &'a str,
    pub invoice_id: Option<i64>,
    pub user_name: &'a str,
}

/// Applies `change` and records it. `false` (and nothing changed) when it
/// would take more than is in stock.
pub async fn change(tx: &mut Transaction, change: Change<'_>) -> Result<bool> {
    let mut query = Product::where_eq("id", change.product_id);
    if change.quantity < 0 {
        query = query.where_op("stock", ">=", -change.quantity);
    }
    if query.increment(&mut *tx, "stock", change.quantity).await? == 0 {
        return Ok(false);
    }
    StockMovement::create(
        &mut *tx,
        StockMovement {
            product_id: change.product_id,
            quantity: change.quantity,
            reason: change.reason.into(),
            note: change.note.into(),
            invoice_id: change.invoice_id,
            user_name: change.user_name.into(),
            ..Default::default()
        },
    )
    .await?;
    Ok(true)
}

/// The ledger of one product, on its page.
pub fn ledger_grid() -> Grid {
    Grid::new("movements")
        .title("Stock ledger")
        .per_page(15)
        .column(Column::datetime("created_at", "When").mobile())
        .column(
            Column::select(
                "reason",
                "Reason",
                [
                    ("received", "Received"),
                    ("sold", "Sold"),
                    ("returned", "Returned"),
                    ("counted", "Counted"),
                    ("damaged", "Damaged"),
                    ("imported", "Imported"),
                ],
            )
            .mobile()
            .badges(&[
                ("received", "success"),
                ("imported", "success"),
                ("returned", "info"),
                ("sold", "neutral"),
                ("counted", "warning"),
                ("damaged", "danger"),
            ]),
        )
        .column(
            Column::number("quantity", "Change")
                .mobile()
                .summary(renox::grid::Summary::Sum),
        )
        .column(Column::text("note", "Note").searchable().limit(40))
        .column(
            Column::related("invoice", "Invoice", "invoices", "invoice_id", "number")
                .link("/invoices/{invoice_id}"),
        )
        .column(Column::text("user_name", "By"))
        .sort_by("-created_at")
        .empty_state("No movements yet", None)
}

#[derive(Deserialize, Serialize, Validate)]
pub struct AdjustForm {
    /// `received` adds, `damaged` takes away, `counted` sets the stock to
    /// what was counted on the shelf.
    #[validate(required, one_of(&["received", "damaged", "counted"]))]
    pub reason: String,
    #[validate(required, min = 0, max = 100000)]
    pub quantity: i64,
    #[validate(max = 200)]
    pub note: Option<String>,
}

pub(crate) async fn adjust(
    State(db): State<Db>,
    user: AuthUser,
    Path(id): Path<i64>,
    Valid(form): Valid<AdjustForm>,
) -> Result<(Toast, HxRefresh)> {
    let mut tx = db.begin().await?;
    let product = Product::find_or_404(&mut tx, id).await?;
    let quantity = match form.reason.as_str() {
        "received" => form.quantity,
        "damaged" => -form.quantity,
        // The difference between the shelf and the books.
        _ => form.quantity - product.stock,
    };
    let note = form.note.unwrap_or_default();
    let changed = quantity == 0
        || change(
            &mut tx,
            Change {
                product_id: id,
                quantity,
                reason: &form.reason,
                note: &note,
                invoice_id: None,
                user_name: &user.name,
            },
        )
        .await?;
    if !changed {
        let mut errors = Errors::new();
        errors.add(
            "quantity",
            format!("Only {} in stock to take away.", product.stock),
        );
        return Err(ValidationError::new(errors).into());
    }
    tx.commit().await?;
    let stock = product.stock + quantity;
    Ok((
        Toast::success(format!("{}: {stock} in stock.", product.name)),
        HxRefresh,
    ))
}

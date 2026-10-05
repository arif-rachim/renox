//! Products: a grid with the stock level (a custom cell), prices edited in
//! place, bulk "activate" and "deactivate", a "New product" wizard (the
//! kit's `wizard_action`) and a CSV import (`import.rs`, the kit's
//! `import_action`). A product's page shows its stock ledger (`stock.rs`),
//! with "Adjust stock" for staff with `stock.adjust`, and an action group:
//! "Duplicate" (`Model::replicate` into the new-product form) and "Export
//! ledger" (`Grid::export_as`, a CSV outside the grid's page).

pub mod import;
pub mod stock;

use renox::grid::{Action, Column, ExportFormat, Grid, GridRequest, Selection};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products")]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    /// In rupiah.
    pub price: i64,
    /// Kept by the ledger (`stock::change`); never set directly.
    pub stock: i64,
    /// Below this, the product is "low" (the dashboard lists it).
    pub min_stock: i64,
    pub active: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Product {
    /// `out`, `low` or `ok`: the stock cell's badge.
    pub fn level(&self) -> &'static str {
        if self.stock <= 0 {
            "out"
        } else if self.stock < self.min_stock {
            "low"
        } else {
            "ok"
        }
    }
}

/// Products below their minimum, the fewest first.
pub fn running_low() -> renox::db::Query<Product> {
    Product::where_eq("active", true)
        .where_raw("stock < min_stock", Vec::<i64>::new())
        .order_by("stock")
}

pub fn grid(user: &AuthUser) -> Grid {
    let grid = Grid::new("products")
        .title("Products")
        .column(Column::text("sku", "SKU").frozen().searchable().copyable())
        .column(
            Column::text("name", "Name")
                .mobile()
                .searchable()
                .editable(),
        )
        .column(Column::money("price", "Price (Rp)").mobile().editable())
        // Drawn in products/index.html: the number with a badge.
        .column(Column::custom("level", "Stock").mobile())
        .column(
            Column::number("stock", "In stock")
                .hidden()
                .summary(renox::grid::Summary::Sum),
        )
        .column(Column::number("min_stock", "Minimum").editable())
        .column(Column::bool("active", "Active").icons().editable())
        .column(Column::count_of("sold", "Invoice lines", "invoice_lines", "product_id").hidden())
        .sort_by("name")
        .row_url("/products/{id}")
        .cards_on_mobile()
        .exports()
        .empty_state(
            "No products yet",
            Some("Add one with New product, or import a CSV file."),
        );
    if !user.has_permission(crate::PRODUCTS) {
        return grid;
    }
    grid.edit_url("/products/{id}")
        .row_action(Action::link("Duplicate", "/products/{id}/replicate"))
        .bulk_action(Action::new("Activate", "/products/bulk/active/1"))
        .bulk_action(Action::new("Deactivate", "/products/bulk/active/0"))
}

pub(super) async fn index(request: GridRequest, user: AuthUser) -> Result<Response> {
    let grid = grid(&user);
    if let Some(file) = grid.export(Product::query(), &request).await? {
        return Ok(file);
    }
    let products = grid
        .page(Product::query(), &request)
        .await?
        .extend(|product| json!({ "level": product.level() }));
    Ok(view("products/index.html", context! { products }).into_response())
}

pub(super) async fn show(
    State(db): State<Db>,
    Path(id): Path<i64>,
    request: GridRequest,
) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    let movements = stock::ledger_grid()
        .page(stock::StockMovement::where_eq("product_id", id), &request)
        .await?;
    let level = product.level();
    Ok(view(
        "products/show.html",
        context! { product, level, movements },
    ))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct ProductForm {
    #[validate(required, max = 30, alpha_dash, unique("products", "sku"))]
    pub sku: String,
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, min = 0)]
    pub price: i64,
    #[validate(min = 0)]
    pub min_stock: Option<i64>,
    /// What is on the shelf today: the ledger's first row.
    #[validate(min = 0, max = 100000)]
    pub opening_stock: Option<i64>,
}

/// "Duplicate": the new-product form, filled from a copy of this product
/// (`replicate()`: no id, no timestamps). The SKU must be unique, so it
/// starts empty.
pub(super) async fn replicate(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let original = Product::find_or_404(&db, id).await?;
    let mut copy = original.replicate();
    copy.sku.clear();
    copy.name = format!("{} (copy)", original.name);
    Ok(view(
        "products/replicate.html",
        context! { original, product => copy },
    ))
}

/// "Export ledger": every movement of one product as CSV, outside the
/// ledger grid's page (its filters don't apply).
pub(super) async fn export_ledger(
    State(db): State<Db>,
    Path(id): Path<i64>,
    request: GridRequest,
) -> Result<Response> {
    let product = Product::find_or_404(&db, id).await?;
    let movements = stock::StockMovement::where_eq("product_id", product.id).order_by_desc("id");
    stock::ledger_grid()
        .export_as(movements, ExportFormat::Csv, &request)
        .await
}

pub(super) async fn store(
    State(db): State<Db>,
    user: AuthUser,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, HxRedirect)> {
    let mut tx = db.begin().await?;
    let product = Product::create(
        &mut tx,
        Product {
            sku: form.sku.to_uppercase(),
            name: form.name,
            price: form.price,
            min_stock: form.min_stock.unwrap_or(0),
            active: true,
            ..Default::default()
        },
    )
    .await?;
    if let Some(quantity) = form.opening_stock.filter(|q| *q > 0) {
        stock::change(
            &mut tx,
            stock::Change {
                product_id: product.id,
                quantity,
                reason: "received",
                note: "Opening stock",
                invoice_id: None,
                user_name: &user.name,
            },
        )
        .await?;
    }
    tx.commit().await?;
    // To the new product's page, from the wizard or the "Duplicate" form.
    Ok((
        Toast::success(format!("{} added.", product.name)),
        HxRedirect(format!("/products/{}", product.id)),
    ))
}

/// A cell edited in the grid. Stock isn't here: it changes through the
/// ledger only.
#[derive(Deserialize, Validate)]
pub struct ProductEdit {
    #[validate(max = 100)]
    pub name: Option<String>,
    #[validate(min = 0)]
    pub price: Option<i64>,
    #[validate(min = 0)]
    pub min_stock: Option<i64>,
    pub active: Option<bool>,
}

pub(super) async fn update(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Valid(edit): Valid<ProductEdit>,
) -> Result<Toast> {
    let mut product = Product::find_or_404(&db, id).await?;
    if let Some(name) = edit.name.filter(|n| !n.trim().is_empty()) {
        product.name = name.trim().to_owned();
    }
    if let Some(price) = edit.price {
        product.price = price;
    }
    if let Some(min_stock) = edit.min_stock {
        product.min_stock = min_stock;
    }
    if let Some(active) = edit.active {
        product.active = active;
    }
    product
        .save_only(&db, &["name", "price", "min_stock", "active"])
        .await?;
    Ok(Toast::success(format!("{} saved.", product.name)))
}

/// "Activate" / "Deactivate" over the selected (or all matching) products.
pub(super) async fn bulk_active(
    State(db): State<Db>,
    user: AuthUser,
    Path(active): Path<u8>,
    request: GridRequest,
    Form(selection): Form<Selection>,
) -> Result<Toast> {
    let active = active == 1;
    let changed = grid(&user)
        .selected(Product::query(), &request, &selection)?
        .update(&db, &[("active", &active)])
        .await?;
    let word = if active { "activated" } else { "deactivated" };
    Ok(Toast::success(format!("{changed} products {word}.")))
}

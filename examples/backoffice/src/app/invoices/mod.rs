//! Invoices: the grid (with an export made in the background, `export.rs`),
//! a form with line items (the kit's `repeater`, sent as
//! `lines[0][product_id]`, `lines[0][quantity]`…), and the life of an
//! invoice: issued (the stock leaves, through the ledger, in the same
//! transaction), paid (cash here, or online through `payments.rs`), void
//! (the stock comes back), printed.

pub mod export;
pub mod model;
pub mod payments;

use renox::chrono::{Days, NaiveDate};
use renox::grid::{Action, Column, Grid, GridRequest};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Settings;
use crate::app::customers::Customer;
use crate::app::products::{Product, stock};
pub use model::{Invoice, InvoiceLine, STATUS_TONES, STATUSES};

/// Today in `APP_TIMEZONE`: an invoice issued at 1 a.m. in Jakarta is
/// dated that day, not the day before (UTC).
pub fn today(config: &Config) -> NaiveDate {
    let zone: renox::timezone::Zone = config.timezone;
    zone.local(renox::db::now().timestamp()).date()
}

/// `can_export`: the bulk action that exports in the background.
pub fn grid(can_export: bool) -> Grid {
    let grid = Grid::new("invoices")
        .title("Invoices")
        .column(
            Column::text("number", "Number")
                .frozen()
                .mobile()
                .searchable()
                .copyable(),
        )
        .column(
            Column::related("customer", "Customer", "customers", "customer_id", "name")
                .mobile()
                .searchable(),
        )
        .column(
            Column::select("status", "Status", STATUSES)
                .mobile()
                .badges(&STATUS_TONES),
        )
        .column(Column::date("issued_on", "Issued"))
        .column(Column::date("due_on", "Due"))
        .column(
            Column::money("total", "Total (Rp)")
                .mobile()
                .summary(renox::grid::Summary::Sum),
        )
        .column(
            Column::money("tax", "Tax (Rp)")
                .hidden()
                .summary(renox::grid::Summary::Sum),
        )
        .column(Column::text("paid_via", "Paid via").hidden())
        .column(Column::count_of("lines", "Lines", "invoice_lines", "invoice_id").hidden())
        .sort_by("-issued_on")
        .groups(&["status"])
        .advanced_filter()
        .remember()
        .row_url("/invoices/{id}")
        .audit()
        .cards_on_mobile()
        .exports()
        .row_action(Action::link("Open", "/invoices/{id}"))
        .row_action(Action::link("Print", "/invoices/{id}/print"))
        .empty_state("No invoices yet", Some("Write one with New invoice."));
    if can_export {
        // The grid's filters go along: a file of every matching invoice (or
        // the selected ones), made by a job; the bell says when it's ready.
        grid.bulk_action(Action::new("Export in the background", "/invoices/export"))
    } else {
        grid
    }
}

pub(super) async fn index(request: GridRequest, user: AuthUser) -> Result<Response> {
    let grid = grid(user.has_permission(crate::EXPORTS));
    if let Some(file) = grid.export(Invoice::query(), &request).await? {
        return Ok(file);
    }
    let invoices = grid.page(Invoice::query(), &request).await?;
    Ok(view("invoices/index.html", context! { invoices }).into_response())
}

/// An invoice, its customer and lines, for the page and the print.
async fn load(db: &Db, id: i64) -> Result<(Invoice, Customer, Vec<InvoiceLine>)> {
    let invoice = Invoice::find_or_404(db, id).await?;
    let customer = Customer::find_or_404(db, invoice.customer_id).await?;
    let lines = InvoiceLine::where_eq("invoice_id", id)
        .order_by("id")
        .get(db)
        .await?;
    Ok((invoice, customer, lines))
}

pub(super) async fn show(State(state): State<AppState>, Path(id): Path<i64>) -> Result<View> {
    let (invoice, customer, lines) = load(&state.db, id).await?;
    let settings = Settings::load(&state.db).await?;
    let overdue = invoice.overdue(today(&state.config));
    let gateway = payments::configured(&state, &settings);
    Ok(view(
        "invoices/show.html",
        context! { invoice, customer, lines, overdue, gateway },
    ))
}

pub(super) async fn print(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let (invoice, customer, lines) = load(&db, id).await?;
    Ok(view(
        "invoices/print.html",
        context! { invoice, customer, lines },
    ))
}

pub(super) async fn create(State(db): State<Db>) -> Result<View> {
    let customers: Vec<(i64, String)> = Customer::query()
        .order_by("name")
        .get(&db)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let products: Vec<(i64, String)> = Product::where_eq("active", true)
        .order_by("name")
        .get(&db)
        .await?
        .into_iter()
        .map(|p| {
            (
                p.id,
                format!("{} · {} ({} in stock)", p.sku, p.name, p.stock),
            )
        })
        .collect();
    Ok(view(
        "invoices/create.html",
        context! { customers, products },
    ))
}

#[derive(Deserialize, Serialize)]
pub struct InvoiceForm {
    pub customer_id: i64,
    pub notes: Option<String>,
    #[serde(default)]
    pub lines: Vec<LineForm>,
}

#[derive(Deserialize, Serialize)]
pub struct LineForm {
    pub product_id: i64,
    pub quantity: i64,
    /// Empty: the product's price.
    pub unit_price: Option<i64>,
}

/// One line's rules; `v.nested` keys its errors `lines.0.quantity`, where
/// the repeater shows them.
impl Validate for LineForm {
    fn rules(&self, v: &mut Validator) {
        v.field("product_id", &self.product_id)
            .label("Product")
            .required()
            .exists("products", "id");
        v.field("quantity", &self.quantity)
            .required()
            .between(1, 10_000);
        v.field("unit_price", &self.unit_price)
            .label("Price")
            .min(0);
    }
}

impl Validate for InvoiceForm {
    fn rules(&self, v: &mut Validator) {
        v.field("customer_id", &self.customer_id)
            .label("Customer")
            .required()
            .exists("customers", "id");
        v.field("notes", &self.notes).max(500);
        v.field("lines", &self.lines)
            .label("Lines")
            .required()
            .min(1)
            .max(50);
        v.nested("lines", &self.lines);
    }
}

/// Saves a draft: lines at the product's price unless one was typed, the
/// tax from the settings, the number from the prefix and the id.
pub(super) async fn store(
    State(state): State<AppState>,
    user: AuthUser,
    Valid(form): Valid<InvoiceForm>,
) -> Result<Redirect> {
    let settings = Settings::load(&state.db).await?;
    let today = today(&state.config);
    let mut tx = state.db.begin().await?;
    let mut lines = Vec::with_capacity(form.lines.len());
    for line in &form.lines {
        let product = Product::find_or_404(&mut tx, line.product_id).await?;
        let unit_price = line.unit_price.unwrap_or(product.price);
        lines.push(InvoiceLine {
            product_id: product.id,
            description: product.name,
            quantity: line.quantity,
            unit_price,
            amount: unit_price * line.quantity,
            ..Default::default()
        });
    }
    let subtotal: i64 = lines.iter().map(|l| l.amount).sum();
    let tax = Invoice::tax_on(subtotal, settings.tax_percent);
    let mut invoice = Invoice::create(
        &mut tx,
        Invoice {
            customer_id: form.customer_id,
            status: "draft".into(),
            issued_on: today,
            due_on: today + Days::new(settings.payment_days.max(0) as u64),
            subtotal,
            tax,
            total: subtotal + tax,
            notes: form.notes.unwrap_or_default(),
            created_by: user.name.clone(),
            updated_by: user.name.clone(),
            ..Default::default()
        },
    )
    .await?;
    invoice.number = format!("{}{:05}", settings.invoice_prefix, invoice.id);
    invoice.save_only(&mut tx, &["number"]).await?;
    for mut line in lines {
        line.invoice_id = invoice.id;
        InvoiceLine::create(&mut tx, line).await?;
    }
    tx.commit().await?;
    Redirect::route("invoices.show", &[&invoice.id])
}

/// Moves an invoice from `from` to `to`, or answers 409 when someone else
/// moved it first (two clicks, two tabs).
async fn claim(
    tx: &mut renox::db::Transaction,
    id: i64,
    from: &[&str],
    to: &str,
    user: &str,
) -> Result {
    let moved = Invoice::where_eq("id", id)
        .where_in("status", from.iter().copied())
        .update(&mut *tx, &[("status", &to), ("updated_by", &user)])
        .await?;
    abort_if(
        moved == 0,
        StatusCode::CONFLICT,
        "This invoice has changed meanwhile; reload the page.",
    )
}

/// Issues a draft: dated today, due after the settings' payment days, and
/// each line's quantity taken from stock. Not enough stock for any line
/// and nothing changes.
pub(super) async fn issue(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let settings = Settings::load(&state.db).await?;
    let today = today(&state.config);
    let mut tx = state.db.begin().await?;
    claim(&mut tx, id, &["draft"], "issued", &user.name).await?;
    let lines = InvoiceLine::where_eq("invoice_id", id).get(&mut tx).await?;
    for line in &lines {
        let taken = stock::change(
            &mut tx,
            stock::Change {
                product_id: line.product_id,
                quantity: -line.quantity,
                reason: "sold",
                note: "",
                invoice_id: Some(id),
                user_name: &user.name,
            },
        )
        .await?;
        if !taken {
            // Dropping `tx` rolls everything back, the status too.
            let left = Product::find_or_404(&mut tx, line.product_id).await?.stock;
            return Err(abort(
                StatusCode::CONFLICT,
                format!(
                    "Only {left} of {} in stock; {} needed.",
                    line.description, line.quantity
                ),
            ));
        }
    }
    Invoice::where_eq("id", id)
        .update(
            &mut tx,
            &[
                ("issued_on", &today),
                (
                    "due_on",
                    &(today + Days::new(settings.payment_days.max(0) as u64)),
                ),
            ],
        )
        .await?;
    tx.commit().await?;
    Ok((
        Toast::success("Issued: the stock is taken."),
        Redirect::route("invoices.show", &[&id])?,
    ))
}

/// Paid in cash at the counter.
pub(super) async fn mark_paid(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let invoice = Invoice::find_or_404(&state.db, id).await?;
    abort_if(
        !payments::mark_paid(&state, &invoice.number, "cash", &user.name).await?,
        StatusCode::CONFLICT,
        "Only an issued invoice can be paid.",
    )?;
    Ok((
        Toast::success(format!("{} is paid.", invoice.number)),
        Redirect::route("invoices.show", &[&id])?,
    ))
}

/// Voids a draft or an unpaid invoice; an issued one's stock comes back.
pub(super) async fn void(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let invoice = Invoice::find_or_404(&state.db, id).await?;
    abort_if(
        !["draft", "issued"].contains(&invoice.status.as_str()),
        StatusCode::CONFLICT,
        "A paid invoice can't be voided.",
    )?;
    let mut tx = state.db.begin().await?;
    // From the status read above only: had it been issued meanwhile, its
    // stock would have to come back too.
    claim(&mut tx, id, &[&invoice.status], "void", &user.name).await?;
    if invoice.status == "issued" {
        let lines = InvoiceLine::where_eq("invoice_id", id).get(&mut tx).await?;
        for line in &lines {
            stock::change(
                &mut tx,
                stock::Change {
                    product_id: line.product_id,
                    quantity: line.quantity,
                    reason: "returned",
                    note: "Invoice voided",
                    invoice_id: Some(id),
                    user_name: &user.name,
                },
            )
            .await?;
        }
    }
    tx.commit().await?;
    renox::audit::record(
        &state.db,
        renox::audit::Entry::new("invoice.voided")
            .user(user.id)
            .subject("invoices", id)
            .data(json!({ "number": invoice.number, "was": invoice.status })),
    )
    .await?;
    Ok((
        Toast::success(format!("{} is void.", invoice.number)),
        Redirect::route("invoices.show", &[&id])?,
    ))
}

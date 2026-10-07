use renox::chrono::NaiveDate;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// An invoice: a draft until it's issued (the stock leaves then), then paid
/// (cash, or through the payment gateway's webhook) or void (the stock
/// comes back).
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "invoices")]
pub struct Invoice {
    pub id: i64,
    pub number: String,
    pub customer_id: i64,
    /// `draft`, `issued`, `paid` or `void`.
    pub status: String,
    pub issued_on: NaiveDate,
    pub due_on: NaiveDate,
    /// In cents (`APP_CURRENCY`, USD).
    pub subtotal: i64,
    pub tax: i64,
    pub total: i64,
    pub notes: String,
    /// The gateway's payment page, once made.
    pub payment_url: Option<String>,
    pub paid_at: Option<DateTime>,
    /// `cash`, `midtrans` or `xendit`.
    pub paid_via: Option<String>,
    /// Staff names, for the grid's audit details.
    pub created_by: String,
    pub updated_by: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "invoice_lines")]
pub struct InvoiceLine {
    pub id: i64,
    pub invoice_id: i64,
    pub product_id: i64,
    /// The product's name when the invoice was written.
    pub description: String,
    pub quantity: i64,
    pub unit_price: i64,
    pub amount: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

pub const STATUSES: [(&str, &str); 4] = [
    ("draft", "Draft"),
    ("issued", "Issued"),
    ("paid", "Paid"),
    ("void", "Void"),
];

pub const STATUS_TONES: [(&str, &str); 4] = [
    ("draft", "neutral"),
    ("issued", "info"),
    ("paid", "success"),
    ("void", "danger"),
];

impl Invoice {
    /// Issued and past its due date.
    pub fn overdue(&self, today: NaiveDate) -> bool {
        self.status == "issued" && self.due_on < today
    }

    /// The tax on `subtotal` (cents) at `percent`, rounded to the cent.
    pub fn tax_on(subtotal: i64, percent: i64) -> i64 {
        (subtotal * percent + 50) / 100
    }
}

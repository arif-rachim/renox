//! `db:seed`: the roles, three staff (admin, cashier, warehouse, all with
//! `password123`), customers, products with their opening stock, and three
//! months of invoices, so every page has something to show. Seeding twice
//! is harmless: a seeded database stays as it is.

use renox::chrono::{Days, Duration};
use renox::db::Transaction;
use renox::fake::Fake;
use renox::fake::faker::name::en::Name;
use renox::prelude::*;

use super::customers::Customer;
use super::invoices::{Invoice, InvoiceLine};
use super::products::{Product, stock};
use crate::Settings;

const CITIES: [&str; 6] = ["Bandung", "Jakarta", "Bogor", "Cimahi", "Garut", "Sumedang"];

/// SKU, name, price in cents.
const PRODUCTS: [(&str, &str, i64); 16] = [
    ("COF-ARB", "Arabica coffee 250 g", 1_299),
    ("COF-ROB", "Robusta coffee 250 g", 899),
    ("TEA-JAS", "Jasmine tea 100 g", 499),
    ("SUGAR-PALM", "Palm sugar 500 g", 599),
    ("MILK-UHT", "UHT milk 1 L", 299),
    ("CUP-12", "Paper cups 12 oz (50)", 799),
    ("CUP-16", "Paper cups 16 oz (50)", 899),
    ("LID-90", "Cup lids 90 mm (50)", 399),
    ("SPOON-WD", "Wooden spoons (100)", 499),
    ("SYRUP-VAN", "Vanilla syrup 750 ml", 1_499),
    ("SYRUP-CRM", "Caramel syrup 750 ml", 1_499),
    ("COCOA-PWD", "Cocoa powder 500 g", 1_199),
    ("FILTER-V60", "V60 filter papers (100)", 699),
    ("BAG-PPR", "Paper bags (25)", 399),
    ("ICE-5KG", "Ice cubes 5 kg", 349),
    ("WATER-19L", "Mineral water 19 L", 599),
];

pub async fn run(state: AppState) -> Result {
    let db = state.db;
    if User::find_by_email(&db, "admin@example.com")
        .await?
        .is_some()
    {
        return Ok(());
    }
    crate::define_roles(&db).await?;
    Settings::default().save(&db).await?;
    for (name, email, role) in [
        ("Alex", "admin@example.com", "admin"),
        ("Diana", "cashier@example.com", "cashier"),
        ("Ben", "warehouse@example.com", "warehouse"),
    ] {
        let user = User::register(&db, name, email, "password123").await?;
        // Seeded staff need no verification mail.
        renox::db::sql("UPDATE users SET email_verified_at = ? WHERE id = ?")
            .bind(renox::db::now())
            .bind(user.id)
            .execute(&db)
            .await?;
        user.assign_role(&db, role).await?;
    }

    let mut tx = db.begin().await?;
    for _ in 0..40 {
        let name: String = Name().fake();
        let email = format!(
            "{}@example.com",
            name.to_lowercase().replace([' ', '.', '\''], "")
        );
        Customer::create(
            &mut tx,
            Customer {
                name,
                email: Some(email),
                phone: Some(format!(
                    "08{}",
                    (1_000_000_000_u64..9_999_999_999).fake::<u64>()
                )),
                city: Some(CITIES[(0..CITIES.len()).fake::<usize>()].into()),
                ..Default::default()
            },
        )
        .await?;
    }
    let mut products = Vec::new();
    for (i, (sku, name, price)) in PRODUCTS.into_iter().enumerate() {
        // A few run low, for the dashboard.
        let (min_stock, opening) = if i % 5 == 2 {
            (40, (30..50).fake())
        } else {
            (10, (80..200).fake())
        };
        let product = Product::create(
            &mut tx,
            Product {
                sku: sku.into(),
                name: name.into(),
                price,
                min_stock,
                active: true,
                ..Default::default()
            },
        )
        .await?;
        received(&mut tx, product.id, opening).await?;
        products.push(product);
    }
    // The opening stock arrived before the first invoice.
    renox::db::sql("UPDATE stock_movements SET created_at = ?")
        .bind(renox::db::now() - Duration::days(100))
        .execute(&mut tx)
        .await?;
    let today = renox::db::now().date_naive();
    let settings = Settings::default();
    for n in 1..=150_i64 {
        let days_ago: u64 = (0..90).fake();
        let issued_on = today - Days::new(days_ago);
        let mut lines = Vec::new();
        for _ in 0..(1..5).fake::<usize>() {
            let product = &products[(0..products.len()).fake::<usize>()];
            let quantity: i64 = (1..6).fake();
            lines.push(InvoiceLine {
                product_id: product.id,
                description: product.name.clone(),
                quantity,
                unit_price: product.price,
                amount: product.price * quantity,
                ..Default::default()
            });
        }
        let subtotal: i64 = lines.iter().map(|l| l.amount).sum();
        let tax = Invoice::tax_on(subtotal, settings.tax_percent);
        // Older invoices are mostly paid; a few are void or still drafts.
        let status = match (0..10).fake::<u8>() {
            0 => "void",
            1 if days_ago < 10 => "draft",
            2..=4 if days_ago < 40 => "issued",
            _ if days_ago < 5 => "issued",
            _ => "paid",
        };
        let paid = status == "paid";
        // Written that morning, for the grid's audit details and the ledger.
        let written = renox::db::now() - Duration::days(days_ago as i64);
        let invoice = Invoice::create(
            &mut tx,
            Invoice {
                number: format!("{}{n:05}", settings.invoice_prefix),
                customer_id: (1..=40).fake(),
                status: status.into(),
                issued_on,
                due_on: issued_on + Days::new(settings.payment_days as u64),
                subtotal,
                tax,
                total: subtotal + tax,
                paid_at: paid.then(|| {
                    renox::db::now() - Duration::days(days_ago as i64)
                        + Duration::days((0..days_ago.min(10) as i64 + 1).fake())
                }),
                paid_via: paid
                    .then(|| ["cash", "midtrans", "xendit"][(0..3).fake::<usize>()].into()),
                created_by: "Diana".into(),
                updated_by: "Diana".into(),
                created_at: Some(written),
                ..Default::default()
            },
        )
        .await?;
        for mut line in lines {
            line.invoice_id = invoice.id;
            if matches!(status, "issued" | "paid") {
                // Seeded stock is plenty; a line that doesn't fit is skipped.
                stock::change(
                    &mut tx,
                    stock::Change {
                        product_id: line.product_id,
                        quantity: -line.quantity,
                        reason: "sold",
                        note: "",
                        invoice_id: Some(invoice.id),
                        user_name: "Diana",
                    },
                )
                .await?;
            }
            InvoiceLine::create(&mut tx, line).await?;
        }
        renox::db::sql("UPDATE stock_movements SET created_at = ? WHERE invoice_id = ?")
            .bind(written)
            .bind(invoice.id)
            .execute(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn received(tx: &mut Transaction, product_id: i64, quantity: i64) -> Result {
    stock::change(
        tx,
        stock::Change {
            product_id,
            quantity,
            reason: "received",
            note: "Opening stock",
            invoice_id: None,
            user_name: "Ben",
        },
    )
    .await?;
    Ok(())
}

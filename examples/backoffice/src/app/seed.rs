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

const PRODUCTS: [(&str, &str, i64); 16] = [
    ("KOPI-ARB", "Kopi arabika 250 g", 65_000),
    ("KOPI-ROB", "Kopi robusta 250 g", 45_000),
    ("TEH-MLT", "Teh melati 100 g", 18_000),
    ("GULA-AREN", "Gula aren 500 g", 32_000),
    ("SUSU-UHT", "Susu UHT 1 L", 21_000),
    ("CUP-12", "Gelas kertas 12 oz (50)", 38_000),
    ("CUP-16", "Gelas kertas 16 oz (50)", 44_000),
    ("LID-90", "Tutup gelas 90 mm (50)", 19_000),
    ("SDT-KYU", "Sendok kayu (100)", 25_000),
    ("SIRUP-VAN", "Sirup vanila 750 ml", 89_000),
    ("SIRUP-KRM", "Sirup karamel 750 ml", 89_000),
    ("COKLAT-BBK", "Cokelat bubuk 500 g", 74_000),
    ("FILTER-V60", "Kertas filter V60 (100)", 55_000),
    ("TAS-KRT", "Tas kertas (25)", 27_000),
    ("ES-BATU", "Es batu kristal 5 kg", 15_000),
    ("AIR-GLN", "Air mineral galon", 22_000),
];

pub async fn run(db: Db) -> Result {
    if User::find_by_email(&db, "admin@example.com")
        .await?
        .is_some()
    {
        return Ok(());
    }
    crate::define_roles(&db).await?;
    Settings::default().save(&db).await?;
    for (name, email, role) in [
        ("Arif", "admin@example.com", "admin"),
        ("Dewi", "cashier@example.com", "cashier"),
        ("Budi", "warehouse@example.com", "warehouse"),
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
                created_by: "Dewi".into(),
                updated_by: "Dewi".into(),
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
                        user_name: "Dewi",
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
            user_name: "Budi",
        },
    )
    .await?;
    Ok(())
}

//! `db:seed`: the roles, an admin and an editor (`password123`),
//! categories, products and customers. Seeding twice is harmless: a seeded
//! database stays as it is.

use renox::chrono::NaiveDate;
use renox::prelude::*;

use crate::resources::{Category, Customer, Product};

const CATEGORIES: [(&str, &str); 4] = [
    ("Coffee", "Beans and ground coffee."),
    ("Tea", "Leaves and bags."),
    ("Supplies", "Cups, lids and the rest of the counter."),
    ("Syrups", "For the sweet ones."),
];

/// Name, SKU, category (index), price, stock, status, featured.
#[rustfmt::skip]
const PRODUCTS: [(&str, &str, usize, i64, i64, &str, bool); 14] = [
    ("Arabica coffee 250 g",  "COF-ARB",   0, 65_000,  42, "live",     true),
    ("Robusta coffee 250 g",  "COF-ROB",   0, 45_000,  18, "live",     false),
    ("Decaf coffee 250 g",    "COF-DEC",   0, 70_000,   3, "draft",    false),
    ("Jasmine tea 100 g",     "TEA-JAS",   1, 18_000,  25, "live",     true),
    ("Green tea 100 g",       "TEA-GRN",   1, 22_000,   0, "live",     false),
    ("Earl grey 100 g",       "TEA-EGR",   1, 27_000,   9, "archived", false),
    ("Paper cups 12 oz (50)", "CUP-12",    2, 38_000, 120, "live",     false),
    ("Paper cups 16 oz (50)", "CUP-16",    2, 44_000,   4, "live",     false),
    ("Cup lids 90 mm (50)",   "LID-90",    2, 19_000,  60, "live",     false),
    ("Wooden spoons (100)",   "SPOON-WD",  2, 25_000,   2, "draft",    false),
    ("Vanilla syrup 750 ml",  "SYRUP-VAN", 3, 89_000,  11, "live",     true),
    ("Caramel syrup 750 ml",  "SYRUP-CRM", 3, 89_000,   7, "live",     false),
    ("Hazelnut syrup 750 ml", "SYRUP-HAZ", 3, 92_000,   0, "draft",    false),
    ("Cocoa powder 500 g",    "COCOA-PWD", 3, 74_000,  15, "live",     false),
];

/// Name, city, tier.
const CUSTOMERS: [(&str, &str, &str); 8] = [
    ("Ayu Lestari", "Bandung", "gold"),
    ("Budi Santoso", "Jakarta", "regular"),
    ("Citra Dewi", "Bogor", "silver"),
    ("Dimas Pratama", "Bandung", "regular"),
    ("Eka Putri", "Garut", "silver"),
    ("Fajar Nugroho", "Jakarta", "gold"),
    ("Gita Rahma", "Cimahi", "regular"),
    ("Hadi Wijaya", "Sumedang", "regular"),
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
    for (name, email, role) in [
        ("Alex", "admin@example.com", "admin"),
        ("Eva", "editor@example.com", "editor"),
    ] {
        let user = User::register(&db, name, email, "password123").await?;
        user.assign_role(&db, role).await?;
    }
    let mut categories = Vec::new();
    for (name, description) in CATEGORIES {
        let category = Category::create(
            &db,
            Category {
                name: name.into(),
                description: Some(description.into()),
                ..Default::default()
            },
        )
        .await?;
        categories.push(category.id);
    }
    for (i, (name, sku, category, price, stock, status, featured)) in
        PRODUCTS.into_iter().enumerate()
    {
        Product::create(
            &db,
            Product {
                name: name.into(),
                sku: sku.into(),
                category_id: Some(categories[category]),
                price,
                stock,
                status: status.into(),
                featured,
                released_on: NaiveDate::from_ymd_opt(2026, 1 + (i as u32 % 9), 1 + i as u32),
                description: Some(format!("**{name}**, from the shop's own suppliers.")),
                ..Default::default()
            },
        )
        .await?;
    }
    for (name, city, tier) in CUSTOMERS {
        let first = name.split(' ').next().unwrap_or(name).to_lowercase();
        Customer::create(
            &db,
            Customer {
                name: name.into(),
                email: format!("{first}@example.com"),
                city: Some(city.into()),
                tier: tier.into(),
                newsletter: tier != "regular",
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

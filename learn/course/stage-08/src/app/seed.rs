use renox::prelude::*;

use super::menu::category::Category;
use super::menu::model::MenuItem;

/// The demo data: two people, three categories and the menu.
/// It does nothing when Bea is there already, so it can run twice.
pub async fn run(state: AppState) -> Result {
    let db = &state.db;
    let bea_is_there = User::query()
        .where_eq("email", "bea@example.com")
        .first(db)
        .await?
        .is_some();
    if bea_is_there {
        return Ok(());
    }

    let bea = User::register(db, "Bea", "bea@example.com", "password123").await?;
    User::register(db, "Sam", "sam@example.com", "password123").await?;

    // (category, name, description, price in cents, available, stock)
    let items = [
        (
            "Coffee",
            "Espresso",
            "A double shot, short and strong.",
            300,
            true,
            None,
        ),
        (
            "Coffee",
            "Flat white",
            "Espresso with steamed whole milk.",
            450,
            true,
            None,
        ),
        (
            "Coffee",
            "Cappuccino",
            "Espresso, milk and a thick layer of foam.",
            450,
            true,
            None,
        ),
        (
            "Coffee",
            "Cold brew",
            "Steeped for 18 hours. Back in summer.",
            500,
            false,
            None,
        ),
        (
            "Tea",
            "Earl Grey",
            "Black tea with bergamot.",
            350,
            true,
            None,
        ),
        (
            "Tea",
            "Matcha latte",
            "Stone-ground matcha with oat milk.",
            525,
            true,
            None,
        ),
        (
            "Pastry",
            "Croissant",
            "Baked this morning, all butter.",
            375,
            true,
            Some(12),
        ),
        (
            "Pastry",
            "Cinnamon bun",
            "Cardamom dough, cinnamon sugar.",
            425,
            true,
            Some(3),
        ),
        (
            "Pastry",
            "Blueberry muffin",
            "Wild blueberries, lemon zest.",
            350,
            true,
            Some(0),
        ),
    ];
    let mut categories = std::collections::HashMap::new();
    for (category, name, description, price, available, stock) in items {
        if !categories.contains_key(category) {
            let created = Category::create(
                db,
                Category {
                    name: category.into(),
                    ..Default::default()
                },
            )
            .await?;
            categories.insert(category, created.id);
        }
        MenuItem::create(
            db,
            MenuItem {
                category_id: categories[category],
                user_id: bea.id,
                name: name.into(),
                description: Some(description.into()),
                price,
                available,
                stock,
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

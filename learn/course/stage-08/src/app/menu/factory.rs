use std::sync::atomic::{AtomicUsize, Ordering};

use renox::prelude::*;

use super::category::Category;
use super::model::MenuItem;

// [learn:menu.factory]
/// Test records: `Category::factory().create_one(&db)` saves a "Coffee"
/// category, `MenuItem::factory().count(3).create(&db)` three items.
impl Factory for Category {
    fn definition() -> Self {
        Category {
            name: "Coffee".into(),
            ..Default::default()
        }
    }
}

/// Names count up ("Item 1", "Item 2"…) because item names are unique. An item
/// belongs to a category and a user, and the database enforces both: a test
/// sets `category_id` and `user_id` with `.state(…)`.
impl Factory for MenuItem {
    fn definition() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(1);
        MenuItem {
            name: format!("Item {}", NEXT.fetch_add(1, Ordering::Relaxed)),
            price: 400,
            available: true,
            stock: None,
            ..Default::default()
        }
    }
}
// [/learn:menu.factory]

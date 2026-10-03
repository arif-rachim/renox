use renox::fake::Fake;
use renox::fake::faker::lorem::en::{Sentence, Words};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "categories")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products")]
pub struct Product {
    pub id: i64,
    pub category_id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub description: String,
    /// In rupiah.
    pub price: i64,
    pub stock: i64,
    /// Storage key of the photo, e.g. `public/products/abc.jpg`.
    pub photo: Option<String>,
    /// Hidden from the shop when false.
    pub active: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Factory for Product {
    fn definition() -> Self {
        let words: Vec<String> = Words(2..4).fake();
        let name = words.join(" ");
        Product {
            slug: slug(&name),
            name,
            description: Sentence(8..16).fake(),
            price: (10..500).fake::<i64>() * 1_000,
            stock: (0..40).fake(),
            active: true,
            ..Default::default()
        }
    }
}

impl Product {
    /// The category, for one product; for a page of them use
    /// `relations::belongs_to` (see `catalog::index`).
    pub async fn category(&self, db: &Db) -> Result<Option<Category>> {
        match self.category_id {
            Some(id) => Category::find(db, id).await,
            None => Ok(None),
        }
    }
}

/// `Coffee Latte Brown Sugar!` -> `coffee-latte-brown-sugar`.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn slugs() {
        assert_eq!(
            super::slug("Coffee Latte  Brown Sugar!"),
            "coffee-latte-brown-sugar"
        );
        assert_eq!(super::slug("  Green -- Tea "), "green-tea");
    }
}

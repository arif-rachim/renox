use renox::db::ModelHooks;
use renox::fake::Fake;
use renox::fake::faker::lorem::en::Word;
use renox::prelude::*;
use renox::validation::Errors;
use serde::{Deserialize, Serialize};

/// The cache key of the product count on the list page. The hooks below
/// forget it whenever a product is saved or deleted.
pub const COUNT_KEY: &str = "products.count";

// `hooks` makes the derive call `impl ModelHooks for Product` around
// `save`, `create`/`insert`, `save_only`, `save_changes`, `delete` and
// `force_delete`. Bulk writes (`Product::where_eq(..).update(..)`,
// `Query::delete`, `insert_many`) and `restore` don't run them.
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products", soft_deletes, hooks)]
pub struct Product {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    /// Made from `name` by the `saving` hook, e.g. "Coffee Latte" → "coffee-latte".
    pub slug: String,
    /// In the smallest currency unit (e.g. rupiah), to avoid float rounding.
    pub price: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub deleted_at: Option<DateTime>,
}

impl ModelHooks for Product {
    /// Before every INSERT/UPDATE: derive the slug, and refuse a name that
    /// has no letters or digits to make one from. The error is a
    /// `ValidationError` on `name`, so a form shows it next to the field
    /// (422 for htmx, a redirect back for plain forms).
    fn saving(&mut self, _creating: bool) -> Result {
        self.slug = slugify(&self.name);
        if self.slug.is_empty() {
            let mut errors = Errors::new();
            errors.add("name", "The name needs at least one letter or digit.");
            return Err(ValidationError::new(errors).into());
        }
        Ok(())
    }

    /// After the write: the cached count may be stale now. `context::app()`
    /// is the request's app (or a job's, task's, command's); there's none in
    /// a test that calls `Product::create` directly, so nothing to forget.
    async fn saved(&self, created: bool) -> Result {
        if created {
            forget_count().await?;
        }
        Ok(())
    }

    async fn deleted(&self) -> Result {
        forget_count().await
    }
}

/// Forgets the cached product count. `restore` doesn't run hooks, so the
/// restore handler calls this itself.
pub async fn forget_count() -> Result {
    if let Some(state) = renox::context::app() {
        state.cache.forget(COUNT_KEY).await?;
    }
    Ok(())
}

/// Lowercase letters and digits, words joined by `-`.
pub fn slugify(name: &str) -> String {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

impl Factory for Product {
    fn definition() -> Self {
        Product {
            name: Word().fake(),
            price: (1_000..100_000).fake(),
            ..Default::default()
        }
    }
}

impl Product {
    /// A fake product owned by `user`, for seeders and tests.
    pub fn for_owner(user: &User) -> Self {
        Product {
            user_id: user.id,
            ..Product::factory().make_one()
        }
    }
}

//! Made with `rnx make:module products`,
//! `rnx make:model Product --module products --migration`,
//! `rnx make:policy Product --module products` and
//! `rnx make:command products:import --module products`, then filled in.

pub mod import;
pub mod model;
pub mod policy;

use renox::Toast;
use renox::prelude::*;
use renox::validation::ValidateHooks;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use model::{COUNT_KEY, Product};

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        let public = Routes::new()
            .get("/", home)
            .name("home")
            .get("/products", index)
            .name("products.index");
        let members = Routes::new()
            .get("/products/new", create)
            .name("products.create")
            .post("/products", store)
            .name("products.store")
            .get("/products/trash", trash)
            .name("products.trash")
            .get("/products/{id}/edit", edit)
            .name("products.edit")
            // Forms send these as POST with `{{ method_field('PUT') }}` etc.
            .put("/products/{id}", update)
            .name("products.update")
            .delete("/products/{id}", destroy)
            .name("products.destroy")
            .post("/products/{id}/restore", restore)
            .name("products.restore")
            .require_auth();
        public.merge(members)
    }

    fn register(&self, app: &mut Registry) {
        // `cargo run -- products:import products.csv --owner demo@example.com`
        app.typed_command::<import::ImportProducts>();
    }
}

/// What the create and edit forms send. The rules are attributes
/// (`#[derive(Validate)]`); `#[validate(hooks)]` adds `prepare` below.
#[derive(Deserialize, Serialize, Validate)]
#[validate(hooks)]
struct ProductForm {
    #[validate(required, max = 100)]
    name: String,
    #[validate(min = 0)]
    price: i64,
}

impl ValidateHooks for ProductForm {
    /// Before the rules: "  Kopi   Susu " is saved (and checked) as "Kopi Susu".
    fn prepare(&mut self) {
        self.name = self.name.split_whitespace().collect::<Vec<_>>().join(" ");
    }
}

async fn home() -> Redirect {
    Redirect::to("/products")
}

async fn index(
    State(state): State<AppState>,
    user: Option<AuthUser>,
    Page(page): Page,
) -> Result<View> {
    let db = &state.db;
    // Cached for ten minutes; the model's `saved`/`deleted` hooks forget it,
    // so it's never stale after a change made through the model.
    let total: i64 = state
        .cache
        .remember(COUNT_KEY, Duration::from_secs(600), || async {
            Ok(Product::query().count(db).await? as i64)
        })
        .await?;
    let products = Product::query()
        .latest()
        .paginate(db, page, 10)
        .await?
        // Lets the view ask the policy: `can('update', product)`.
        .map(|product| Can::new(product, user.as_deref(), &["update", "delete"]));
    Ok(view("products/index.html", context! { products, total }))
}

async fn create() -> View {
    view("products/form.html", context! {})
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, Redirect)> {
    let product = Product {
        user_id: user.id,
        name: form.name,
        price: form.price,
        ..Default::default()
    };
    Product::create(&db, product).await?;
    // Shown on the next page (or at once for an htmx request).
    Ok((
        Toast::success("Product created."),
        Redirect::to("/products"),
    ))
}

async fn edit(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    user.authorize("update", &product)?;
    Ok(view("products/form.html", context! { product }))
}

async fn update(
    State(db): State<Db>,
    user: AuthUser,
    Path(id): Path<i64>,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, Redirect)> {
    let original = Product::find_or_404(&db, id).await?;
    user.authorize("update", &original)?;
    let mut product = original.clone();
    product.name = form.name;
    product.price = form.price;
    // Writes only the columns that differ from `original` (plus
    // `updated_at`), including the slug the `saving` hook derives, so a
    // concurrent change to another column isn't overwritten. `false` means
    // nothing changed: no query, no `saved` hook.
    let changed = product.save_changes(&db, &original).await?;
    let toast = if changed {
        Toast::success("Product updated.")
    } else {
        Toast::info("Nothing changed.")
    };
    Ok((toast, Redirect::to("/products")))
}

async fn destroy(
    State(db): State<Db>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::find_or_404(&db, id).await?;
    user.authorize("delete", &product)?;
    product.delete(&db).await?; // soft: sets deleted_at
    Ok((
        Toast::success(format!("“{}” moved to the trash.", product.name)),
        Redirect::to("/products"),
    ))
}

async fn trash(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let products = Product::query()
        .only_trashed()
        .where_eq("user_id", user.id)
        .latest()
        .get(&db)
        .await?;
    Ok(view("products/trash.html", context! { products }))
}

async fn restore(
    State(db): State<Db>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::query()
        .only_trashed()
        .where_eq("id", id)
        .first(&db)
        .await?
        .ok_or(Error::NotFound)?;
    user.authorize("restore", &product)?;
    product.restore(&db).await?;
    model::forget_count().await?; // `restore` runs no hooks
    Ok((
        Toast::success("Product restored."),
        Redirect::to("/products"),
    ))
}

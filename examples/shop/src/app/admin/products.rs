use renox::Toast;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::catalog::FEATURED;
use crate::app::catalog::model::{Product, slug};

/// `/admin/products?q=…&sort=stock`
#[derive(Deserialize, Serialize, Default)]
pub struct Filters {
    q: Option<String>,
    sort: Option<String>,
}

pub async fn index(
    State(db): State<Db>,
    Query(filters): Query<Filters>,
    Page(page): Page,
) -> Result<View> {
    let q = filters.q.as_deref().unwrap_or("").trim();
    let query = Product::query().when(!q.is_empty(), |query| {
        query.where_like("name", format!("%{q}%"))
    });
    let query = match filters.sort.as_deref() {
        Some("name") => query.order_by("name"),
        Some("price") => query.order_by("price"),
        Some("stock") => query.order_by("stock"),
        _ => query.latest(),
    };
    let products = query.paginate(&db, page, 20).await?;
    Ok(view(
        "admin/products/index.html",
        context! { products, filters },
    ))
}

#[derive(Deserialize)]
pub struct ProductForm {
    name: String,
    category_id: Option<i64>,
    #[serde(default)]
    description: String,
    price: i64,
    stock: i64,
    active: bool,
    photo: Option<Upload>,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100);
        v.field("category_id", &self.category_id)
            .exists("categories", "id");
        v.field("description", &self.description).max(2000);
        v.field("price", &self.price).min(0);
        v.field("stock", &self.stock).min(0);
        v.field("photo", &self.photo).image().max(2048);
    }
}

/// The categories aren't loaded: the form's select asks for them as the
/// admin types (`categories::options`).
pub async fn create() -> Result<View> {
    Ok(view("admin/products/form.html", context! {}))
}

pub async fn store(
    State(state): State<AppState>,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::default();
    fill(&state, &mut product, form).await?;
    let product = Product::create(&state.db, product).await?;
    state.cache.forget(FEATURED).await?;
    Ok((
        Toast::success(format!("“{}” created.", product.name)),
        Redirect::route("admin.products.index", &[])?,
    ))
}

pub async fn edit(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    // Only the current category's name: the select fetches the rest.
    let category_name = product.category(&db).await?.map(|c| c.name);
    Ok(view(
        "admin/products/form.html",
        context! { product, category_name },
    ))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::find_or_404(&state.db, id).await?;
    fill(&state, &mut product, form).await?;
    product.save(&state.db).await?;
    state.cache.forget(FEATURED).await?;
    Ok((
        Toast::success(format!("“{}” saved.", product.name)),
        Redirect::route("admin.products.index", &[])?,
    ))
}

pub async fn destroy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::find_or_404(&state.db, id).await?;
    product.delete(&state.db).await?; // orders keep the name and price
    if let Some(photo) = &product.photo {
        state.storage.delete(photo).await?;
    }
    state.cache.forget(FEATURED).await?;
    Ok((
        Toast::success(format!("“{}” deleted.", product.name)),
        Redirect::route("admin.products.index", &[])?,
    ))
}

/// Copies the form into the product; a new photo replaces the old one.
async fn fill(state: &AppState, product: &mut Product, form: ProductForm) -> Result {
    if product.name != form.name || product.slug.is_empty() {
        product.slug = free_slug(&state.db, &form.name, product.id).await?;
    }
    product.name = form.name;
    product.category_id = form.category_id;
    product.description = form.description;
    product.price = form.price;
    product.stock = form.stock;
    product.active = form.active;
    if let Some(photo) = form.photo {
        let key = photo.store_public(&state.storage, "products").await?;
        if let Some(old) = product.photo.replace(key) {
            state.storage.delete(&old).await?;
        }
    }
    Ok(())
}

/// The name's slug, with `-2`, `-3`… when another product has it.
async fn free_slug(db: &Db, name: &str, except_id: i64) -> Result<String> {
    let base = match slug(name) {
        s if s.is_empty() => "product".to_owned(),
        s => s,
    };
    let mut candidate = base.clone();
    for n in 2.. {
        let taken = Product::where_eq("slug", &candidate)
            .where_op("id", "!=", except_id)
            .count(db)
            .await?;
        if taken == 0 {
            break;
        }
        candidate = format!("{base}-{n}");
    }
    Ok(candidate)
}

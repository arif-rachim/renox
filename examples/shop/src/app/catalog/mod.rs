//! The shop front: the home page, the product list with search, filters,
//! sorting and pagination, a product page with SEO tags, and the sitemap.
//!
//! Made with `rnx make:module catalog`, `rnx make:model Category --module
//! catalog -m`, `rnx make:model Product --module catalog -m` and
//! `rnx make:factory Product --module catalog`, then filled in.

pub mod model;

use std::time::Duration;

use renox::db::relations::belongs_to;
use renox::prelude::*;
use renox::seo::Sitemap;
use serde::{Deserialize, Serialize};

use model::{Category, Product};

/// Cache key of the home page's products; the admin forgets it on changes.
pub const FEATURED: &str = "catalog.featured";

pub struct Catalog;

impl Module for Catalog {
    fn name(&self) -> &'static str {
        "catalog"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", home)
            .name("home")
            .get("/products", index)
            .name("products.index")
            .get("/products/{slug}", show)
            .name("products.show")
            .get("/sitemap.xml", sitemap)
            .name("sitemap")
            .get("/language/{locale}", language)
            .name("language")
    }
}

/// The home page: the newest products (cached) and the categories.
/// The session key of the products a visitor opened last (their ids).
const RECENT: &str = "recently_viewed";

async fn home(State(state): State<AppState>, session: Session) -> Result<View> {
    let db = &state.db;
    let featured: Vec<Product> = state
        .cache
        .remember(FEATURED, Duration::from_secs(600), || async {
            Product::where_eq("active", true)
                .where_op("stock", ">", 0)
                .latest()
                .limit(8)
                .get(db)
                .await
        })
        .await?;
    let categories = Category::query().order_by("name").get(db).await?;
    // Newest first, each once; the template shows the first four.
    let mut ids: Vec<i64> = session.get(RECENT).unwrap_or_default();
    ids.reverse();
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    let found = Product::find_many(db, ids.iter().copied()).await?;
    let recent: Vec<&Product> = ids
        .iter()
        .filter_map(|id| found.iter().find(|p| p.id == *id && p.active))
        .collect();
    Ok(view(
        "catalog/home.html",
        context! { featured, categories, recent },
    ))
}

/// `/products?q=kopi&category=minuman&sort=price_asc&page=2`
#[derive(Deserialize, Serialize, Default)]
struct Filters {
    q: Option<String>,
    category: Option<String>,
    sort: Option<String>,
}

/// A product with its category, for the list.
#[derive(Serialize)]
struct Card {
    #[serde(flatten)]
    product: Product,
    category: Option<Category>,
}

async fn index(
    State(db): State<Db>,
    Query(filters): Query<Filters>,
    Page(page): Page,
) -> Result<View> {
    let q = filters.q.as_deref().unwrap_or("").trim();
    let category = match filters.category.as_deref().filter(|s| !s.is_empty()) {
        Some(slug) => Some(Category::where_eq("slug", slug).first_or_404(&db).await?),
        None => None,
    };
    let query = Product::where_eq("active", true)
        .when(!q.is_empty(), |query| {
            let like = format!("%{q}%");
            query.where_any(|any| {
                any.where_like("name", like.clone())
                    .where_like("description", like)
            })
        })
        .when(category.is_some(), |query| {
            query.where_eq("category_id", category.as_ref().map(|c| c.id))
        });
    // Only known orders: the column name never comes from the visitor.
    let query = match filters.sort.as_deref() {
        Some("price_asc") => query.order_by("price"),
        Some("price_desc") => query.order_by_desc("price"),
        Some("name") => query.order_by("name"),
        _ => query.latest(),
    };
    let products = query.paginate(&db, page, 12).await?;
    // One query for the categories of the whole page, not one per product.
    let categories = belongs_to::<Category, _, _>(&db, &products.items, |p| p.category_id).await?;
    let products = products.map(|product| Card {
        category: product
            .category_id
            .and_then(|id| categories.get(&id).cloned()),
        product,
    });
    // The category filter's options: [slug, name] pairs for the kit's select.
    let category_options: Vec<(String, String)> = Category::query()
        .order_by("name")
        .get(&db)
        .await?
        .into_iter()
        .map(|c| (c.slug, c.name))
        .collect();
    Ok(view(
        "catalog/index.html",
        context! { products, filters, category_options },
    )
    // The search box asks with htmx and gets only the results back.
    .fragment("results"))
}

async fn show(State(db): State<Db>, session: Session, Path(slug): Path<String>) -> Result<View> {
    let product = Product::where_eq("slug", &slug)
        .where_eq("active", true)
        .first_or_404(&db)
        .await?;
    // Remembered for "Recently viewed" on the home page; the last eight
    // only, so the session cookie stays small.
    if session.push(RECENT, product.id)? > 8 {
        let mut ids: Vec<i64> = session.get(RECENT).unwrap_or_default();
        ids.drain(..ids.len() - 8);
        session.put(RECENT, ids)?;
    }
    let category = product.category(&db).await?;
    Ok(view("catalog/show.html", context! { product, category }))
}

async fn sitemap(State(state): State<AppState>) -> Result<Sitemap> {
    let mut map =
        Sitemap::new(&state)
            .route("home", &[], None)?
            .route("products.index", &[], None)?;
    let products = Product::where_eq("active", true)
        .order_by("id")
        .get(&state.db)
        .await?;
    for product in products {
        map = map.route("products.show", &[&product.slug], product.updated_at)?;
    }
    Ok(map)
}

/// Remembers the visitor's language and goes back to where they were. A
/// logged-in user's is saved too: their mail and notifications are written
/// in it (`users.locale`, which `Recipient::locale` reads).
async fn language(
    State(state): State<AppState>,
    session: Session,
    user: Option<AuthUser>,
    back: Back,
    Path(locale): Path<String>,
) -> Result<Back> {
    if ["en", "id"].contains(&locale.as_str()) {
        renox::i18n::set_locale(&session, &locale)?;
        if let Some(user) = user {
            renox::db::sql("UPDATE users SET locale = ? WHERE id = ?")
                .bind(&locale)
                .bind(user.id)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(back)
}

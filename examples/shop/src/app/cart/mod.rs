//! The cart, kept in the database per user so it survives logins on other
//! devices. Every route needs a login.
//!
//! Made with `rnx make:module cart` and `rnx make:model CartItem --module cart -m`,
//! then filled in.

use renox::Toast;
use renox::db::relations::belongs_to;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::catalog::model::Product;

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "cart_items")]
pub struct CartItem {
    pub id: i64,
    pub user_id: i64,
    pub product_id: i64,
    pub quantity: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// A cart item with its product, as the cart page and checkout see it.
#[derive(Serialize, Clone)]
pub struct Line {
    pub item: CartItem,
    pub product: Product,
    pub subtotal: i64,
}

/// The user's cart, oldest item first. Items whose product is gone or hidden
/// are left out.
pub async fn lines(db: &Db, user_id: i64) -> Result<Vec<Line>> {
    let items = CartItem::where_eq("user_id", user_id)
        .order_by("id")
        .get(db)
        .await?;
    let products = belongs_to::<Product, _, _>(db, &items, |i| i.product_id).await?;
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let product = products.get(&item.product_id).filter(|p| p.active)?.clone();
            Some(Line {
                subtotal: product.price * item.quantity,
                item,
                product,
            })
        })
        .collect())
}

pub struct Cart;

impl Module for Cart {
    fn name(&self) -> &'static str {
        "cart"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/cart", show)
            .name("cart.show")
            .post("/cart", add)
            .name("cart.add")
            .patch("/cart/{id}", update)
            .name("cart.update")
            .delete("/cart/{id}", remove)
            .name("cart.remove")
            .require_auth()
    }
}

async fn show(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let lines = lines(&db, user.id).await?;
    let total: i64 = lines.iter().map(|l| l.subtotal).sum();
    Ok(view("cart/show.html", context! { lines, total }))
}

#[derive(Deserialize)]
struct AddForm {
    product_id: i64,
    quantity: i64,
}

impl Validate for AddForm {
    fn rules(&self, v: &mut Validator) {
        v.field("product_id", &self.product_id)
            .exists("products", "id");
        v.field("quantity", &self.quantity).between(1, 99);
    }
}

async fn add(
    State(db): State<Db>,
    user: AuthUser,
    lang: Lang,
    back: Back,
    Valid(form): Valid<AddForm>,
) -> Result<(Toast, Back)> {
    let product = Product::find_or_404(&db, form.product_id).await?;
    abort_unless(
        product.active,
        StatusCode::NOT_FOUND,
        "This product isn't sold any more.",
    )?;
    // One row per product: adding it again adds to the quantity.
    renox::db::sql(
        "INSERT INTO cart_items (user_id, product_id, quantity, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (user_id, product_id) \
         DO UPDATE SET quantity = cart_items.quantity + excluded.quantity, updated_at = excluded.updated_at",
    )
    .bind(user.id)
    .bind(product.id)
    .bind(form.quantity)
    .bind(renox::db::now())
    .bind(renox::db::now())
    .execute(&db)
    .await?;
    Ok((
        Toast::success(lang.t("cart.added", &[("name", &product.name)])),
        back,
    ))
}

#[derive(Deserialize)]
struct QuantityForm {
    quantity: i64,
}

impl Validate for QuantityForm {
    fn rules(&self, v: &mut Validator) {
        v.field("quantity", &self.quantity).between(1, 99);
    }
}

async fn update(
    State(db): State<Db>,
    user: AuthUser,
    Path(id): Path<i64>,
    Valid(form): Valid<QuantityForm>,
) -> Result<Redirect> {
    // Someone else's item is simply not found.
    let mut item = CartItem::where_eq("id", id)
        .where_eq("user_id", user.id)
        .first_or_404(&db)
        .await?;
    item.quantity = form.quantity;
    item.save(&db).await?;
    Redirect::route("cart.show", &[])
}

async fn remove(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<Redirect> {
    CartItem::where_eq("id", id)
        .where_eq("user_id", user.id)
        .delete(&db)
        .await?;
    Redirect::route("cart.show", &[])
}

//! Made with `rnx make:module products` and `rnx make:model Product --module products -m`.
//! Each field shows one pairing of HTML input, Rust type and column type.

use renox::chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use renox::db::Json;
use renox::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A `<select>`, stored as text (`small`, `medium`, `large`).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
pub enum Size {
    Small,
    #[default]
    Medium,
    Large,
}

pub const COLORS: &[&str] = &["black", "white", "red", "green"];

#[derive(Model, Serialize, Default, Debug, Clone, PartialEq)]
#[model(table = "products")]
pub struct Product {
    pub id: i64,
    /// Safe to show in URLs, unlike the sequential id (renox's `uuid` feature).
    pub public_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub stock: i64,
    pub weight_kg: f64,
    /// Money in the smallest unit (rupiah), never a float.
    pub price: i64,
    pub available: bool,
    pub size: Size,
    pub colors: Json<Vec<String>>,
    pub opens_at: Option<NaiveTime>,
    pub launch_at: Option<NaiveDateTime>,
    pub released_on: Option<NaiveDate>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Deserialize, Serialize)]
struct ProductForm {
    name: String,                // <input>
    description: Option<String>, // <textarea>, empty → None
    stock: i64,                  // <input type="number">
    weight_kg: f64,              // <input type="number" step="0.01">
    price: i64,                  // <input type="number" step="1">
    available: bool,             // <input type="checkbox">: "on", or nothing → false
    size: Size,                  // <select>
    #[serde(default)]
    colors: Vec<String>, // <select multiple> or checkboxes named "colors"
    opens_at: Option<NaiveTime>, // <input type="time">
    launch_at: Option<NaiveDateTime>, // <input type="datetime-local">
    released_on: Option<NaiveDate>, // <input type="date">
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100);
        v.field("stock", &self.stock).min(0);
        v.field("weight_kg", &self.weight_kg).min(0);
        v.field("price", &self.price).min(0);
        // Rules for each item of a list: errors are keyed `colors.0`,
        // `colors.1`…, and `error('colors')` in the form shows the first.
        v.each("colors", &self.colors, |color| color.one_of(COLORS));
        // A color ticked twice (a crafted request, or two inputs for one
        // value): the repeat gets the error.
        v.distinct("colors", &self.colors);
    }
}

impl ProductForm {
    fn apply(self, product: &mut Product) {
        product.name = self.name;
        product.description = self.description;
        product.stock = self.stock;
        product.weight_kg = self.weight_kg;
        product.price = self.price;
        product.available = self.available;
        product.size = self.size;
        product.colors = Json(self.colors);
        product.opens_at = self.opens_at;
        product.launch_at = self.launch_at;
        product.released_on = self.released_on;
    }
}

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("home")
            .get("/products/new", create)
            .name("products.create")
            .post("/products", store)
            .name("products.store")
            .get("/products/{public_id}/edit", edit)
            .name("products.edit")
            .put("/products/{public_id}", update)
            .name("products.update")
    }
}

fn form_view(product: Option<&Product>) -> View {
    view(
        "products/form.html",
        context! { product, sizes => Size::ALL, colors => COLORS },
    )
}

async fn find(db: &Db, public_id: Uuid) -> Result<Product> {
    Product::where_eq("public_id", public_id)
        .first(db)
        .await?
        .ok_or(Error::NotFound)
}

async fn index(State(db): State<Db>) -> Result<View> {
    let products = Product::query().order_by("name").get(&db).await?;
    Ok(view("products/index.html", context! { products }))
}

async fn create() -> View {
    form_view(None)
}

async fn store(State(db): State<Db>, Valid(form): Valid<ProductForm>) -> Result<Redirect> {
    let mut product = Product {
        public_id: Uuid::new_v4(),
        ..Default::default()
    };
    form.apply(&mut product);
    let product = Product::create(&db, product).await?;
    Ok(Redirect::to(&format!(
        "/products/{}/edit",
        product.public_id
    )))
}

async fn edit(State(db): State<Db>, Path(public_id): Path<Uuid>) -> Result<View> {
    Ok(form_view(Some(&find(&db, public_id).await?)))
}

async fn update(
    State(db): State<Db>,
    session: Session,
    Path(public_id): Path<Uuid>,
    Valid(form): Valid<ProductForm>,
) -> Result<Redirect> {
    let mut product = find(&db, public_id).await?;
    form.apply(&mut product);
    product.save(&db).await?;
    session.flash("status", "Saved.")?;
    Ok(Redirect::to(&format!("/products/{public_id}/edit")))
}

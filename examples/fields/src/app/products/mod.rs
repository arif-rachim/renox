//! Made with `rnx make:module products`, then
//! `rnx make:model Product --module products -m --key uuid` (a `Uuid` key and
//! its migration), `rnx make:migration add_tags_and_specs_to_products` and
//! `rnx make:migration add_details_and_settings_to_products`.
//! Each field shows one pairing of HTML input, Rust type and column type.

use renox::chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use renox::db::Json;
use renox::prelude::*;
use renox::uuid::Uuid;
use renox_editors::RichText;
use serde::{Deserialize, Serialize};

/// One choice of a few: the kit's `radio` group, stored as text (`small`,
/// `medium`, `large`).
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
    /// The key: a UUID v7 made on insert (renox's `uuid` feature). Safe to
    /// show in URLs, unlike a sequential number.
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub stock: i64,
    pub weight_kg: f64,
    /// Money in the smallest unit (cents: `1699` is $16.99), never a float.
    pub price: i64,
    pub available: bool,
    pub size: Size,
    pub colors: Json<Vec<String>>,
    /// Free tags: a JSON list, like `colors`, typed instead of picked.
    pub tags: Json<Vec<String>>,
    /// Pairs typed by the user ("Origin": "Aceh"): `KeyValues`, stored as a
    /// JSON list of `[key, value]` pairs, so their order holds.
    pub specs: Json<KeyValues>,
    /// Rich text from the rich text editor: HTML, cleaned when the form
    /// was read (`RichText`), shown with the `rich_text` filter.
    pub details: Option<String>,
    /// JSON typed in the code editor, kept as the text it was typed as.
    pub settings: Option<String>,
    pub opens_at: Option<NaiveTime>,
    pub launch_at: Option<NaiveDateTime>,
    pub released_on: Option<NaiveDate>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Deserialize, Serialize)]
struct ProductForm {
    name: String,                // <input>
    description: Option<String>, // the Markdown editor (a <textarea>), empty → None
    stock: i64,                  // <input type="number">
    weight_kg: f64,              // <input type="number" step="0.01">
    price: f64,                  // <input type="number" step="0.01">: dollars, stored as cents
    available: bool,             // <input type="checkbox">: "on", or nothing → false
    size: Size,                  // the kit's radio group: one value of the enum
    #[serde(default)]
    colors: Vec<String>, // <select multiple> or checkboxes named "colors"
    opens_at: Option<NaiveTime>, // <input type="time">
    launch_at: Option<NaiveDateTime>, // <input type="datetime-local">
    released_on: Option<NaiveDate>, // <input type="date">
    #[serde(default)]
    tags: Vec<String>, // the kit's tags_input: one hidden "tags" input per tag
    // The kit's key_value: specs[0][key], specs[0][value]… A nested name
    // makes `Valid` read the whole form as a tree; every type above still
    // parses from its text.
    #[serde(default)]
    specs: KeyValues,
    // The rich text editor (renox-editors): HTML in a hidden input, cleaned
    // as it is read. An emptied editor still sends markup, so `RichText`
    // tells "no text" apart for the rules.
    details: Option<RichText>,
    // The code editor: the text as typed (JSON here).
    settings: Option<String>,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100);
        v.field("stock", &self.stock).min(0);
        v.field("weight_kg", &self.weight_kg).min(0);
        v.field("price", &self.price).min(0).decimal(0, 2);
        // Rules for each item of a list: errors are keyed `colors.0`,
        // `colors.1`…, and `error('colors')` in the form shows the first.
        v.each("colors", &self.colors, |color| color.one_of(COLORS));
        // A color ticked twice (a crafted request, or two inputs for one
        // value): the repeat gets the error.
        v.distinct("colors", &self.colors);
        v.field("tags", &self.tags).max(5);
        v.each("tags", &self.tags, |tag| tag.max(20));
        v.field("specs", &self.specs).max(8);
        // Letters of text, not of markup.
        v.field("details", &self.details).max(5000);
        v.field("settings", &self.settings).json().max(5000);
    }
}

impl ProductForm {
    fn apply(self, product: &mut Product) {
        product.name = self.name;
        product.description = self.description;
        product.stock = self.stock;
        product.weight_kg = self.weight_kg;
        product.price = (self.price * 100.0).round() as i64;
        product.available = self.available;
        product.size = self.size;
        product.colors = Json(self.colors);
        product.tags = Json(self.tags);
        product.specs = Json(self.specs);
        // An emptied editor (`<div><br></div>`) stores nothing.
        product.details = self
            .details
            .filter(|details| !details.is_empty())
            .map(RichText::into_string);
        product.settings = self.settings;
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
            .get("/products/{id}", show)
            .name("products.show")
            .get("/products/{id}/edit", edit)
            .name("products.edit")
            .put("/products/{id}", update)
            .name("products.update")
            .delete("/products/{id}", destroy)
            .name("products.destroy")
    }
}

fn form_view(product: Option<&Product>) -> View {
    view(
        "products/form.html",
        context! {
            product,
            // The price in dollars, as the form takes it.
            price => product.map(|p| format!("{:.2}", p.price as f64 / 100.0)),
            sizes => Size::ALL,
            colors => COLORS,
        },
    )
}

async fn index(State(db): State<Db>) -> Result<View> {
    let products = Product::query().order_by("name").get(&db).await?;
    Ok(view("products/index.html", context! { products }))
}

async fn create() -> View {
    form_view(None)
}

async fn store(State(db): State<Db>, Valid(form): Valid<ProductForm>) -> Result<(Toast, Redirect)> {
    let mut product = Product::default(); // the nil UUID: not saved yet
    form.apply(&mut product);
    let product = Product::create(&db, product).await?;
    // The toast is shown on the next page (or at once for an htmx request).
    Ok((
        Toast::success("Product created."),
        Redirect::route("products.edit", &[&product.id])?,
    ))
}

/// The product read-only, on the kit's infolist.
async fn show(State(db): State<Db>, Path(id): Path<Uuid>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    Ok(view("products/show.html", context! { product }))
}

async fn edit(State(db): State<Db>, Path(id): Path<Uuid>) -> Result<View> {
    Ok(form_view(Some(&Product::find_or_404(&db, id).await?)))
}

async fn update(
    State(db): State<Db>,
    Path(id): Path<Uuid>,
    Valid(form): Valid<ProductForm>,
) -> Result<(Toast, Redirect)> {
    let mut product = Product::find_or_404(&db, id).await?;
    form.apply(&mut product);
    product.save(&db).await?;
    Ok((
        Toast::success("Saved."),
        Redirect::route("products.edit", &[&id])?,
    ))
}

/// The index page's Delete, behind the kit's `confirm` sheet.
async fn destroy(State(db): State<Db>, Path(id): Path<Uuid>) -> Result<(Toast, Redirect)> {
    let mut product = Product::find_or_404(&db, id).await?;
    product.delete(&db).await?; // no soft deletes here: the row goes
    Ok((
        Toast::success(format!("“{}” deleted.", product.name)),
        Redirect::route("home", &[])?,
    ))
}

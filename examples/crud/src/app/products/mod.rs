//! Made with `rnx make:module products`,
//! `rnx make:model Product --module products --migration` and
//! `rnx make:policy Product --module products`, then filled in.

pub mod model;
pub mod policy;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

use model::Product;

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
}

/// What the create and edit forms send.
#[derive(Deserialize, Serialize)]
struct ProductForm {
    name: String,
    price: i64,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100);
        v.field("price", &self.price).min(0);
    }
}

async fn home() -> Redirect {
    Redirect::to("/products")
}

async fn index(State(db): State<Db>, user: Option<AuthUser>, Page(page): Page) -> Result<View> {
    let products = Product::query()
        .latest()
        .paginate(&db, page, 10)
        .await?
        // Lets the view ask the policy: `can('update', product)`.
        .map(|product| Can::new(product, user.as_deref(), &["update", "delete"]));
    Ok(view("products/index.html", context! { products }))
}

async fn create() -> View {
    view("products/form.html", context! {})
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Valid(form): Valid<ProductForm>,
) -> Result<Redirect> {
    let product = Product {
        user_id: user.id,
        name: form.name,
        price: form.price,
        ..Default::default()
    };
    Product::create(&db, product).await?;
    session.flash("status", "Product created.")?;
    Ok(Redirect::to("/products"))
}

async fn edit(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let product = Product::find_or_404(&db, id).await?;
    user.authorize("update", &product)?;
    Ok(view("products/form.html", context! { product }))
}

async fn update(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Path(id): Path<i64>,
    Valid(form): Valid<ProductForm>,
) -> Result<Redirect> {
    let mut product = Product::find_or_404(&db, id).await?;
    user.authorize("update", &product)?;
    product.name = form.name;
    product.price = form.price;
    product.save(&db).await?;
    session.flash("status", "Product updated.")?;
    Ok(Redirect::to("/products"))
}

async fn destroy(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Redirect> {
    let mut product = Product::find_or_404(&db, id).await?;
    user.authorize("delete", &product)?;
    product.delete(&db).await?; // soft: sets deleted_at
    session.flash("status", "Product moved to the trash.")?;
    Ok(Redirect::to("/products"))
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
    session: Session,
    Path(id): Path<i64>,
) -> Result<Redirect> {
    let mut product = Product::query()
        .only_trashed()
        .where_eq("id", id)
        .first(&db)
        .await?
        .ok_or(Error::NotFound)?;
    user.authorize("restore", &product)?;
    product.restore(&db).await?;
    session.flash("status", "Product restored.")?;
    Ok(Redirect::to("/products"))
}

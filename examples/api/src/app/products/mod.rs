//! Made with `rnx make:module products` and `rnx make:model Product --module products -m`.

use std::time::Duration;

use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products")]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub price: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        // Apps log in here without a session, so without a CSRF token. That's
        // safe: no cookie is involved, and another site can't read the reply.
        let tokens = Routes::new()
            .post("/api/tokens", issue_token)
            .name("api.tokens.store")
            .without_csrf();
        let api = Routes::new()
            .get("/api/products", index)
            .name("api.products.index")
            .post("/api/products", store)
            .name("api.products.store")
            .get("/api/products/{id}", show)
            .name("api.products.show")
            .delete("/api/tokens/current", revoke_current)
            .name("api.tokens.destroy")
            .delete("/api/tokens", revoke_all)
            .name("api.tokens.destroy_all")
            .require_auth(); // a missing or wrong Bearer token → 401
        tokens
            .merge(api)
            .throttle(60, Duration::from_secs(60))
            .cors(&["https://app.example.com"])
    }
}

#[derive(Deserialize, Serialize)]
struct Login {
    email: String,
    password: String,
    /// Shown in the user's list of tokens, e.g. "Arif's iPhone".
    device: String,
}

impl Validate for Login {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
        v.field("password", &self.password).required();
        v.field("device", &self.device).required().max(100);
    }
}

async fn issue_token(
    State(db): State<Db>,
    Valid(login): Valid<Login>,
) -> Result<Json<renox::serde_json::Value>> {
    let Some(user) = User::attempt(&db, &login.email, &login.password).await? else {
        return Err(Error::Unauthorized);
    };
    let token = user.create_token(&db, &login.device, None).await?;
    Ok(Json(
        json!({ "token": token.plain, "user": { "id": user.id, "name": user.name } }),
    ))
}

/// "Log out" in the app: this device's token stops working.
async fn revoke_current(State(db): State<Db>, user: AuthUser) -> Result<StatusCode> {
    if let Some(id) = user.token_id() {
        user.revoke_token(&db, id).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// "Log out everywhere": every token of this user stops working.
async fn revoke_all(State(db): State<Db>, user: AuthUser) -> Result<StatusCode> {
    user.revoke_tokens(&db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<Json<Paginated<Product>>> {
    Ok(Json(
        Product::query()
            .order_by("name")
            .paginate(&db, page, 20)
            .await?,
    ))
}

async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<Json<Product>> {
    Ok(Json(Product::find_or_404(&db, id).await?))
}

#[derive(Deserialize)]
struct NewProduct {
    name: String,
    price: i64,
}

impl Validate for NewProduct {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name)
            .required()
            .max(100)
            .unique("products", "name");
        v.field("price", &self.price).min(0);
    }
}

// JSON bodies are validated like forms; errors come back as 422 JSON.
async fn store(
    State(db): State<Db>,
    Valid(input): Valid<NewProduct>,
) -> Result<(StatusCode, Json<Product>)> {
    let product = Product {
        name: input.name,
        price: input.price,
        ..Default::default()
    };
    Ok((
        StatusCode::CREATED,
        Json(Product::create(&db, product).await?),
    ))
}

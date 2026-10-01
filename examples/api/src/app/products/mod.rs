//! Made with `rnx make:module products` and `rnx make:model Product --module products -m`.

use renox::db::{CursorPage, Ulid};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products")]
pub struct Product {
    /// A ULID (`01J9Z3…`), made on insert: a public id that sorts by
    /// creation time and doesn't reveal how many products there are. It's
    /// also what the list's cursors are made of.
    pub id: Ulid,
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
        // `require_ability` (like every route layer) guards only the routes
        // added before it, so each ability gets its own group. A token
        // without the ability gets 403; logged-in sessions always pass.
        let read = Routes::new()
            .get("/api/products", index)
            .name("api.products.index")
            .get("/api/products/{id}", show)
            .name("api.products.show")
            .require_ability("products:read");
        let write = Routes::new()
            .post("/api/products", store)
            .name("api.products.store")
            .delete("/api/products/{id}", destroy)
            .name("api.products.destroy")
            .require_ability("products:write");
        let account = Routes::new()
            .delete("/api/tokens/current", revoke_current)
            .name("api.tokens.destroy")
            .delete("/api/tokens", revoke_all)
            .name("api.tokens.destroy_all");
        // Added last, so it runs first: a missing, wrong or expired Bearer
        // token → 401 before any ability is checked.
        let api = read.merge(write).merge(account).require_auth();
        tokens
            .merge(api)
            // The limit is picked per request by the `api` limiter (lib.rs).
            .throttle_by("api")
            .cors(&["https://app.example.com"])
    }
}

#[derive(Deserialize, Serialize)]
struct Login {
    email: String,
    password: String,
    /// Shown in the user's list of tokens, e.g. "Arif's iPhone".
    device: String,
    /// `true` asks for a token that can only read (e.g. for a dashboard);
    /// by default the token can read and write products.
    #[serde(default)]
    read_only: bool,
}

/// How long an API token works; the app logs in again after that.
const TOKEN_LIFETIME_DAYS: i64 = 30;

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
    // Give each token only the abilities it needs, and an expiry, so a
    // leaked token is limited in what it can do and for how long.
    let abilities: &[&str] = if login.read_only {
        &["products:read"]
    } else {
        &["products:read", "products:write"]
    };
    let expires_at = renox::db::now() + renox::chrono::TimeDelta::days(TOKEN_LIFETIME_DAYS);
    let token = user
        .create_token_with(&db, &login.device, abilities, Some(expires_at))
        .await?;
    Ok(Json(json!({
        "token": token.plain,
        "abilities": abilities,
        "expires_at": expires_at,
        "user": { "id": user.id, "name": user.name },
    })))
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

#[derive(Deserialize)]
struct ListParams {
    /// The `next_cursor` of the previous response; none for the first page.
    cursor: Option<String>,
}

/// Newest products first, 20 at a time. The reply's `next_cursor` goes back
/// as `?cursor=…` for the next 20; it is `null` on the last page. Unlike page
/// numbers, products added meanwhile don't shift the pages.
async fn index(
    State(db): State<Db>,
    Query(params): Query<ListParams>,
) -> Result<Json<CursorPage<Product>>> {
    Ok(Json(
        Product::query()
            .cursor_paginate(&db, params.cursor.as_deref(), 20)
            .await?,
    ))
}

/// A malformed id in the path is a 404, like an unknown one.
async fn show(State(db): State<Db>, Path(id): Path<Ulid>) -> Result<Json<Product>> {
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

async fn destroy(State(db): State<Db>, Path(id): Path<Ulid>) -> Result<StatusCode> {
    Product::find_or_404(&db, id).await?.delete(&db).await?;
    Ok(StatusCode::NO_CONTENT)
}

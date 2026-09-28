//! Example: a small online shop, the whole way. Customers browse and search
//! products, fill a cart and check out; the order takes the stock in one
//! transaction, a confirmation mail goes out through the queue, and admins
//! manage products (with photos) and ship orders from `/admin`.
//!
//! Run it from this directory:
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed                          # categories, products, admin@example.com
//! cargo run -- shop:make-admin you@example.com  # after registering
//! cargo run
//! ```

pub mod app;

use renox::prelude::*;

use app::catalog::model::{Category, Product};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        .module(app::catalog::Catalog)
        .module(app::cart::Cart)
        .module(app::orders::Orders)
        .module(app::admin::AdminPanel)
        // `role` is the column the first migration adds to `users`.
        .gate("admin", is_admin)
        .templates(|env| {
            env.add_filter("rupiah", |n: i64| {
                format!("Rp {}", renox::format_number(n as f64, 0, "id"))
            });
        })
        // The number in the cart link, on every page.
        .share("cart_count", |ctx| async move {
            let Some(user) = ctx.user else { return Ok(0) };
            let n: i64 = renox::db::sql(
                "SELECT COALESCE(SUM(quantity), 0) FROM cart_items WHERE user_id = ?",
            )
            .bind(user.id)
            .scalar(&ctx.state.db)
            .await?;
            Ok(n)
        })
        .command(
            "shop:make-admin",
            "Give a registered user the admin role: shop:make-admin EMAIL",
            make_admin,
        )
        .seeder(seed)
}

pub fn is_admin(user: &User) -> bool {
    user.get::<String>("role").as_deref() == Some("admin")
}

/// Everyone with the admin role.
pub async fn admins(db: &Db) -> Result<Vec<User>> {
    let ids: Vec<i64> = renox::db::sql("SELECT id FROM users WHERE role = 'admin' ORDER BY id")
        .scalars(db)
        .await?;
    User::find_many(db, ids).await
}

async fn make_admin(state: AppState, args: renox::command::Args) -> Result {
    let Some(email) = args.positional().first().copied() else {
        return Err(Error::BadRequest("usage: shop:make-admin EMAIL".into()));
    };
    let mut user = User::where_eq("email", email)
        .first(&state.db)
        .await?
        .ok_or_else(|| Error::BadRequest(format!("no user has the email {email}")))?;
    user.set(&state.db, "role", "admin").await?;
    println!("{email} is an admin now.");
    Ok(())
}

async fn seed(db: Db) -> Result {
    let mut admin = User::register(&db, "Admin", "admin@example.com", "password123").await?;
    admin.set(&db, "role", "admin").await?;
    for name in ["Coffee", "Tea", "Snacks"] {
        let category = Category::create(
            &db,
            Category {
                name: name.into(),
                slug: app::catalog::model::slug(name),
                ..Default::default()
            },
        )
        .await?;
        for _ in 0..8 {
            let mut product = Product::make();
            product.category_id = Some(category.id);
            product.slug = format!("{}-{}", product.slug, category.id); // fake names repeat
            if Product::where_eq("slug", &product.slug).count(&db).await? == 0 {
                Product::create(&db, product).await?;
            }
        }
    }
    Ok(())
}

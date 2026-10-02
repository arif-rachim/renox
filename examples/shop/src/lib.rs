//! Example: a small online shop, the whole way. Customers browse and search
//! products, fill a cart and check out; the order takes the stock in one
//! transaction, a confirmation mail goes out through the queue, and admins
//! manage products (with photos) and ship orders from `/admin`. Admins are
//! the users with the `admin` role (the `Permissions` module), and what they
//! do to orders goes into the audit log (the `Audit` module).
//!
//! The modules name the `rnx make:*` commands that made them. The
//! `shop:make-admin` command below is the kind `rnx make:command
//! shop:make-admin --module admin` writes; it sits here next to the roles.
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

use renox::audit::Audit;
use renox::auth::{Permissions, permissions};
use renox::clap;
use renox::command::AppCommand;
use renox::prelude::*;

use app::catalog::model::{Category, Product};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new())
        // Roles for users: `require_role("admin")` on `/admin`,
        // `user.has_role("admin")` in handlers, `auth.roles` in templates.
        .module(Permissions)
        // An `audit_logs` table: logins and account changes are recorded on
        // their own; the admin records order changes (`admin/orders.rs`).
        .module(Audit)
        .module(app::catalog::Catalog)
        .module(app::cart::Cart)
        .module(app::orders::Orders)
        .module(app::admin::AdminPanel)
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
        .typed_command::<MakeAdmin>()
        .seeder(seed)
}

/// The role that opens `/admin`.
pub const ADMIN: &str = "admin";

/// Gives `user` the admin role, creating the role on a fresh install.
pub async fn make_admin_of(db: &Db, user: &User) -> Result {
    // `define_role` is idempotent: it creates the role if it's new and sets
    // the permissions it grants (none: the shop checks the role itself).
    permissions::define_role(db, ADMIN, &[]).await?;
    user.assign_role(db, ADMIN).await
}

/// Everyone with the admin role (e.g. to tell them about a new order), by id.
pub async fn admins(db: &Db) -> Result<Vec<User>> {
    permissions::users_with_role(db, ADMIN).await
}

/// Give a registered user the admin role.
#[derive(clap::Parser)]
#[command(name = "shop:make-admin")]
struct MakeAdmin {
    /// Their email; asked for when it's left out.
    email: Option<String>,
}

impl AppCommand for MakeAdmin {
    async fn run(self, state: AppState) -> Result {
        let email = match self.email {
            Some(email) => email,
            None => renox::prompt::ask("Email of the new admin").await?,
        };
        let user = User::where_eq("email", &email)
            .first(&state.db)
            .await?
            .ok_or_else(|| Error::BadRequest(format!("no user has the email {email}")))?;
        make_admin_of(&state.db, &user).await?;
        println!("{email} is an admin now.");
        Ok(())
    }
}

async fn seed(db: Db) -> Result {
    let admin = User::register(&db, "Admin", "admin@example.com", "password123").await?;
    make_admin_of(&db, &admin).await?;
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

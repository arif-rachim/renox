//! Roles and permissions, opt in with `App::module(Permissions)`.
//!
//! A role (`editor`) grants permissions (`posts.publish`); users have roles.
//! Define roles once (a seeder or a command), assign them to users, then
//! check in handlers, routes and templates:
//!
//! ```
//! use renox::prelude::*;
//! use renox::auth::{Permissions, permissions};
//!
//! fn app() -> App {
//!     App::new()
//!         .module(Auth::new())
//!         .module(Permissions)
//!         .gate_before(|user, _ability| {
//!             // Super-admins may do everything (a column the app added).
//!             (user.get::<bool>("is_super_admin") == Some(true)).then_some(true)
//!         })
//! }
//!
//! async fn setup(db: &Db, user: &User) -> Result {
//!     permissions::define_role(db, "editor", &["posts.create", "posts.publish"]).await?;
//!     user.assign_role(db, "editor").await?;
//!     Ok(())
//! }
//!
//! async fn publish(user: AuthUser) -> Result<&'static str> {
//!     if !user.allows("posts.publish") { // gate_before, a gate, then permissions
//!         return Err(Error::Forbidden);
//!     }
//!     let _editor = user.has_role("editor");
//!     Ok("published")
//! }
//!
//! fn routes() -> Routes {
//!     Routes::new()
//!         .post("/posts/{id}/publish", publish)
//!         .require_permission("posts.publish") // 403 otherwise, shown in route:list
//! }
//! # let _ = (app, routes);
//! ```
//!
//! In templates, `can('posts.publish')` and `auth.roles` work the same way.
//! A user's roles and permissions are loaded once per request.

use std::collections::HashSet;

use super::{Grants, User};
use crate::db::{Db, Migration, Model, now, sql};
use crate::{Module, Registry, Result, Routes};

const MIGRATIONS: &[Migration] = &[crate::db::framework_migration!(
    "permissions",
    "00010101000500_create_roles_and_permissions_tables"
)];

/// Adds roles and permissions to the app: their tables, and loading the
/// current user's grants for `AuthUser::has_role` / `has_permission`,
/// `allows`, `Routes::require_role` / `require_permission` and `can()` in
/// templates. Needs the `Auth` module's `users` table.
pub struct Permissions;

impl Module for Permissions {
    fn name(&self) -> &'static str {
        "permissions"
    }

    fn migrations(&self) -> &'static [Migration] {
        MIGRATIONS
    }

    fn routes(&self) -> Routes {
        Routes::new()
    }

    fn register(&self, app: &mut Registry) {
        app.permissions = true;
    }
}

/// Creates the role `name` if it's new, and makes it grant exactly
/// `permissions` (creating the ones that don't exist yet).
pub async fn define_role(db: &Db, name: &str, permissions: &[&str]) -> Result {
    let mut tx = db.begin().await?;
    let role = ensure(&mut tx, "roles", name).await?;
    sql("DELETE FROM permission_role WHERE role_id = ?")
        .bind(role)
        .execute(&mut tx)
        .await?;
    for permission in permissions {
        let permission = ensure(&mut tx, "permissions", permission).await?;
        link(&mut tx, permission, role).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Adds `permissions` to the existing role `name`.
pub async fn grant(db: &Db, role: &str, permissions: &[&str]) -> Result {
    let mut tx = db.begin().await?;
    let role = existing_role(&mut tx, role).await?;
    for permission in permissions {
        let permission = ensure(&mut tx, "permissions", permission).await?;
        link(&mut tx, permission, role).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Removes `permissions` from the role `name`.
pub async fn revoke(db: &Db, role: &str, permissions: &[&str]) -> Result {
    let mut tx = db.begin().await?;
    let role = existing_role(&mut tx, role).await?;
    for permission in permissions {
        sql(
            "DELETE FROM permission_role WHERE role_id = ? AND permission_id = \
             (SELECT id FROM permissions WHERE name = ?)",
        )
        .bind(role)
        .bind(*permission)
        .execute(&mut tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Deletes the role `name`; users lose it. Returns whether it existed.
pub async fn delete_role(db: &Db, name: &str) -> Result<bool> {
    Ok(sql("DELETE FROM roles WHERE name = ?")
        .bind(name)
        .execute(db)
        .await?
        > 0)
}

/// Every role, by name, with the permissions it grants.
pub async fn roles(db: &Db) -> Result<Vec<(String, Vec<String>)>> {
    let rows: Vec<(String, Option<String>)> = sql("SELECT r.name, p.name FROM roles r \
         LEFT JOIN permission_role pr ON pr.role_id = r.id \
         LEFT JOIN permissions p ON p.id = pr.permission_id \
         ORDER BY r.name, p.name")
    .fetch_as(db)
    .await?;
    let mut roles: Vec<(String, Vec<String>)> = Vec::new();
    for (role, permission) in rows {
        if roles.last().is_none_or(|(name, _)| *name != role) {
            roles.push((role, Vec::new()));
        }
        if let (Some(permission), Some((_, list))) = (permission, roles.last_mut()) {
            list.push(permission);
        }
    }
    Ok(roles)
}

impl User {
    /// Gives the user the existing role `role` (see `permissions::define_role`).
    pub async fn assign_role(&self, db: &Db, role: &str) -> Result {
        let mut tx = db.begin().await?;
        let role = existing_role(&mut tx, role).await?;
        sql("INSERT INTO role_user (role_id, user_id) SELECT ?, ? \
             WHERE NOT EXISTS (SELECT 1 FROM role_user WHERE role_id = ? AND user_id = ?)")
        .bind(role)
        .bind(self.id)
        .bind(role)
        .bind(self.id)
        .execute(&mut tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Takes the role `role` away from the user.
    pub async fn remove_role(&self, db: &Db, role: &str) -> Result {
        sql("DELETE FROM role_user WHERE user_id = ? AND role_id = \
             (SELECT id FROM roles WHERE name = ?)")
        .bind(self.id)
        .bind(role)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Makes the user's roles exactly `roles` (each must exist).
    pub async fn sync_roles(&self, db: &Db, roles: &[&str]) -> Result {
        let mut tx = db.begin().await?;
        let mut ids = Vec::new();
        for role in roles {
            ids.push(existing_role(&mut tx, role).await?);
        }
        sql("DELETE FROM role_user WHERE user_id = ?")
            .bind(self.id)
            .execute(&mut tx)
            .await?;
        for role in ids {
            sql("INSERT INTO role_user (role_id, user_id) VALUES (?, ?)")
                .bind(role)
                .bind(self.id)
                .execute(&mut tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The names of the user's roles, sorted.
    pub async fn roles(&self, db: &Db) -> Result<Vec<String>> {
        Ok(grants(db, self.id).await?.roles)
    }

    /// The permissions the user's roles grant, sorted.
    pub async fn permissions(&self, db: &Db) -> Result<Vec<String>> {
        let mut list: Vec<String> = grants(db, self.id).await?.permissions.into_iter().collect();
        list.sort();
        Ok(list)
    }
}

/// The users who have `role`, ordered by id (e.g. to notify every admin).
pub async fn users_with_role(db: &Db, role: &str) -> Result<Vec<User>> {
    let ids: Vec<i64> = sql(
        "SELECT ru.user_id FROM role_user ru JOIN roles r ON r.id = ru.role_id \
         WHERE r.name = ? ORDER BY ru.user_id",
    )
    .bind(role)
    .scalars(db)
    .await?;
    let mut users = User::find_many(db, ids).await?;
    users.sort_by_key(|u| u.id);
    Ok(users)
}

/// A user's roles and permissions, for the auth middleware.
pub(crate) async fn grants(db: &Db, user_id: i64) -> Result<Grants> {
    let roles: Vec<String> = sql(
        "SELECT r.name FROM roles r JOIN role_user ru ON ru.role_id = r.id \
         WHERE ru.user_id = ? ORDER BY r.name",
    )
    .bind(user_id)
    .scalars(db)
    .await?;
    let permissions: Vec<String> = if roles.is_empty() {
        Vec::new()
    } else {
        sql("SELECT DISTINCT p.name FROM permissions p \
             JOIN permission_role pr ON pr.permission_id = p.id \
             JOIN role_user ru ON ru.role_id = pr.role_id WHERE ru.user_id = ?")
        .bind(user_id)
        .scalars(db)
        .await?
    };
    Ok(Grants {
        roles,
        permissions: permissions.into_iter().collect::<HashSet<_>>(),
    })
}

/// The id of the row named `name` in `table`, created if missing.
async fn ensure(tx: &mut crate::db::Transaction, table: &str, name: &str) -> Result<i64> {
    let at = now();
    sql(format!(
        "INSERT INTO {table} (name, created_at, updated_at) SELECT ?, ?, ? \
         WHERE NOT EXISTS (SELECT 1 FROM {table} WHERE name = ?)"
    ))
    .bind(name)
    .bind(at)
    .bind(at)
    .bind(name)
    .execute(&mut *tx)
    .await?;
    Ok(sql(format!("SELECT id FROM {table} WHERE name = ?"))
        .bind(name)
        .scalar(&mut *tx)
        .await?)
}

async fn existing_role(tx: &mut crate::db::Transaction, name: &str) -> Result<i64> {
    let id: Option<i64> = sql("SELECT id FROM roles WHERE name = ?")
        .bind(name)
        .scalars(&mut *tx)
        .await?
        .into_iter()
        .next();
    id.ok_or_else(|| {
        anyhow::anyhow!("there is no role `{name}`; create it with permissions::define_role").into()
    })
}

async fn link(tx: &mut crate::db::Transaction, permission: i64, role: i64) -> Result {
    sql(
        "INSERT INTO permission_role (permission_id, role_id) SELECT ?, ? \
         WHERE NOT EXISTS (SELECT 1 FROM permission_role WHERE permission_id = ? AND role_id = ?)",
    )
    .bind(permission)
    .bind(role)
    .bind(permission)
    .bind(role)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

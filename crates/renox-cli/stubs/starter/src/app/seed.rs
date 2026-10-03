//! Demo data for a local database: `cargo run -- db:seed` (or
//! `migrate:fresh --seed`). Seeding twice changes nothing.

use renox::prelude::*;

use super::roles::{self, ADMIN, MEMBER};

pub async fn run(state: AppState) -> Result {
    let db = &state.db;
    roles::define(db).await?;
    for (name, email, role) in [
        ("Admin", "admin@example.com", ADMIN),
        ("Member", "member@example.com", MEMBER),
    ] {
        if User::find_by_email(db, email).await?.is_some() {
            continue;
        }
        let mut user = User::register(db, name, email, "password123").await?;
        // Seeded people need no verification mail.
        user.email_verified_at = Some(renox::db::now());
        user.save(db).await?;
        user.assign_role(db, role).await?;
    }
    Ok(())
}

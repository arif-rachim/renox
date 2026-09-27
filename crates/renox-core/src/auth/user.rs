use std::future::Future;
use std::sync::LazyLock;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};

use super::Policy;
use crate::db::{DateTime, Db, DbValue, Executor, Model, Row, ToDbValue};
use crate::{Error, Result};

/// A row of the `users` table created by the `Auth` module.
///
/// Add your own columns with a migration and read them through your own
/// model on the same table, e.g. `#[model(table = "users")] struct Pelanggan`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    /// The Argon2 hash; never serialized, so it can't leak into templates or JSON.
    #[serde(skip_serializing, default)]
    pub password: String,
    pub email_verified_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Model for User {
    const TABLE: &'static str = "users";
    const COLUMNS: &'static [&'static str] = &[
        "id",
        "name",
        "email",
        "password",
        "email_verified_at",
        "created_at",
        "updated_at",
    ];

    fn id(&self) -> i64 {
        self.id
    }

    fn set_id(&mut self, id: i64) {
        self.id = id;
    }

    fn from_row(row: &Row) -> std::result::Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            email: row.try_get("email")?,
            password: row.try_get("password")?,
            email_verified_at: row.try_get("email_verified_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn values(&self) -> Vec<DbValue> {
        vec![
            self.name.to_db_value(),
            self.email.to_db_value(),
            self.password.to_db_value(),
            self.email_verified_at.to_db_value(),
            self.created_at.to_db_value(),
            self.updated_at.to_db_value(),
        ]
    }

    fn touch(&mut self, now: DateTime, creating: bool) {
        if creating && self.created_at.is_none() {
            self.created_at = Some(now);
        }
        self.updated_at = Some(now);
    }
}

impl User {
    /// Emails are matched case-insensitively.
    pub fn find_by_email<'c, E: Executor<'c>>(
        db: E,
        email: &str,
    ) -> impl Future<Output = Result<Option<Self>>> + Send {
        Self::query().where_eq("email", email.trim()).first(db)
    }

    /// Creates a user with a hashed password.
    pub async fn register(db: &Db, name: &str, email: &str, password: &str) -> Result<Self> {
        let user = Self {
            name: name.trim().to_owned(),
            email: email.trim().to_owned(),
            password: hash_password(password).await?,
            ..Self::default()
        };
        Self::create(db, user).await
    }

    /// Changes the password, which also logs out the user's other sessions.
    pub async fn set_password(&mut self, db: &Db, password: &str) -> Result {
        self.password = hash_password(password).await?;
        self.save(db).await
    }

    pub async fn check_password(&self, password: &str) -> bool {
        verify_password(password, &self.password).await
    }

    pub fn can(&self, ability: &str, target: &impl Policy) -> bool {
        target.allows(self, ability)
    }

    pub fn authorize(&self, ability: &str, target: &impl Policy) -> Result {
        if self.can(ability, target) {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}

/// A real hash of a throwaway password, verified against when an email is
/// unknown so the response takes as long as for a real account.
pub(crate) fn dummy_hash() -> String {
    static HASH: LazyLock<String> = LazyLock::new(|| {
        Argon2::default()
            .hash_password(b"renox-timing-equaliser")
            .map(|hash| hash.to_string())
            .unwrap_or_default()
    });
    HASH.clone()
}

/// Hashes a password with Argon2id on a blocking thread.
pub async fn hash_password(password: &str) -> Result<String> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|err| anyhow::anyhow!("could not hash the password: {err}"))
    })
    .await
    .map_err(anyhow::Error::from)?
    .map_err(Error::from)
}

/// Checks a password against an Argon2 hash on a blocking thread.
pub async fn verify_password(password: &str, hash: &str) -> bool {
    let (password, hash) = (password.to_owned(), hash.to_owned());
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_verify() {
        let hash = hash_password("rahasia123").await.unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("rahasia123", &hash).await);
        assert!(!verify_password("salah", &hash).await);
        assert!(!verify_password("rahasia123", "not a hash").await);
    }
}

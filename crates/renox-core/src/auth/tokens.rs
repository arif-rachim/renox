use serde::Serialize;

use super::User;
use crate::Result;
use crate::crypto::{constant_time_eq, random_token};
use crate::db::{DateTime, Db, Model, now};

/// An API token. Only a SHA-256 hash of the secret is stored.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AccessToken {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub last_used_at: Option<DateTime>,
    pub expires_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
}

/// A freshly created token. `plain` is shown once; send it as
/// `Authorization: Bearer <plain>`.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct NewToken {
    pub token: AccessToken,
    pub plain: String,
}

pub(crate) fn sha256_hex(value: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn from_row(row: &crate::db::Row) -> std::result::Result<AccessToken, crate::db::DbError> {
    Ok(AccessToken {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        last_used_at: row.try_get("last_used_at")?,
        expires_at: row.try_get("expires_at")?,
        created_at: row.try_get("created_at")?,
    })
}

const COLUMNS: &str = "id, user_id, name, last_used_at, expires_at, created_at";

impl User {
    /// Creates an API token, optionally expiring at `expires_at`.
    pub async fn create_token(
        &self,
        db: &Db,
        name: &str,
        expires_at: Option<DateTime>,
    ) -> Result<NewToken> {
        let secret = random_token();
        let created = now();
        let row = crate::db::sql(format!(
            "INSERT INTO personal_access_tokens (user_id, name, token, expires_at, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) RETURNING {COLUMNS}"
        ))
        .bind(self.id)
        .bind(name)
        .bind(sha256_hex(&secret))
        .bind(expires_at)
        .bind(created)
        .bind(created)
        .fetch_one(db)
        .await?;
        let token = from_row(&row)?;
        Ok(NewToken {
            plain: format!("{}|{secret}", token.id),
            token,
        })
    }

    /// The user's API tokens, newest first.
    pub async fn tokens(&self, db: &Db) -> Result<Vec<AccessToken>> {
        let rows = crate::db::sql(format!(
            "SELECT {COLUMNS} FROM personal_access_tokens WHERE user_id = ? ORDER BY id DESC"
        ))
        .bind(self.id)
        .fetch_all(db)
        .await?;
        Ok(rows
            .iter()
            .map(from_row)
            .collect::<std::result::Result<_, _>>()?)
    }

    /// Revokes one of the user's tokens; returns whether it existed.
    pub async fn revoke_token(&self, db: &Db, token_id: i64) -> Result<bool> {
        let done =
            crate::db::sql("DELETE FROM personal_access_tokens WHERE id = ? AND user_id = ?")
                .bind(token_id)
                .bind(self.id)
                .execute(db)
                .await?;
        Ok(done > 0)
    }

    /// Revokes all of the user's tokens.
    pub async fn revoke_tokens(&self, db: &Db) -> Result<u64> {
        let done = crate::db::sql("DELETE FROM personal_access_tokens WHERE user_id = ?")
            .bind(self.id)
            .execute(db)
            .await?;
        Ok(done)
    }
}

/// The user behind `Authorization: Bearer <id|secret>` and the token's id,
/// if the token is valid.
pub(crate) async fn authenticate(db: &Db, bearer: &str) -> Result<Option<(User, i64)>> {
    let Some((id, secret)) = bearer.split_once('|') else {
        return Ok(None);
    };
    let Ok(id) = id.parse::<i64>() else {
        return Ok(None);
    };
    let Some(row) = crate::db::sql(
        "SELECT user_id, token, expires_at FROM personal_access_tokens WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let hash: String = row.try_get("token")?;
    let expires_at: Option<DateTime> = row.try_get("expires_at")?;
    if !constant_time_eq(&hash, &sha256_hex(secret)) || expires_at.is_some_and(|at| at <= now()) {
        return Ok(None);
    }
    crate::db::sql("UPDATE personal_access_tokens SET last_used_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(User::find(db, row.try_get("user_id")?)
        .await?
        .map(|user| (user, id)))
}

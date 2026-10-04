use renox::db::Encrypted;
use renox::prelude::*;
use serde::Serialize;

/// A user's two-factor authentication: their TOTP secret (sealed with
/// `APP_KEY`), whether they confirmed it, the last code used, and their
/// hashed recovery codes. A row without `confirmed_at` is a setup the user
/// hasn't finished: it doesn't count at login.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "two_factor")]
pub struct TwoFactorCredential {
    pub id: i64,
    pub user_id: i64,
    /// Never serialized: it isn't shown once confirmed.
    #[serde(skip)]
    pub secret: Encrypted<String>,
    pub confirmed_at: Option<DateTime>,
    pub last_used_step: Option<i64>,
    /// Hashed codes, as JSON.
    #[serde(skip)]
    pub recovery_codes: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl TwoFactorCredential {
    /// `user_id`'s row, confirmed or not.
    pub async fn of(db: &Db, user_id: i64) -> Result<Option<Self>> {
        Self::where_eq("user_id", user_id).first(db).await
    }

    /// Whether `user_id` has two-factor authentication on (confirmed).
    pub async fn enabled(db: &Db, user_id: i64) -> Result<bool> {
        Ok(Self::of(db, user_id)
            .await?
            .is_some_and(|credential| credential.confirmed_at.is_some()))
    }
}

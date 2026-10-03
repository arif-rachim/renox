//! The company's settings: one `settings` row per key, read into a typed
//! struct (missing keys take the defaults), edited on `/settings` by staff
//! with `settings.manage`. Shared with every view as `company`.

use renox::Toast;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    #[validate(required, max = 80)]
    pub company_name: String,
    #[validate(max = 300)]
    pub company_address: String,
    #[validate(email)]
    pub company_email: String,
    /// VAT on every invoice, in percent.
    #[validate(between(0, 100))]
    pub tax_percent: i64,
    /// Invoice numbers start with this: INV-00042.
    #[validate(required, max = 10, alpha_dash)]
    pub invoice_prefix: String,
    /// Days from issue to due.
    #[validate(between(0, 365))]
    pub payment_days: i64,
    /// Where customers pay online: `none`, `midtrans` or `xendit` (keys in
    /// `.env`).
    #[validate(one_of(&["none", "midtrans", "xendit"]))]
    pub payment_gateway: String,
    /// The accent colour of the pages, `#rrggbb`.
    #[validate(matches(r"^#[0-9a-fA-F]{6}$"))]
    pub brand_color: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            company_name: "Corner Store".into(),
            company_address: "17 Market Street, Bandung".into(),
            company_email: "hello@cornerstore.test".into(),
            tax_percent: 11,
            invoice_prefix: "INV-".into(),
            payment_days: 14,
            payment_gateway: "none".into(),
            brand_color: "#0f766e".into(),
        }
    }
}

impl Settings {
    /// The saved settings, with defaults for what was never saved.
    pub async fn load(db: &Db) -> Result<Settings> {
        let rows: Vec<(String, String)> = renox::db::sql("SELECT key, value FROM settings")
            .fetch_as(db)
            .await?;
        let mut map = renox::serde_json::Map::new();
        for (key, value) in rows {
            // Numbers are stored as text; serde wants them as numbers.
            let value = match value.parse::<i64>() {
                Ok(n) => json!(n),
                Err(_) => json!(value),
            };
            map.insert(key, value);
        }
        Ok(renox::serde_json::from_value(map.into()).unwrap_or_default())
    }

    /// Saves every key in one transaction.
    pub async fn save(&self, db: &Db) -> Result {
        let mut tx = db.begin().await?;
        if let renox::serde_json::Value::Object(map) = renox::serde_json::to_value(self)? {
            for (key, value) in map {
                let value = match value {
                    renox::serde_json::Value::String(s) => s,
                    other => other.to_string(),
                };
                renox::db::sql(
                    "INSERT INTO settings (key, value) VALUES (?, ?) \
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                )
                .bind(key)
                .bind(value)
                .execute(&mut tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

pub(super) async fn edit(State(db): State<Db>) -> Result<View> {
    let settings = Settings::load(&db).await?;
    Ok(view("settings/edit.html", context! { settings }))
}

pub(super) async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Valid(settings): Valid<Settings>,
) -> Result<(Toast, Redirect)> {
    let before = Settings::load(&state.db).await?;
    settings.save(&state.db).await?;
    renox::audit::record(
        &state.db,
        renox::audit::Entry::new("settings.updated")
            .user(user.id)
            .data(json!({ "before": before, "after": settings })),
    )
    .await?;
    Ok((
        Toast::success("Settings saved."),
        Redirect::route("settings.edit", &[])?,
    ))
}

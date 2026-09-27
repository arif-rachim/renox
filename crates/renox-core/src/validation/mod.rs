//! Form validation with Laravel-style rules, database-backed `unique` and
//! `exists`, and messages in English and Indonesian.
//!
//! ```ignore
//! #[derive(Deserialize, Serialize)]
//! struct ProdukForm {
//!     nama: String,
//!     harga: i64,
//!     email: Option<String>,
//! }
//!
//! impl Validate for ProdukForm {
//!     fn rules(&self, v: &mut Validator) {
//!         v.field("nama", &self.nama).required().max(100).unique("produk", "nama");
//!         v.field("harga", &self.harga).label("harga jual").min(1_000);
//!         v.field("email", &self.email).email();
//!     }
//! }
//!
//! async fn store(State(db): State<Db>, Valid(form): Valid<ProdukForm>) -> Result<Back> { ... }
//! ```

mod extract;
mod messages;
mod value;

use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{AssertSqlSafe, Row};

pub use extract::Valid;
pub use messages::Locale;
pub use value::{FieldValue, Inspected};

use crate::Result;
use crate::db::value_bind;
use crate::db::{Db, DbValue, quote};
use messages::{render, template};

/// A built-in message by key (e.g. `required`, `auth.failed`) in `locale`.
pub(crate) fn message(locale: Locale, key: &str, label: &str, params: &[(&str, String)]) -> String {
    render(template(locale, key), label, params)
}

/// Validation errors: messages keyed by field name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Errors(BTreeMap<String, Vec<String>>);

impl Errors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.entry(field.into()).or_default().push(message.into());
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn has(&self, field: &str) -> bool {
        self.0.contains_key(field)
    }

    /// The first message for a field.
    pub fn first(&self, field: &str) -> Option<&str> {
        self.0
            .get(field)
            .and_then(|m| m.first())
            .map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.0.iter().map(|(f, m)| (f.as_str(), m.as_slice()))
    }
}

/// Declares the rules for a form. Used by the `Valid<T>` extractor.
pub trait Validate {
    fn rules(&self, v: &mut Validator);
}

struct Pending {
    field: String,
    label: String,
    table: String,
    column: String,
    value: DbValue,
    ignore_id: Option<i64>,
    unique: bool,
    message: Option<String>,
}

/// Collects rule failures. Rules run in order and stop at a field's first
/// failure; rules other than `required` and `accepted` skip empty values.
pub struct Validator {
    locale: Locale,
    errors: Errors,
    pending: Vec<Pending>,
}

impl Validator {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            errors: Errors::new(),
            pending: Vec::new(),
        }
    }

    /// Starts the rules for one field. The label in messages defaults to the
    /// name with `_` replaced by spaces.
    pub fn field<'v>(&'v mut self, name: &str, value: &impl FieldValue) -> Field<'v> {
        Field {
            label: name.replace('_', " "),
            name: name.to_owned(),
            value: value.inspect(),
            db_value: value.db_value(),
            failed: false,
            last_pending: None,
            v: self,
        }
    }

    /// The language messages are written in, e.g. to pick labels.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// Adds an error that no rule covers.
    pub fn error(&mut self, field: &str, message: impl Into<String>) {
        self.errors.add(field, message);
    }

    /// Runs the database checks and returns every error (empty when valid).
    pub async fn finish(self, db: &Db) -> Result<Errors> {
        let mut errors = self.errors;
        for check in self.pending {
            if errors.has(&check.field) {
                continue;
            }
            let mut sql = format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE {} = ?",
                quote(&check.table),
                quote(&check.column)
            );
            if check.ignore_id.is_some() {
                sql.push_str(" AND \"id\" != ?");
            }
            sql.push(')');
            let mut query = value_bind(sqlx::query(AssertSqlSafe(sql)), check.value);
            if let Some(id) = check.ignore_id {
                query = query.bind(id);
            }
            let found: bool = query.fetch_one(db).await?.try_get(0)?;
            if found == check.unique {
                let key = if check.unique { "unique" } else { "exists" };
                let message = check
                    .message
                    .unwrap_or_else(|| render(template(self.locale, key), &check.label, &[]));
                errors.add(check.field, message);
            }
        }
        Ok(errors)
    }

    /// Applies `data`'s rules; call `finish` to run the database checks.
    ///
    /// ```ignore
    /// let errors = Validator::rules_of(&form, Locale::Id).finish(&db).await?;
    /// ```
    pub fn rules_of(data: &impl Validate, locale: Locale) -> Self {
        let mut validator = Self::new(locale);
        data.rules(&mut validator);
        validator
    }
}

/// The rules for one field, chained: `v.field("nama", &self.nama).required().max(100)`.
pub struct Field<'v> {
    v: &'v mut Validator,
    name: String,
    label: String,
    value: Inspected,
    db_value: DbValue,
    failed: bool,
    last_pending: Option<usize>,
}

fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
}

impl Field<'_> {
    /// The name used in messages, e.g. `.label("harga jual")`.
    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }

    fn fail(&mut self, key: &str, params: &[(&str, String)]) {
        if !self.failed {
            let message = render(template(self.v.locale, key), &self.label, params);
            self.v.errors.add(&self.name, message);
            self.failed = true;
            self.last_pending = None;
        }
    }

    fn present(&self) -> bool {
        !self.failed && self.value != Inspected::Missing
    }

    /// Replaces the message of the rule just before it.
    pub fn message(self, message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(i) = self.last_pending {
            self.v.pending[i].message = Some(message);
        } else if self.failed
            && let Some(last) = self
                .v
                .errors
                .0
                .get_mut(&self.name)
                .and_then(|messages| messages.last_mut())
        {
            *last = message;
        }
        self
    }

    pub fn required(mut self) -> Self {
        if !self.failed && self.value == Inspected::Missing {
            self.fail("required", &[]);
        }
        self
    }

    fn size_rule(
        mut self,
        kind: &str,
        ok: impl Fn(f64) -> bool,
        params: &[(&str, String)],
    ) -> Self {
        if !self.present() {
            return self;
        }
        let (size, suffix) = match &self.value {
            Inspected::Text(text) => (text.chars().count() as f64, "string"),
            Inspected::Number(n) => (*n, "numeric"),
            Inspected::Items(n) => (*n as f64, "array"),
            _ => return self,
        };
        if !ok(size) {
            self.fail(&format!("{kind}.{suffix}"), params);
        }
        self
    }

    /// At least `min` characters, items, or as a number.
    pub fn min(self, min: impl Into<f64>) -> Self {
        let min = min.into();
        self.size_rule("min", |s| s >= min, &[("min", number(min))])
    }

    /// At most `max` characters, items, or as a number.
    pub fn max(self, max: impl Into<f64>) -> Self {
        let max = max.into();
        self.size_rule("max", |s| s <= max, &[("max", number(max))])
    }

    pub fn between(self, min: impl Into<f64>, max: impl Into<f64>) -> Self {
        let (min, max) = (min.into(), max.into());
        self.size_rule(
            "between",
            |s| s >= min && s <= max,
            &[("min", number(min)), ("max", number(max))],
        )
    }

    pub fn email(mut self) -> Self {
        if let (true, Inspected::Text(text)) = (self.present(), &self.value)
            && !is_email(text)
        {
            self.fail("email", &[]);
        }
        self
    }

    pub fn url(mut self) -> Self {
        if let (true, Inspected::Text(text)) = (self.present(), &self.value)
            && !is_url(text)
        {
            self.fail("url", &[]);
        }
        self
    }

    /// One of the given values (Laravel's `in`).
    pub fn one_of<V: FieldValue>(mut self, allowed: &[V]) -> Self {
        if self.present() && !allowed.iter().any(|a| a.inspect() == self.value) {
            self.fail("in", &[]);
        }
        self
    }

    /// Equal to its confirmation field, e.g. `password` and `password_confirmation`.
    pub fn confirmed(mut self, confirmation: &impl FieldValue) -> Self {
        if self.present() && confirmation.inspect() != self.value {
            self.fail("confirmed", &[]);
        }
        self
    }

    /// A checkbox that must be ticked.
    pub fn accepted(mut self) -> Self {
        if !self.failed && self.value != Inspected::Bool(true) {
            self.fail("accepted", &[]);
        }
        self
    }

    /// A custom check: fails with `message` when `valid` is false.
    pub fn rule(mut self, valid: bool, message: impl Into<String>) -> Self {
        if !self.failed && !valid {
            self.v.errors.add(&self.name, message);
            self.failed = true;
            self.last_pending = None;
        }
        self
    }

    fn database(mut self, table: &str, column: &str, unique: bool) -> Self {
        if self.present() {
            self.v.pending.push(Pending {
                field: self.name.clone(),
                label: self.label.clone(),
                table: table.to_owned(),
                column: column.to_owned(),
                value: self.db_value.clone(),
                ignore_id: None,
                unique,
                message: None,
            });
            self.last_pending = Some(self.v.pending.len() - 1);
        }
        self
    }

    /// No row in `table` has this value in `column`.
    pub fn unique(self, table: &str, column: &str) -> Self {
        self.database(table, column, true)
    }

    /// Skips the row with this id in the preceding `unique`, for updates.
    pub fn ignore(self, id: i64) -> Self {
        if let Some(i) = self.last_pending {
            self.v.pending[i].ignore_id = Some(id);
        }
        self
    }

    /// Some row in `table` has this value in `column`.
    pub fn exists(self, table: &str, column: &str) -> Self {
        self.database(table, column, false)
    }
}

fn is_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !text.chars().any(char::is_whitespace)
        && !domain.contains('@')
        && domain.contains('.')
        && domain.split('.').all(|part| !part.is_empty())
}

fn is_url(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    matches!(rest, Some(rest) if !rest.is_empty()
        && !rest.starts_with('/')
        && !text.chars().any(char::is_whitespace))
}

/// Keys never flashed back into forms.
const DONT_FLASH: &[&str] = &[
    "password",
    "password_confirmation",
    "current_password",
    "_token",
];

/// A failed validation. As a response it is `422` with
/// `{"message": ..., "errors": {...}}`; for regular (non-HTMX, non-JSON)
/// requests Renox turns it into a redirect back with the errors and old
/// input flashed to the session.
///
/// Return it from a handler for errors found after validation:
///
/// ```ignore
/// let mut errors = Errors::new();
/// errors.add("stok", "Stok tidak cukup.");
/// return Err(ValidationError::new(errors).with_input(&form).into());
/// ```
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub errors: Errors,
    pub input: Map<String, Value>,
}

impl ValidationError {
    pub fn new(errors: Errors) -> Self {
        Self {
            errors,
            input: Map::new(),
        }
    }

    /// The submitted values to refill the form with (passwords are dropped).
    pub fn with_input(mut self, input: &impl Serialize) -> Self {
        if let Ok(Value::Object(map)) = serde_json::to_value(input) {
            self.input = map;
        }
        self.input
            .retain(|key, _| !DONT_FLASH.contains(&key.as_str()));
        self
    }

    pub(crate) fn with_input_map(mut self, input: Map<String, Value>) -> Self {
        self.input = input;
        self.input
            .retain(|key, _| !DONT_FLASH.contains(&key.as_str()));
        self
    }
}

impl From<Errors> for ValidationError {
    fn from(errors: Errors) -> Self {
        Self::new(errors)
    }
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        let message = self
            .errors
            .iter()
            .next()
            .and_then(|(_, m)| m.first().cloned())
            .unwrap_or_default();
        let body = json!({ "message": message, "errors": self.errors });
        let mut res = (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response();
        res.extensions_mut().insert(self);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_emails_and_urls() {
        assert!(is_email("arif@example.com"));
        assert!(!is_email("arif@localhost"));
        assert!(!is_email("arif example@x.com"));
        assert!(!is_email("@x.com"));
        assert!(is_url("https://renox.dev/docs"));
        assert!(!is_url("ftp://renox.dev"));
        assert!(!is_url("https://"));
    }

    #[test]
    fn formats_numbers_without_trailing_zeros() {
        assert_eq!(number(3.0), "3");
        assert_eq!(number(2.5), "2.5");
    }
}

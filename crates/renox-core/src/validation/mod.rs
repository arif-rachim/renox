//! Form validation with Laravel-style rules, database-backed `unique` and
//! `exists`, and messages in English and Indonesian.
//!
//! ```
//! # use renox::prelude::*;
//! use serde::{Deserialize, Serialize};
//!
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
//! async fn store(State(db): State<Db>, back: Back, Valid(form): Valid<ProdukForm>) -> Result<Back> {
//!     // `form` passed every rule; invalid input never gets here.
//! #   let _ = (db, form);
//!     Ok(back)
//! }
//! ```

pub(crate) mod extract;
mod messages;
mod value;

use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub use extract::Valid;
pub use messages::Locale;
pub(crate) use messages::{render, template_for};
pub use value::{FieldValue, Inspected};

use crate::Result;
use crate::db::{Db, DbValue, ToDbValue, quote};
use chrono::NaiveDateTime;

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

/// Declares the rules for a form. Used by the `Valid<T>` extractor, which
/// calls, in order: [`prepare`](Validate::prepare),
/// [`authorize`](Validate::authorize), [`rules`](Validate::rules) and, when
/// they pass, [`after`](Validate::after). Only `rules` is required.
///
/// ```
/// # use renox::prelude::*;
/// use renox::validation::FormContext;
///
/// #[derive(serde::Deserialize)]
/// struct Invite { email: String, team_id: i64 }
///
/// impl Validate for Invite {
///     // Laravel's prepareForValidation: tidy the input first.
///     fn prepare(&mut self) {
///         self.email = self.email.trim().to_lowercase();
///     }
///
///     // Laravel's authorize: `false` answers 403 before any rule runs.
///     async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {
///         Ok(form.user.is_some_and(|user| user.has_role("owner")))
///     }
///
///     fn rules(&self, v: &mut Validator) {
///         v.field("email", &self.email).required().email();
///     }
///
///     // Laravel's `after`: checks that need the database or several fields,
///     // once the rules pass. Errors added here are shown like any other.
///     async fn after(&self, form: &FormContext<'_>, errors: &mut Errors) -> Result {
///         let members: i64 = renox::db::sql("SELECT COUNT(*) FROM team_user WHERE team_id = ?")
///             .bind(self.team_id)
///             .scalar(&form.state.db)
///             .await?;
///         if members >= 10 {
///             errors.add("email", "This team is full (10 members).");
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Validate {
    fn rules(&self, v: &mut Validator);

    /// Tidies the input before anything checks it (Laravel's
    /// `prepareForValidation`): trim, lowercase an email, fill a slug. The
    /// form refilled after an error still shows what was typed.
    fn prepare(&mut self) {}

    /// Whether this request may send the form (Laravel's `authorize`), e.g.
    /// only the team's owner invites. `false` answers 403 before the rules
    /// run. It sees the input, so it runs once the input is read.
    fn authorize(
        &self,
        form: &FormContext<'_>,
    ) -> impl std::future::Future<Output = Result<bool>> + Send {
        let _ = form;
        std::future::ready(Ok(true))
    }

    /// Checks that need the database, another service or several fields,
    /// once the rules pass (Laravel's `after`, and async rules). Add
    /// errors with `errors.add(field, message)`.
    fn after(
        &self,
        form: &FormContext<'_>,
        errors: &mut Errors,
    ) -> impl std::future::Future<Output = Result> + Send {
        let _ = (form, errors);
        std::future::ready(Ok(()))
    }
}

/// `prepare`, `authorize` and `after` for a struct with
/// `#[derive(Validate)]`, which writes `rules` from its `#[validate(…)]`
/// attributes: add `#[validate(hooks)]` on the struct and implement the
/// ones you need.
///
/// ```
/// # use renox::prelude::*;
/// use renox::validation::{FormContext, ValidateHooks};
///
/// #[derive(serde::Deserialize, Validate)]
/// #[validate(hooks)]
/// struct Invite {
///     #[validate(required, email)]
///     email: String,
/// }
///
/// impl ValidateHooks for Invite {
///     fn prepare(&mut self) {
///         self.email = self.email.trim().to_lowercase();
///     }
///
///     async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {
///         Ok(form.user.is_some())
///     }
/// }
/// ```
pub trait ValidateHooks {
    fn prepare(&mut self) {}

    fn authorize(
        &self,
        form: &FormContext<'_>,
    ) -> impl std::future::Future<Output = Result<bool>> + Send {
        let _ = form;
        std::future::ready(Ok(true))
    }

    fn after(
        &self,
        form: &FormContext<'_>,
        errors: &mut Errors,
    ) -> impl std::future::Future<Output = Result> + Send {
        let _ = (form, errors);
        std::future::ready(Ok(()))
    }
}

/// What [`Validate::authorize`] and [`Validate::after`] see of the request.
#[non_exhaustive]
pub struct FormContext<'a> {
    pub state: &'a crate::AppState,
    /// The logged-in user, if any.
    pub user: Option<&'a crate::auth::User>,
    pub method: &'a axum::http::Method,
    pub path: &'a str,
}

struct Pending {
    field: String,
    label: String,
    table: String,
    column: String,
    value: DbValue,
    ignore_id: Option<crate::db::DbValue>,
    /// Extra conditions (`where_eq`, `where_null`, `where_not_null`).
    scope: Vec<ScopeCondition>,
    unique: bool,
    message: Option<String>,
}

/// A condition added to a `unique`/`exists` check.
enum ScopeCondition {
    Eq(String, DbValue),
    Null(String),
    NotNull(String),
}

impl ScopeCondition {
    fn column(&self) -> &str {
        match self {
            Self::Eq(column, _) | Self::Null(column) | Self::NotNull(column) => column,
        }
    }
}

/// SQLite reads an unknown double-quoted column as a string literal, which
/// would make `unique("t", "typo")` always pass; refuse unknown columns.
async fn ensure_sqlite_column(db: &Db, table: &str, column: &str) -> Result {
    let found: i64 = crate::db::sql("SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?")
        .bind(table)
        .bind(column)
        .scalar(db)
        .await?;
    if found == 0 {
        return Err(anyhow::anyhow!(
            "unique/exists rule: table `{table}` has no column `{column}`"
        )
        .into());
    }
    Ok(())
}

/// Collects rule failures. Rules run in order and stop at a field's first
/// failure; rules other than `required` and `accepted` skip empty values.
pub struct Validator {
    locale: Locale,
    /// The request language's lang file, for overridden messages and labels.
    texts: Option<crate::i18n::Texts>,
    errors: Errors,
    pending: Vec<Pending>,
}

impl Validator {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            texts: None,
            errors: Errors::new(),
            pending: Vec::new(),
        }
    }

    /// Uses the app's translations of messages (`renox.validation.*`) and
    /// field names (`renox.validation.attributes.*`).
    pub(crate) fn with_texts(mut self, texts: crate::i18n::Texts) -> Self {
        self.texts = Some(texts);
        self
    }

    fn template(&self, key: &str) -> std::borrow::Cow<'static, str> {
        messages::template_for(self.locale, self.texts.as_ref(), key)
    }

    /// A field's name for messages: the app's translation
    /// (`renox.validation.attributes.{name}`) or the name with spaces.
    fn translated_label(&self, name: &str) -> Option<String> {
        self.texts.as_ref().and_then(|t| {
            t.get(&format!("renox.validation.attributes.{name}"))
                .cloned()
        })
    }

    fn label_for(&self, name: &str) -> String {
        self.translated_label(name)
            .unwrap_or_else(|| name.replace('_', " "))
    }

    /// Rules for each item of a list, e.g. every tag or every uploaded
    /// photo; errors are keyed `name.0`, `name.1`, … and labelled
    /// "`name` #1", "#2", …
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # struct Form { tags: Vec<String>, photos: Vec<Upload> }
    /// # impl Validate for Form {
    /// fn rules(&self, v: &mut Validator) {
    ///     v.field("tags", &self.tags).max(5);
    ///     v.each("tags", &self.tags, |tag| tag.required().max(20));
    ///     v.each("photos", &self.photos, |photo| photo.image().max(2048));
    /// }
    /// # }
    /// ```
    pub fn each<T: FieldValue>(
        &mut self,
        name: &str,
        items: &[T],
        rules: impl for<'a> Fn(Field<'a>) -> Field<'a>,
    ) {
        let base = self.label_for(name);
        for (i, item) in items.iter().enumerate() {
            let key = format!("{name}.{i}");
            let label = format!("{base} #{}", i + 1);
            rules(self.field(&key, item).label(&label));
        }
    }

    /// Each item's own `Validate` rules, for a list of structs (e.g. the
    /// lines of an order sent as JSON); errors are keyed `name.0.field`.
    pub fn nested<T: Validate>(&mut self, name: &str, items: &[T]) {
        for (i, item) in items.iter().enumerate() {
            let mut inner = Validator {
                locale: self.locale,
                texts: self.texts.clone(),
                errors: Errors::new(),
                pending: Vec::new(),
            };
            item.rules(&mut inner);
            for (field, messages) in inner.errors.iter() {
                for message in messages {
                    self.errors
                        .add(format!("{name}.{i}.{field}"), message.clone());
                }
            }
            for mut pending in inner.pending {
                pending.field = format!("{name}.{i}.{}", pending.field);
                self.pending.push(pending);
            }
        }
    }

    /// No two items of a list are the same (Laravel's `distinct`), e.g. the
    /// emails invited at once; each repeat gets the error, keyed `name.i`.
    pub fn distinct<T: FieldValue>(&mut self, name: &str, items: &[T]) {
        let base = self.label_for(name);
        let template = self.template("distinct");
        let mut seen: Vec<Inspected> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let value = match item.inspect() {
                Inspected::Text(text) => Inspected::Text(text.trim().to_lowercase()),
                other => other,
            };
            if value == Inspected::Missing {
                continue;
            }
            if seen.contains(&value) {
                let label = format!("{base} #{}", i + 1);
                self.errors
                    .add(format!("{name}.{i}"), render(&template, &label, &[]));
            } else {
                seen.push(value);
            }
        }
    }

    /// Starts the rules for one field. The label in messages defaults to the
    /// name with `_` replaced by spaces.
    pub fn field<'v>(&'v mut self, name: &str, value: &impl FieldValue) -> Field<'v> {
        let translated = self.translated_label(name);
        let mut field = Field {
            translated: translated.is_some(),
            label: translated.unwrap_or_else(|| name.replace('_', " ")),
            name: name.to_owned(),
            value: value.inspect(),
            db_value: value.db_value(),
            failed: false,
            last_pending: None,
            v: self,
        };
        // `NaN`, `inf` and `1e999` parse as floats, but aren't numbers anyone entered.
        if let Inspected::Number(n) = field.value
            && !n.is_finite()
        {
            field.fail("numeric", &[]);
        }
        field
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
            let dialect = db.dialect();
            if dialect == crate::db::Dialect::Sqlite {
                ensure_sqlite_column(db, &check.table, &check.column).await?;
                for condition in &check.scope {
                    ensure_sqlite_column(db, &check.table, condition.column()).await?;
                }
            }
            // Form input is text; PostgreSQL won't compare text with a number
            // column, so compare as text (a no-op for text columns).
            let column = match (&check.value, dialect) {
                (DbValue::Text(_), crate::db::Dialect::Postgres) => {
                    format!("CAST({} AS TEXT)", quote(&check.column))
                }
                _ => quote(&check.column),
            };
            let mut sql = format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE {column} = ?",
                quote(&check.table),
            );
            if check.ignore_id.is_some() {
                sql.push_str(" AND \"id\" != ?");
            }
            let mut scope_values = Vec::new();
            for condition in check.scope {
                match condition {
                    ScopeCondition::Eq(column, value) => {
                        sql.push_str(&format!(" AND {} = ?", quote(&column)));
                        scope_values.push(value);
                    }
                    ScopeCondition::Null(column) => {
                        sql.push_str(&format!(" AND {} IS NULL", quote(&column)));
                    }
                    ScopeCondition::NotNull(column) => {
                        sql.push_str(&format!(" AND {} IS NOT NULL", quote(&column)));
                    }
                }
            }
            sql.push(')');
            let mut query = crate::db::sql(sql).bind(check.value);
            if let Some(id) = check.ignore_id {
                query = query.bind(id);
            }
            let query = query.bind_all(scope_values);
            let found: bool = query.scalar(db).await?;
            if found == check.unique {
                let key = if check.unique { "unique" } else { "exists" };
                let message = check.message.unwrap_or_else(|| {
                    render(
                        &messages::template_for(self.locale, self.texts.as_ref(), key),
                        &check.label,
                        &[],
                    )
                });
                errors.add(check.field, message);
            }
        }
        Ok(errors)
    }

    /// Like `rules_of`, with the app's translations of messages and labels.
    pub(crate) fn rules_with_texts(
        data: &impl Validate,
        locale: Locale,
        texts: crate::i18n::Texts,
    ) -> Self {
        let mut validator = Self::new(locale).with_texts(texts);
        data.rules(&mut validator);
        validator
    }

    /// Applies `data`'s rules; call `finish` to run the database checks.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use renox::validation::Locale;
    /// # #[derive(serde::Deserialize)] struct ProdukForm { nama: String }
    /// # impl Validate for ProdukForm { fn rules(&self, v: &mut Validator) { v.field("nama", &self.nama).required(); } }
    /// # async fn demo(form: ProdukForm, db: Db) -> Result {
    /// let errors = Validator::rules_of(&form, Locale::Id).finish(&db).await?;
    /// # let _ = errors; Ok(()) }
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
    /// The label came from the app's lang file.
    translated: bool,
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

    /// Uses `label` unless the app's lang file names this field.
    pub(crate) fn fallback_label(mut self, label: &str) -> Self {
        if !self.translated {
            self.label = label.to_owned();
        }
        self
    }

    fn fail(&mut self, key: &str, params: &[(&str, String)]) {
        if !self.failed {
            let message = render(&self.v.template(key), &self.label, params);
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
            Inspected::File { kilobytes, .. } => (*kilobytes, "file"),
            _ => return self,
        };
        if !ok(size) {
            self.fail(&format!("{kind}.{suffix}"), params);
        }
        self
    }

    /// At least `min` characters, items, kilobytes (files), or as a number.
    pub fn min(self, min: impl Into<f64>) -> Self {
        let min = min.into();
        self.size_rule("min", |s| s >= min, &[("min", number(min))])
    }

    /// At most `max` characters, items, kilobytes (files), or as a number.
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

    /// An uploaded image: PNG, JPEG, GIF or WebP, checked from the file's
    /// content rather than its name.
    pub fn image(mut self) -> Self {
        if let (true, Inspected::File { image, .. }) = (self.present(), &self.value)
            && !*image
        {
            self.fail("image", &[]);
        }
        self
    }

    /// An uploaded file of one of these types, e.g. `&["jpg", "png", "pdf"]`
    /// (`jpeg` counts as `jpg`). The content decides for the formats Renox can
    /// recognise, the file name for the rest.
    pub fn mimes(mut self, extensions: &[&str]) -> Self {
        if let (true, Inspected::File { extension, .. }) = (self.present(), &self.value) {
            let normalise = |e: &str| match e.to_ascii_lowercase().as_str() {
                "jpeg" => "jpg".to_owned(),
                other => other.to_owned(),
            };
            let ok = extensions
                .iter()
                .any(|e| normalise(e) == normalise(extension));
            if !ok {
                self.fail("mimes", &[("values", extensions.join(", "))]);
            }
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

    /// The whole text matches `pattern` (a regular expression; anchor it
    /// with `^…$` to match all of it), e.g. `.matches(r"^[A-Z]{2}\d{4}$")`.
    pub fn matches(mut self, pattern: &str) -> Self {
        if let (true, Inspected::Text(text)) = (self.present(), &self.value) {
            let ok = match cached_regex(pattern) {
                Ok(regex) => regex.is_match(text),
                Err(err) => {
                    tracing::error!(pattern, error = %err, "invalid pattern in a `matches` rule");
                    false
                }
            };
            if !ok {
                self.fail("regex", &[]);
            }
        }
        self
    }

    /// Exactly `n` digits (and nothing else), e.g. a PIN.
    pub fn digits(mut self, n: usize) -> Self {
        if self.present() && digit_count(&self.value) != Some(n) {
            self.fail("digits", &[("digits", n.to_string())]);
        }
        self
    }

    /// Between `min` and `max` digits (and nothing else), e.g. a phone number.
    pub fn digits_between(mut self, min: usize, max: usize) -> Self {
        if self.present() && !digit_count(&self.value).is_some_and(|n| n >= min && n <= max) {
            self.fail(
                "digits_between",
                &[("min", min.to_string()), ("max", max.to_string())],
            );
        }
        self
    }

    /// A date (`2026-10-01`) or a date and time (`2026-10-01T10:30`).
    pub fn date(mut self) -> Self {
        if self.present() && self.as_date().is_none() {
            self.fail("date", &[]);
        }
        self
    }

    fn date_rule(
        mut self,
        key: &str,
        limit: NaiveDateTime,
        ok: impl Fn(NaiveDateTime, NaiveDateTime) -> bool,
    ) -> Self {
        if !self.present() {
            return self;
        }
        match self.as_date() {
            None => self.fail("date", &[]),
            Some(date) if !ok(date, limit) => {
                let shown = if limit.time() == chrono::NaiveTime::MIN {
                    limit.date().to_string()
                } else {
                    limit.format("%Y-%m-%d %H:%M").to_string()
                };
                self.fail(key, &[("date", shown)]);
            }
            Some(_) => {}
        }
        self
    }

    /// A date before `limit` (a `NaiveDate`, `NaiveDateTime` or `DateTime`),
    /// e.g. `.before(today)` for a birth date.
    pub fn before(self, limit: impl FieldValue) -> Self {
        match limit.inspect() {
            Inspected::Date(limit) => self.date_rule("before", limit, |d, l| d < l),
            _ => self,
        }
    }

    pub fn before_or_equal(self, limit: impl FieldValue) -> Self {
        match limit.inspect() {
            Inspected::Date(limit) => self.date_rule("before_or_equal", limit, |d, l| d <= l),
            _ => self,
        }
    }

    /// A date after `limit`, e.g. `.after(self.start)` for an end date.
    pub fn after(self, limit: impl FieldValue) -> Self {
        match limit.inspect() {
            Inspected::Date(limit) => self.date_rule("after", limit, |d, l| d > l),
            _ => self,
        }
    }

    pub fn after_or_equal(self, limit: impl FieldValue) -> Self {
        match limit.inspect() {
            Inspected::Date(limit) => self.date_rule("after_or_equal", limit, |d, l| d >= l),
            _ => self,
        }
    }

    fn as_date(&self) -> Option<NaiveDateTime> {
        match &self.value {
            Inspected::Date(date) => Some(*date),
            Inspected::Text(text) => parse_date(text.trim()),
            _ => None,
        }
    }

    /// None of the given values (Laravel's `not_in`).
    pub fn none_of<V: FieldValue>(mut self, refused: &[V]) -> Self {
        if self.present() && refused.iter().any(|r| r.inspect() == self.value) {
            self.fail("not_in", &[]);
        }
        self
    }

    fn text_rule(
        mut self,
        key: &str,
        ok: impl Fn(&str) -> bool,
        params: &[(&str, String)],
    ) -> Self {
        if let (true, Inspected::Text(text)) = (self.present(), &self.value)
            && !ok(text)
        {
            self.fail(key, params);
        }
        self
    }

    /// Letters only (any language's: `é`, `ü`, `ß` count).
    pub fn alpha(self) -> Self {
        self.text_rule("alpha", |t| t.chars().all(char::is_alphabetic), &[])
    }

    /// Letters and digits only.
    pub fn alpha_num(self) -> Self {
        self.text_rule("alpha_num", |t| t.chars().all(char::is_alphanumeric), &[])
    }

    /// Letters, digits, `-` and `_`, e.g. a username or a slug.
    pub fn alpha_dash(self) -> Self {
        self.text_rule(
            "alpha_dash",
            |t| {
                t.chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            },
            &[],
        )
    }

    /// No uppercase letters.
    pub fn lowercase(self) -> Self {
        self.text_rule("lowercase", |t| !t.chars().any(char::is_uppercase), &[])
    }

    /// No lowercase letters.
    pub fn uppercase(self) -> Self {
        self.text_rule("uppercase", |t| !t.chars().any(char::is_lowercase), &[])
    }

    /// Starts with one of `prefixes`, e.g. `.starts_with(&["08", "+62"])`.
    pub fn starts_with(self, prefixes: &[&str]) -> Self {
        let values = prefixes.join(", ");
        self.text_rule(
            "starts_with",
            |t| prefixes.iter().any(|p| t.starts_with(p)),
            &[("values", values)],
        )
    }

    /// Ends with one of `suffixes`, e.g. `.ends_with(&["@company.com"])`.
    pub fn ends_with(self, suffixes: &[&str]) -> Self {
        let values = suffixes.join(", ");
        self.text_rule(
            "ends_with",
            |t| suffixes.iter().any(|s| t.ends_with(s)),
            &[("values", values)],
        )
    }

    /// A UUID (`8-4-4-4-12` hexadecimal digits).
    pub fn uuid(self) -> Self {
        self.text_rule("uuid", is_uuid, &[])
    }

    /// An IPv4 or IPv6 address.
    pub fn ip(self) -> Self {
        self.text_rule("ip", |t| t.trim().parse::<std::net::IpAddr>().is_ok(), &[])
    }

    /// Exactly `size` characters, items, kilobytes (files), or equal to it as
    /// a number (Laravel's `size`).
    pub fn size(self, size: impl Into<f64>) -> Self {
        let size = size.into();
        self.size_rule("size", |s| s == size, &[("size", number(size))])
    }

    /// Required when `other` is empty, e.g. an email when there's no phone.
    pub fn required_without(self, other: &impl FieldValue) -> Self {
        let missing = other.inspect() == Inspected::Missing;
        self.required_if(missing)
    }

    /// Must be empty when `condition` holds (Laravel's `prohibited_if`), e.g.
    /// no discount code on a gift card order.
    pub fn prohibited_if(mut self, condition: bool) -> Self {
        if condition && !self.failed && self.value != Inspected::Missing {
            self.fail("prohibited", &[]);
        }
        self
    }

    /// Required when `condition` holds, e.g.
    /// `.required_if(self.kind == "company")` for a company name.
    pub fn required_if(self, condition: bool) -> Self {
        if condition { self.required() } else { self }
    }

    /// Required unless `condition` holds.
    pub fn required_unless(self, condition: bool) -> Self {
        self.required_if(!condition)
    }

    /// Required when `other` has a value, e.g. a phone number's country
    /// code when a phone number is given.
    pub fn required_with(self, other: &impl FieldValue) -> Self {
        let given = other.inspect() != Inspected::Missing;
        self.required_if(given)
    }

    /// Equal to another field, named `other` in the message.
    pub fn same(mut self, other: &str, value: &impl FieldValue) -> Self {
        if self.present() && value.inspect() != self.value {
            let other = self.v.label_for(other);
            self.fail("same", &[("other", other)]);
        }
        self
    }

    /// Different from another field, named `other` in the message.
    pub fn different(mut self, other: &str, value: &impl FieldValue) -> Self {
        if self.present() && value.inspect() == self.value {
            let other = self.v.label_for(other);
            self.fail("different", &[("other", other)]);
        }
        self
    }

    /// The value meets `policy` (length, letters, mixed case, numbers,
    /// symbols); see [`Password`].
    pub fn password(mut self, policy: &Password) -> Self {
        if let (true, Inspected::Text(text)) = (self.present(), &self.value)
            && let Some((key, params)) = policy.broken(text)
        {
            self.fail(key, &params);
        }
        self
    }

    /// A reusable rule; see [`Rule`].
    pub fn apply(mut self, rule: &impl Rule) -> Self {
        if !self.present() {
            return self;
        }
        if let Err(message) = rule.check(&self.value) {
            let message = render(&message, &self.label, &[]);
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
                scope: Vec::new(),
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

    /// Skips the row with this id in the preceding `unique`, for updates:
    /// `.unique("products", "sku").ignore(product.id)`, whatever the key's
    /// type (`i64`, `Ulid`, `Uuid`, `String`).
    pub fn ignore(self, id: impl crate::db::ToDbValue) -> Self {
        if let Some(i) = self.last_pending {
            self.v.pending[i].ignore_id = Some(id.to_db_value());
        }
        self
    }

    /// Some row in `table` has this value in `column`.
    pub fn exists(self, table: &str, column: &str) -> Self {
        self.database(table, column, false)
    }

    /// Only rows where `column = value` count for the preceding `unique` or
    /// `exists`, e.g. the current team's: SKUs are unique per team.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # struct ProductForm { id: i64, sku: String, category_id: i64, team_id: i64 }
    /// # impl Validate for ProductForm {
    /// fn rules(&self, v: &mut Validator) {
    ///     v.field("sku", &self.sku)
    ///         .unique("products", "sku")
    ///         .ignore(self.id)
    ///         .where_eq("team_id", self.team_id)
    ///         .where_null("deleted_at"); // soft-deleted rows don't count
    ///     v.field("category_id", &self.category_id)
    ///         .exists("categories", "id")
    ///         .where_eq("team_id", self.team_id); // not another team's category
    /// }
    /// # }
    /// ```
    pub fn where_eq(self, column: &str, value: impl ToDbValue) -> Self {
        self.scope(ScopeCondition::Eq(column.to_owned(), value.to_db_value()))
    }

    /// Only rows where `column` is null count (e.g. `deleted_at`).
    pub fn where_null(self, column: &str) -> Self {
        self.scope(ScopeCondition::Null(column.to_owned()))
    }

    /// Only rows where `column` is not null count.
    pub fn where_not_null(self, column: &str) -> Self {
        self.scope(ScopeCondition::NotNull(column.to_owned()))
    }

    fn scope(self, condition: ScopeCondition) -> Self {
        if let Some(i) = self.last_pending {
            self.v.pending[i].scope.push(condition);
        }
        self
    }
}

/// What a password must contain; `Field::password(&policy)` checks it.
/// The built-in register, reset and account forms use the app's policy
/// (`Auth::password_rules`), `Password::min(8)` by default.
///
/// ```
/// # use renox::prelude::*;
/// use renox::validation::Password;
/// # struct Form { password: String }
/// # impl Validate for Form {
/// fn rules(&self, v: &mut Validator) {
///     let policy = Password::min(12).mixed_case().numbers().symbols();
///     v.field("password", &self.password).required().password(&policy);
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Password {
    min: usize,
    letters: bool,
    mixed_case: bool,
    numbers: bool,
    symbols: bool,
}

impl Password {
    /// At least `min` characters.
    pub fn min(min: usize) -> Self {
        Self {
            min,
            letters: false,
            mixed_case: false,
            numbers: false,
            symbols: false,
        }
    }

    /// At least one letter.
    pub fn letters(mut self) -> Self {
        self.letters = true;
        self
    }

    /// At least one uppercase and one lowercase letter.
    pub fn mixed_case(mut self) -> Self {
        self.mixed_case = true;
        self
    }

    /// At least one digit.
    pub fn numbers(mut self) -> Self {
        self.numbers = true;
        self
    }

    /// At least one character that isn't a letter, digit or space.
    pub fn symbols(mut self) -> Self {
        self.symbols = true;
        self
    }

    /// The first rule `password` breaks: a message key and its parameters.
    fn broken(&self, password: &str) -> Option<(&'static str, Vec<(&'static str, String)>)> {
        if password.chars().count() < self.min {
            return Some(("min.string", vec![("min", self.min.to_string())]));
        }
        let has = |test: fn(&char) -> bool| password.chars().any(|c| test(&c));
        if self.letters && !has(|c| c.is_alphabetic()) {
            return Some(("password.letters", Vec::new()));
        }
        if self.mixed_case && !(has(|c| c.is_uppercase()) && has(|c| c.is_lowercase())) {
            return Some(("password.mixed", Vec::new()));
        }
        if self.numbers && !has(|c| c.is_numeric()) {
            return Some(("password.numbers", Vec::new()));
        }
        if self.symbols && !has(|c| !c.is_alphanumeric() && !c.is_whitespace()) {
            return Some(("password.symbols", Vec::new()));
        }
        None
    }
}

impl Default for Password {
    fn default() -> Self {
        Self::min(8)
    }
}

/// A rule to reuse across forms, e.g. an Indonesian tax number:
///
/// ```
/// # use renox::prelude::*;
/// use renox::validation::{Inspected, Rule};
///
/// struct Npwp;
///
/// impl Rule for Npwp {
///     fn check(&self, value: &Inspected) -> std::result::Result<(), String> {
///         let Inspected::Text(text) = value else { return Ok(()) };
///         let digits = text.chars().filter(char::is_ascii_digit).count();
///         if digits == 15 || digits == 16 {
///             Ok(())
///         } else {
///             Err("The :attribute must be a valid NPWP.".into()) // :attribute is the field's label
///         }
///     }
/// }
///
/// # struct Form { npwp: String }
/// # impl Validate for Form {
/// fn rules(&self, v: &mut Validator) {
///     v.field("npwp", &self.npwp).required().apply(&Npwp);
/// }
/// # }
/// ```
///
/// Missing values skip the rule (combine it with `required`).
pub trait Rule {
    /// `Err(message)` when `value` breaks the rule.
    fn check(&self, value: &Inspected) -> std::result::Result<(), String>;
}

fn is_uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.trim().split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Compiled patterns, so a rule in a hot form doesn't recompile each time.
fn cached_regex(pattern: &str) -> std::result::Result<regex::Regex, regex::Error> {
    static CACHE: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<String, regex::Regex>>,
    > = std::sync::LazyLock::new(Default::default);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(regex) = cache.get(pattern) {
        return Ok(regex.clone());
    }
    let regex = regex::Regex::new(pattern)?;
    if cache.len() < 1000 {
        cache.insert(pattern.to_owned(), regex.clone());
    }
    Ok(regex)
}

/// How many digits the value is made of, if it's only digits.
fn digit_count(value: &Inspected) -> Option<usize> {
    let text = match value {
        Inspected::Text(text) => text.trim().to_owned(),
        Inspected::Number(n) if n.fract() == 0.0 && *n >= 0.0 => format!("{}", *n as u64),
        _ => return None,
    };
    (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit())).then_some(text.len())
}

/// `2026-10-01`, `2026-10-01T10:30`, `2026-10-01 10:30:00` or RFC 3339.
fn parse_date(text: &str) -> Option<NaiveDateTime> {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(date.and_time(chrono::NaiveTime::MIN));
    }
    for format in [
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(date) = NaiveDateTime::parse_from_str(text, format) {
            return Some(date);
        }
    }
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|d| d.naive_utc())
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
/// ```
/// # use renox::prelude::*;
/// # #[derive(serde::Serialize)] struct StokForm { jumlah: i64 }
/// # fn demo(form: StokForm) -> Result {
/// let mut errors = Errors::new();
/// errors.add("stok", "Stok tidak cukup.");
/// return Err(ValidationError::new(errors).with_input(&form).into());
/// # }
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

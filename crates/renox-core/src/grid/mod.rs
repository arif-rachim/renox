//! Data grids: a table that fills the screen, with server-side filters,
//! sorting and pagination, columns each user picks per screen size, frozen
//! columns, grouped headers and cells of any kind.
//!
//! A grid is defined once in Rust and rendered with the `grid` macro of
//! `renox/grid.html`:
//!
//! ```
//! use renox::prelude::*;
//! use renox::grid::{Column, Grid, GridRequest};
//!
//! #[derive(Model, serde::Serialize, Default)]
//! struct Order { id: i64, number: String, customer: String, status: String, total: i64 }
//!
//! fn orders() -> Grid {
//!     Grid::new("orders")
//!         .title("Orders")
//!         .column(Column::text("number", "Order").frozen().mobile())
//!         .column(Column::text("customer", "Customer").mobile())
//!         .column(Column::select("status", "Status", [("new", "New"), ("paid", "Paid")]).mobile())
//!         .column(Column::money("total", "Total").under(["Amounts"]))
//!         .sort_by("-id")
//! }
//!
//! async fn index(request: GridRequest) -> Result<View> {
//!     let page = orders().page(Order::query(), &request).await?;
//!     Ok(view("orders/index.html", context! { orders => page }))
//! }
//! ```
//!
//! ```jinja
//! {% from "renox/grid.html" import grid %}
//! <main class="rx-grid-fill">{{ grid(orders) }}</main>
//! ```
//!
//! Filters, sorting and pages travel in the query string (`q.number=A%`,
//! `min.total=1000`, `from.ordered_on=2026-01-01`, `in.status=paid`,
//! `sort=-total`, `page=2`, `per_page=50`), so a filtered page has a link of
//! its own. Only columns the grid defines are filtered or sorted, and the
//! query builder checks the names again. Columns of kind `custom` are drawn
//! by the template: `{% call(row, column) grid(orders) %}…{% endcall %}`.
//!
//! What a user picks in the column menu (visible columns on small and wide
//! screens, order, frozen columns) is kept per user in `grid_preferences`,
//! or in the session for guests.

use std::collections::BTreeMap;

use axum::Router;
use axum::extract::{FromRequestParts, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

mod export;

pub use export::MAX_EXPORT_ROWS;

use crate::auth::AuthUser;
use crate::db::{Db, Model, Paginated, Query};
use crate::{AppState, Error, Result, Session};

/// The framework migration creating `grid_preferences` (every app gets it).
pub(crate) const MIGRATION: crate::db::Migration =
    crate::db::framework_migration!("grid", "00010101000220_create_grid_preferences_table");

/// Rows per page a user may pick when the grid doesn't say.
const PER_PAGE_OPTIONS: [u32; 4] = [10, 25, 50, 100];

/// What a column holds, which decides how its cells look and how it filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Kind {
    /// Text; filtered with "contains", "starts with", "ends with", "equals",
    /// or a pattern with `%`.
    Text,
    /// A number, right-aligned; filtered with a minimum and a maximum.
    Number,
    /// An amount in the smallest unit (rupiah, cents), with thousands
    /// separators; filtered like a number.
    Money,
    /// A date (`NaiveDate`); filtered with a date range.
    Date,
    /// A moment (`DateTime`); filtered with a date range.
    DateTime,
    /// Yes or no; filtered with a choice.
    Bool,
    /// One of a few values; filtered by picking some of them.
    Select,
    /// Several of a few values, stored as a JSON array of strings
    /// (`Json<Vec<String>>`); filtered by rows having any of the picked ones.
    Tags,
    /// Drawn by the page's template (`caller(row, column)`): charts, buttons,
    /// links. Not sorted or filtered.
    Custom,
}

/// Which edge a frozen column sticks to while the grid scrolls sideways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pin {
    /// The left edge.
    Left,
    /// The right edge.
    Right,
}

/// One column of a [`Grid`]; build it with a constructor per [`Kind`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Column {
    key: String,
    label: String,
    kind: Kind,
    under: Vec<String>,
    sortable: bool,
    filterable: bool,
    mobile: bool,
    hidden: bool,
    pin: Option<Pin>,
    decimals: Option<u8>,
    options: Vec<(String, String)>,
    width: Option<String>,
    editable: bool,
    merge: bool,
}

impl Column {
    fn new(key: &str, label: &str, kind: Kind) -> Self {
        let plain = kind != Kind::Custom;
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
            kind,
            under: Vec::new(),
            sortable: plain && kind != Kind::Tags,
            filterable: plain,
            mobile: false,
            hidden: false,
            pin: None,
            decimals: None,
            options: Vec::new(),
            width: None,
            editable: false,
            merge: false,
        }
    }

    /// A text column; `key` is the model's column.
    pub fn text(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Text)
    }

    /// A number column (`.decimals(n)` for a fixed number of decimals).
    pub fn number(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Number)
    }

    /// An amount in the smallest unit, shown with thousands separators.
    pub fn money(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Money)
    }

    /// A date column.
    pub fn date(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Date)
    }

    /// A date and time column (shown in `APP_TIMEZONE`).
    pub fn datetime(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::DateTime)
    }

    /// A yes/no column.
    pub fn bool(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Bool)
    }

    /// One of `options` (stored value, label shown).
    pub fn select<V: Into<String>, L: Into<String>>(
        key: &str,
        label: &str,
        options: impl IntoIterator<Item = (V, L)>,
    ) -> Self {
        let mut column = Self::new(key, label, Kind::Select);
        column.options = options
            .into_iter()
            .map(|(v, l)| (v.into(), l.into()))
            .collect();
        column
    }

    /// Several of `options`, stored as a JSON array of the values.
    pub fn tags<V: Into<String>, L: Into<String>>(
        key: &str,
        label: &str,
        options: impl IntoIterator<Item = (V, L)>,
    ) -> Self {
        let mut column = Self::select(key, label, options);
        column.kind = Kind::Tags;
        column
    }

    /// A column the page's template draws (`{% call(row, column) grid(…) %}`).
    /// `key` needn't be a database column.
    pub fn custom(key: &str, label: &str) -> Self {
        Self::new(key, label, Kind::Custom)
    }

    /// Puts the column under grouped headings, outermost first:
    /// `.under(["Amounts"])`, `.under(["Sales", "Q1"])`. Neighbouring
    /// columns under the same headings share them.
    pub fn under<S: Into<String>>(mut self, groups: impl IntoIterator<Item = S>) -> Self {
        self.under = groups.into_iter().map(Into::into).collect();
        self
    }

    /// Whether a click on the heading sorts by this column (default: yes,
    /// except for tags and custom columns).
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable && self.kind != Kind::Custom;
        self
    }

    /// Whether the heading has a filter (default: yes, except for custom
    /// columns).
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable && self.kind != Kind::Custom;
        self
    }

    /// Shown on small screens until the user picks otherwise. When no column
    /// says so, the first three are.
    pub fn mobile(mut self) -> Self {
        self.mobile = true;
        self
    }

    /// Hidden on wide screens too until the user turns it on.
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Frozen at the left edge until the user picks otherwise.
    pub fn frozen(mut self) -> Self {
        self.pin = Some(Pin::Left);
        self
    }

    /// Frozen at the right edge (e.g. a column of actions).
    pub fn frozen_right(mut self) -> Self {
        self.pin = Some(Pin::Right);
        self
    }

    /// Shows a number with exactly `decimals` decimals.
    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// A minimum width in CSS units, e.g. `"14rem"`.
    pub fn width(mut self, width: &str) -> Self {
        self.width = Some(width.to_owned());
        self
    }

    /// Cells can be edited in place (double-click, Enter or F2; Enter saves,
    /// Escape cancels) and in the row's edit mode. Saving sends the value
    /// to the grid's [`Grid::edit_url`]. Not for custom columns, nor for
    /// merged ones.
    pub fn editable(mut self) -> Self {
        self.editable = self.kind != Kind::Custom;
        self
    }

    /// Neighbouring rows with the same value share one cell. Merged columns
    /// nest from left to right: a column merges only within the merged
    /// groups of the merged columns before it (Region, then City). Merging
    /// is off while rows are dragged into order.
    pub fn merge(mut self) -> Self {
        self.merge = true;
        self.editable = false;
        self
    }

    /// The column's key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The column's kind.
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

/// A data grid's definition: its columns and defaults. Cheap to build, so
/// a function returning it is the usual place for one.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Grid {
    id: String,
    title: Option<String>,
    columns: Vec<Column>,
    per_page: u32,
    per_page_options: Vec<u32>,
    sort: Option<String>,
    audit: bool,
    details: bool,
    edit_url: Option<String>,
    reorder: Option<(String, String)>,
    exports: bool,
}

impl Grid {
    /// A grid named `id` (letters, digits, `_` and `-`), which keys each
    /// user's preferences and the element's id.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            title: None,
            columns: Vec::new(),
            per_page: 25,
            per_page_options: PER_PAGE_OPTIONS.to_vec(),
            sort: None,
            audit: false,
            details: false,
            edit_url: None,
            reorder: None,
            exports: false,
        }
    }

    /// An export menu in the toolbar: CSV, Excel (with the `xlsx` feature)
    /// and a page to print or save as PDF, each of every row the filters
    /// match (not just the page), in the user's columns. The handler answers
    /// them with [`Grid::export`].
    pub fn exports(mut self) -> Self {
        self.exports = true;
        self
    }

    /// A click on a row (or its chevron) opens details under it: who
    /// created and last changed the row, and when, from its `created_by`,
    /// `created_at`, `updated_by` and `updated_at` values (those it has).
    pub fn audit(mut self) -> Self {
        self.audit = true;
        self
    }

    /// A click on a row opens details under it, drawn by the page's
    /// template: `caller(row, column)` with `column.key == "_details"`.
    /// With [`Grid::audit`], the audit fields come first.
    pub fn details(mut self) -> Self {
        self.details = true;
        self
    }

    /// Where edits are sent: `PATCH` to this URL with `{id}` replaced by the
    /// row's id, the changed fields as a form (`customer=…`, `tags=a&tags=b`,
    /// `paid=true`). Answer with a 2xx (a `Toast` shows), or with
    /// `Valid<T>`'s 422 and the errors show next to the fields; the grid
    /// then reloads its page.
    pub fn edit_url(mut self, url: &str) -> Self {
        self.edit_url = Some(url.to_owned());
        self
    }

    /// Rows can be dragged into order (or moved with the arrow keys on
    /// their handle) while the grid is sorted by `column` ascending; the
    /// new order of the page is `POST`ed to `url` as `ids` and `offset`
    /// (see [`RowOrder`]).
    pub fn reorder(mut self, column: &str, url: &str) -> Self {
        self.reorder = Some((column.to_owned(), url.to_owned()));
        self
    }

    /// The heading shown above the grid.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_owned());
        self
    }

    /// Adds a column.
    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    /// Rows per page until the user picks another number (default 25).
    pub fn per_page(mut self, per_page: u32) -> Self {
        self.per_page = per_page.clamp(1, 500);
        if !self.per_page_options.contains(&self.per_page) {
            self.per_page_options.push(self.per_page);
            self.per_page_options.sort_unstable();
        }
        self
    }

    /// The order before the user sorts: a column key, `-` first for
    /// descending (`"-created_at"`), or several separated by commas
    /// (`"region,city,-total"`, what merged columns want). Ties are broken
    /// by `id`.
    pub fn sort_by(mut self, sort: &str) -> Self {
        self.sort = Some(sort.to_owned());
        self
    }

    /// The grid's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The grid's columns, in their defined order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn find(&self, key: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.key == key)
    }

    /// One page of `query` with the request's filters, sort and page
    /// applied, ready for the `grid` macro.
    pub async fn page<M: Model + Serialize>(
        &self,
        query: Query<M>,
        request: &GridRequest,
    ) -> Result<GridPage<M>> {
        if !valid_key(&self.id) {
            return Err(Error::Internal(anyhow::anyhow!(
                "grid id `{}` may only have letters, digits, `_` and `-`",
                self.id
            )));
        }
        let prefs = load_prefs(request, &self.id).await;
        let state = State_::parse(self, &request.params);
        let query = self.filtered(query, &state);
        let query = self.sorted(query, &state);
        let rows = query
            .paginate(&request.db, state.page, state.per_page)
            .await?;
        Ok(GridPage {
            grid: self.clone(),
            prefs,
            state,
            path: request.path.clone(),
            rows,
            extra: Vec::new(),
        })
    }

    /// `query` with the request's filters applied (also used by `page`):
    /// for exports or totals over every filtered row.
    pub fn filter<M: Model>(&self, query: Query<M>, request: &GridRequest) -> Query<M> {
        let state = State_::parse(self, &request.params);
        let query = self.filtered(query, &state);
        self.sorted(query, &state)
    }

    fn filtered<M: Model>(&self, mut query: Query<M>, state: &State_) -> Query<M> {
        for (key, filter) in &state.filters {
            let Some(column) = self.find(key) else {
                continue;
            };
            query = filter.apply(query, column);
        }
        query
    }

    fn sorted<M: Model>(&self, mut query: Query<M>, state: &State_) -> Query<M> {
        let keys: Vec<(String, bool)> = if state.defaulted {
            self.default_sort()
        } else {
            state.sort.iter().cloned().collect()
        };
        for (key, desc) in &keys {
            query = if *desc {
                query.order_by_desc(key)
            } else {
                query.order_by(key)
            };
        }
        if keys.iter().any(|(key, _)| key == "id") {
            query
        } else {
            query.order_by("id")
        }
    }

    /// `sort_by`'s keys: `"region,city,-total"`.
    fn default_sort(&self) -> Vec<(String, bool)> {
        self.sort
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(|k| match k.strip_prefix('-') {
                Some(key) => (key.to_owned(), true),
                None => (k.to_owned(), false),
            })
            .collect()
    }

    /// The columns in the user's order: frozen-left ones first, then the
    /// others, then frozen-right ones, each with where it's pinned.
    fn ordered(&self, prefs: &GridPrefs) -> Vec<(&Column, Option<Pin>)> {
        let mut columns: Vec<&Column> = self.columns.iter().collect();
        if let Some(order) = &prefs.order {
            columns.sort_by_key(|c| order.iter().position(|k| *k == c.key).unwrap_or(usize::MAX));
        }
        let pin = |c: &Column| match (&prefs.left, &prefs.right) {
            (None, None) => c.pin,
            (left, right) => {
                if left.as_ref().is_some_and(|l| l.contains(&c.key)) {
                    Some(Pin::Left)
                } else if right.as_ref().is_some_and(|r| r.contains(&c.key)) {
                    Some(Pin::Right)
                } else {
                    None
                }
            }
        };
        let mut out: Vec<(&Column, Option<Pin>)> = Vec::new();
        for side in [Some(Pin::Left), None, Some(Pin::Right)] {
            out.extend(
                columns
                    .iter()
                    .filter(|c| pin(c) == side)
                    .map(|c| (*c, side)),
            );
        }
        out
    }

    fn default_visible(&self, compact: bool) -> Vec<String> {
        if compact {
            let flagged: Vec<String> = self
                .columns
                .iter()
                .filter(|c| c.mobile)
                .map(|c| c.key.clone())
                .collect();
            if !flagged.is_empty() {
                return flagged;
            }
            return self
                .columns
                .iter()
                .filter(|c| !c.hidden)
                .take(3)
                .map(|c| c.key.clone())
                .collect();
        }
        self.columns
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.key.clone())
            .collect()
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// What one column is filtered on, from the query string.
#[derive(Debug, Clone, Default, PartialEq)]
struct Filter {
    text: Option<String>,
    mode: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    any_of: Vec<String>,
}

impl Filter {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The `LIKE` pattern for a text filter.
    fn pattern(&self) -> Option<String> {
        let text = self.text.as_deref()?.trim();
        if text.is_empty() {
            return None;
        }
        if text.contains('%') {
            return Some(text.to_owned());
        }
        Some(match self.mode.as_deref() {
            Some("starts") => format!("{text}%"),
            Some("ends") => format!("%{text}"),
            Some("equals") => text.to_owned(),
            _ => format!("%{text}%"),
        })
    }

    fn apply<M: Model>(&self, mut query: Query<M>, column: &Column) -> Query<M> {
        let key = column.key.as_str();
        match column.kind {
            Kind::Text => {
                if let Some(pattern) = self.pattern() {
                    query = query.where_like(key, pattern);
                }
            }
            Kind::Number | Kind::Money => {
                for (value, op) in [(self.min, ">="), (self.max, "<=")] {
                    let Some(value) = value else { continue };
                    query = if value.fract() == 0.0 && value.abs() < 9e15 {
                        query.where_op(key, op, value as i64)
                    } else {
                        query.where_op(key, op, value)
                    };
                }
            }
            Kind::Date => {
                if let Some(from) = self.from {
                    query = query.where_op(key, ">=", from);
                }
                if let Some(to) = self
                    .to
                    .and_then(|d| d.checked_add_signed(Duration::days(1)))
                {
                    query = query.where_op(key, "<", to);
                }
            }
            Kind::DateTime => {
                let at = |d: NaiveDate| d.and_hms_opt(0, 0, 0).map(|t| t.and_utc());
                if let Some(from) = self.from.and_then(at) {
                    query = query.where_op(key, ">=", from);
                }
                if let Some(to) = self
                    .to
                    .and_then(|d| d.checked_add_signed(Duration::days(1)))
                    .and_then(at)
                {
                    query = query.where_op(key, "<", to);
                }
            }
            Kind::Bool => {
                let picked: Vec<bool> = self
                    .any_of
                    .iter()
                    .filter_map(|v| match v.as_str() {
                        "1" | "true" => Some(true),
                        "0" | "false" => Some(false),
                        _ => None,
                    })
                    .collect();
                if !picked.is_empty() {
                    query = query.where_in(key, picked);
                }
            }
            Kind::Select => {
                let picked: Vec<&String> = self
                    .any_of
                    .iter()
                    .filter(|v| column.options.iter().any(|(o, _)| o == *v))
                    .collect();
                if !picked.is_empty() {
                    query = query.where_in(key, picked.into_iter().cloned());
                }
            }
            Kind::Tags => {
                let picked: Vec<String> = self
                    .any_of
                    .iter()
                    .filter(|v| column.options.iter().any(|(o, _)| o == *v))
                    .map(|v| format!("%{}%", serde_json::to_string(v).unwrap_or_default()))
                    .collect();
                if !picked.is_empty() && M::COLUMNS.contains(&key) {
                    // The JSON text holds `"value"` for each picked one.
                    let sql =
                        vec![format!("CAST(\"{key}\" AS TEXT) LIKE ?"); picked.len()].join(" OR ");
                    query = query.where_raw(&sql, picked);
                }
            }
            Kind::Custom => {}
        }
        query
    }

    fn to_value(&self) -> Value {
        json!({
            "text": self.text,
            "mode": self.mode.clone().unwrap_or_else(|| "contains".into()),
            "min": self.min,
            "max": self.max,
            "from": self.from.map(|d| d.to_string()),
            "to": self.to.map(|d| d.to_string()),
            "any_of": self.any_of,
        })
    }
}

/// The request's filters, sort and page, checked against the grid.
#[derive(Debug, Clone)]
struct State_ {
    filters: BTreeMap<String, Filter>,
    sort: Option<(String, bool)>,
    /// `sort` is the grid's default, not in the query string.
    defaulted: bool,
    page: u32,
    per_page: u32,
}

impl State_ {
    fn parse(grid: &Grid, params: &[(String, String)]) -> Self {
        let mut filters: BTreeMap<String, Filter> = BTreeMap::new();
        let mut sort = None;
        let mut page = 1;
        let mut per_page = grid.per_page;
        for (name, value) in params {
            let value = value.trim();
            match name.as_str() {
                "page" => page = value.parse().unwrap_or(1).max(1),
                "per_page" => {
                    if let Ok(n) = value.parse::<u32>()
                        && grid.per_page_options.contains(&n)
                    {
                        per_page = n;
                    }
                }
                "sort" => {
                    let (key, desc) = match value.strip_prefix('-') {
                        Some(key) => (key, true),
                        None => (value, false),
                    };
                    if grid.find(key).is_some_and(|c| c.sortable) || key == "id" {
                        sort = Some((key.to_owned(), desc));
                    }
                }
                _ => {
                    let Some((part, key)) = name.split_once('.') else {
                        continue;
                    };
                    let Some(column) = grid.find(key).filter(|c| c.filterable) else {
                        continue;
                    };
                    if value.is_empty() {
                        continue;
                    }
                    let filter = filters.entry(column.key.clone()).or_default();
                    match part {
                        "q" => filter.text = Some(value.chars().take(200).collect()),
                        "m" if ["contains", "starts", "ends", "equals"].contains(&value) => {
                            filter.mode = Some(value.to_owned())
                        }
                        "min" => filter.min = value.parse().ok().filter(|v: &f64| v.is_finite()),
                        "max" => filter.max = value.parse().ok().filter(|v: &f64| v.is_finite()),
                        "from" => filter.from = value.parse().ok(),
                        "to" => filter.to = value.parse().ok(),
                        "in" if filter.any_of.len() < 100 => filter.any_of.push(value.to_owned()),
                        _ => {}
                    }
                }
            }
        }
        // A mode alone filters nothing.
        filters.retain(|_, f| {
            !Filter {
                mode: None,
                ..f.clone()
            }
            .is_empty()
        });
        // Without a sort of the user's, the grid's (its first key marks the
        // heading).
        let defaulted = sort.is_none();
        if defaulted {
            sort = grid.default_sort().into_iter().next();
        }
        Self {
            filters,
            sort,
            defaulted,
            page,
            per_page,
        }
    }

    /// The query string for this state, without `page` (links set it).
    fn query_string(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (key, f) in &self.filters {
            if let Some(text) = &f.text {
                out.push((format!("q.{key}"), text.clone()));
            }
            if let Some(mode) = &f.mode {
                out.push((format!("m.{key}"), mode.clone()));
            }
            for (name, value) in [("min", f.min), ("max", f.max)] {
                if let Some(v) = value {
                    out.push((format!("{name}.{key}"), v.to_string()));
                }
            }
            for (name, value) in [("from", f.from), ("to", f.to)] {
                if let Some(v) = value {
                    out.push((format!("{name}.{key}"), v.to_string()));
                }
            }
            for v in &f.any_of {
                out.push((format!("in.{key}"), v.clone()));
            }
        }
        if let Some((key, desc)) = self.sort.as_ref().filter(|_| !self.defaulted) {
            out.push((
                "sort".into(),
                format!("{}{key}", if *desc { "-" } else { "" }),
            ));
        }
        out.push(("per_page".into(), self.per_page.to_string()));
        out
    }
}

/// What a user chose in a grid's column menu. `None` means "the grid's
/// default".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GridPrefs {
    /// Column keys in the order they're shown.
    #[serde(default)]
    pub order: Option<Vec<String>>,
    /// Columns frozen at the left edge.
    #[serde(default)]
    pub left: Option<Vec<String>>,
    /// Columns frozen at the right edge.
    #[serde(default)]
    pub right: Option<Vec<String>>,
    /// Columns shown on small screens (under 768 px).
    #[serde(default)]
    pub compact: Option<Vec<String>>,
    /// Columns shown on wide screens.
    #[serde(default)]
    pub wide: Option<Vec<String>>,
}

impl GridPrefs {
    fn is_valid(&self) -> bool {
        [
            &self.order,
            &self.left,
            &self.right,
            &self.compact,
            &self.wide,
        ]
        .into_iter()
        .flatten()
        .all(|keys| keys.len() <= 200 && keys.iter().all(|k| valid_key(k)))
    }
}

fn session_key(grid: &str) -> String {
    format!("_grid.{grid}")
}

async fn load_prefs(request: &GridRequest, grid: &str) -> GridPrefs {
    match request.user_id {
        Some(user_id) => {
            let data: Option<String> =
                crate::db::sql("SELECT data FROM grid_preferences WHERE user_id = ? AND grid = ?")
                    .bind(user_id)
                    .bind(grid)
                    .scalar_optional(&request.db)
                    .await
                    .ok()
                    .flatten();
            data.and_then(|d| serde_json::from_str(&d).ok())
                .unwrap_or_default()
        }
        None => request
            .session
            .as_ref()
            .and_then(|s| s.get(&session_key(grid)))
            .unwrap_or_default(),
    }
}

/// Saves a user's choices for `grid` (what the column menu does).
pub async fn save_prefs(db: &Db, user_id: i64, grid: &str, prefs: &GridPrefs) -> Result {
    let data = serde_json::to_string(prefs)?;
    crate::db::sql(
        "INSERT INTO grid_preferences (user_id, grid, data, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (user_id, grid) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(grid)
    .bind(data)
    .bind(crate::db::now())
    .execute(db)
    .await?;
    Ok(())
}

/// The query string, the session and the user a grid needs, as an
/// extractor: `async fn index(request: GridRequest) -> Result<View>`.
#[derive(Clone)]
pub struct GridRequest {
    params: Vec<(String, String)>,
    path: String,
    db: Db,
    session: Option<Session>,
    user_id: Option<i64>,
    lang: Option<crate::Lang>,
    zone: crate::timezone::Zone,
}

impl GridRequest {
    /// A request for tests and commands: `params` as a query string would
    /// give them, for a guest.
    pub fn new(db: &Db, path: &str, params: &[(&str, &str)]) -> Self {
        Self {
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            path: path.to_owned(),
            db: db.clone(),
            session: None,
            user_id: None,
            lang: None,
            zone: crate::timezone::Zone::Fixed(0),
        }
    }

    /// The query string's value for `name`, if any.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for GridRequest {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Response> {
        let Some(app) = parts.extensions.get::<AppState>().cloned() else {
            return Err(
                Error::Internal(anyhow::anyhow!("GridRequest needs Renox's middleware"))
                    .into_response(),
            );
        };
        let params: Vec<(String, String)> = parts
            .uri
            .query()
            .and_then(|q| serde_html_form::from_str(q).ok())
            .unwrap_or_default();
        let session = Session::from_request_parts(parts, state).await.ok();
        let user = <AuthUser as axum::extract::OptionalFromRequestParts<S>>::from_request_parts(
            parts, state,
        )
        .await
        .ok()
        .flatten();
        let lang = crate::Lang::from_request_parts(parts, state).await.ok();
        Ok(Self {
            params,
            path: parts.uri.path().to_owned(),
            db: app.db.clone(),
            session,
            user_id: user.map(|u| u.id),
            lang,
            zone: app
                .config
                .timezone
                .parse()
                .unwrap_or(crate::timezone::Zone::Fixed(0)),
        })
    }
}

/// One page of a grid, as the `grid` macro of `renox/grid.html` draws it.
/// Pass it to the view as is; [`GridPage::extend`] adds values that aren't
/// columns of the model (a chart's points, a link).
pub struct GridPage<M> {
    grid: Grid,
    prefs: GridPrefs,
    state: State_,
    path: String,
    rows: Paginated<M>,
    extra: Vec<Map<String, Value>>,
}

impl<M: Serialize> GridPage<M> {
    /// Adds values to each row, from the row's model: `page.extend(|order|
    /// json!({ "trend": trends.get(&order.id) }))`. The values must be an
    /// object; its keys join the row's (the `custom` columns read them).
    pub fn extend(mut self, mut values: impl FnMut(&M) -> Value) -> Self {
        self.extra = self
            .rows
            .items
            .iter()
            .map(|item| match values(item) {
                Value::Object(map) => map,
                _ => Map::new(),
            })
            .collect();
        self
    }

    /// The models on this page.
    pub fn items(&self) -> &[M] {
        &self.rows.items
    }

    /// Rows matching the filters, on all pages.
    pub fn total(&self) -> u64 {
        self.rows.total
    }

    fn to_value(&self) -> Value {
        let ordered = self.grid.ordered(&self.prefs);
        let compact = self
            .prefs
            .compact
            .clone()
            .unwrap_or_else(|| self.grid.default_visible(true));
        let wide = self
            .prefs
            .wide
            .clone()
            .unwrap_or_else(|| self.grid.default_visible(false));
        let sort = self.state.sort.clone();
        // Rows can be dragged while sorted by the order column, ascending.
        let dragging = matches!((&self.grid.reorder, &sort), (Some((column, _)), Some((key, false))) if column == key);
        let columns: Vec<Value> = ordered
            .iter()
            .map(|(c, pin)| {
                let filter = self.state.filters.get(&c.key);
                json!({
                    "key": c.key,
                    "label": c.label,
                    "kind": c.kind,
                    "pin": pin,
                    "sortable": c.sortable,
                    "filterable": c.filterable,
                    "decimals": c.decimals,
                    "width": c.width,
                    "numeric": matches!(c.kind, Kind::Number | Kind::Money),
                    "options": c.options.iter().map(|(v, l)| json!({"value": v, "label": l})).collect::<Vec<_>>(),
                    "labels": c.options.iter().map(|(v, l)| (v.clone(), Value::from(l.clone()))).collect::<Map<_, _>>(),
                    "filter": filter.map(Filter::to_value).unwrap_or_else(|| Filter::default().to_value()),
                    "filtered": filter.is_some(),
                    "sort": match &sort {
                        Some((key, desc)) if *key == c.key => Value::from(if *desc { "desc" } else { "asc" }),
                        _ => Value::Null,
                    },
                    "compact": compact.contains(&c.key),
                    "wide": wide.contains(&c.key),
                    "editable": c.editable && self.grid.edit_url.is_some(),
                    "merge": c.merge && !dragging,
                })
            })
            .collect();
        let rows: Vec<Value> = self
            .rows
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let mut row = match serde_json::to_value(item) {
                    Ok(Value::Object(map)) => map,
                    _ => Map::new(),
                };
                if let Some(extra) = self.extra.get(i) {
                    row.extend(extra.clone());
                }
                Value::Object(row)
            })
            .collect();
        let merged: Vec<&str> = if dragging {
            Vec::new()
        } else {
            ordered
                .iter()
                .filter(|(c, _)| c.merge)
                .map(|(c, _)| c.key.as_str())
                .collect()
        };
        let spans = merge_spans(&rows, &merged);
        let rows: Vec<Value> = rows
            .into_iter()
            .zip(spans)
            .map(|(mut row, merge)| {
                let id = row.get("id").map(|id| match id {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
                if let Value::Object(map) = &mut row {
                    map.insert(
                        "_rx".into(),
                        json!({
                            "id": id,
                            "spans": merge.spans,
                            "covered": merge.covered,
                            "groups": merge.groups,
                            "edit": self.grid.edit_url.as_ref().zip(id.as_ref()).map(|(url, id)| url.replace("{id}", id)),
                        }),
                    );
                }
                row
            })
            .collect();
        let editable = self.grid.edit_url.is_some() && self.grid.columns.iter().any(|c| c.editable);
        let query = self.state.query_string();
        let query = &query;
        let config = json!({
            "id": self.grid.id,
            "prefs": format!("/_renox/grid/{}/prefs", self.grid.id),
            "order": ordered.iter().map(|(c, _)| c.key.clone()).collect::<Vec<_>>(),
            "left": ordered.iter().filter(|(_, p)| *p == Some(Pin::Left)).map(|(c, _)| c.key.clone()).collect::<Vec<_>>(),
            "right": ordered.iter().filter(|(_, p)| *p == Some(Pin::Right)).map(|(c, _)| c.key.clone()).collect::<Vec<_>>(),
            "compact": compact,
            "wide": wide,
            "defaults": {
                "compact": self.grid.default_visible(true),
                "wide": self.grid.default_visible(false),
            },
            "columns": ordered.iter().map(|(c, _)| (c.key.clone(), json!({
                "kind": c.kind,
                "label": c.label,
                "options": c.options,
                "editable": c.editable && self.grid.edit_url.is_some(),
            }))).collect::<Map<_, _>>(),
            "reorder": self.grid.reorder.as_ref().filter(|_| dragging).map(|(_, url)| url),
            "offset": u64::from(self.rows.page.saturating_sub(1)) * u64::from(self.rows.per_page),
        });
        json!({
            "id": self.grid.id,
            "title": self.grid.title,
            "path": self.path,
            "columns": columns,
            "header": header_rows(&ordered),
            "rows": rows,
            "page": {
                "page": self.rows.page,
                "per_page": self.rows.per_page,
                "total": self.rows.total,
                "last_page": self.rows.last_page,
                "from": self.rows.from,
                "to": self.rows.to,
                "has_prev": self.rows.has_prev,
                "has_next": self.rows.has_next,
                "pages": self.rows.pages,
            },
            "per_page_options": self.grid.per_page_options,
            "sort": sort.filter(|_| !self.state.defaulted).map(|(key, desc)| format!("{}{key}", if desc { "-" } else { "" })),
            "query": query,
            "filtered": self.state.filters.len(),
            "config": config.to_string(),
            "audit": self.grid.audit,
            "details": self.grid.audit || self.grid.details,
            "custom_details": self.grid.details,
            "editable": editable,
            "reorder": self.grid.reorder.as_ref().map(|(column, _)| json!({
                "column": column,
                "active": dragging,
            })),
            "tools": self.grid.audit || self.grid.details || editable || self.grid.reorder.is_some(),
            "exports": self.grid.exports.then(|| export::urls(&self.path, query)),
        })
    }
}

impl<M: Serialize> Serialize for GridPage<M> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

/// Where a row sits in the merged columns' groups.
#[derive(Debug, Default, Clone, PartialEq)]
struct MergeInfo {
    /// For the rows starting a group: how many rows the cell spans.
    spans: Map<String, Value>,
    /// The merged columns whose cell an earlier row draws.
    covered: Vec<String>,
    /// The group of each merged column, e.g. `city-3` (its first row).
    groups: Map<String, Value>,
}

/// Groups neighbouring rows with equal values in the `merged` columns,
/// each nested in the groups of the columns before it.
fn merge_spans(rows: &[Value], merged: &[&str]) -> Vec<MergeInfo> {
    let mut info = vec![MergeInfo::default(); rows.len()];
    // Where each row's group starts, per column.
    let mut starts: Vec<Vec<usize>> = vec![vec![0; merged.len()]; rows.len()];
    for i in 0..rows.len() {
        let mut outer_break = i == 0;
        for (k, key) in merged.iter().enumerate() {
            let same = !outer_break && rows[i].get(*key) == rows[i - 1].get(*key);
            if same {
                starts[i][k] = starts[i - 1][k];
            } else {
                starts[i][k] = i;
                outer_break = true;
            }
        }
    }
    for (k, key) in merged.iter().enumerate() {
        for i in 0..rows.len() {
            let start = starts[i][k];
            info[i]
                .groups
                .insert((*key).to_owned(), format!("{key}-{start}").into());
            if start == i {
                let len = (i..rows.len()).take_while(|&j| starts[j][k] == i).count();
                info[i].spans.insert((*key).to_owned(), len.into());
            } else {
                info[i].covered.push((*key).to_owned());
            }
        }
    }
    info
}

/// The new order of a page of rows, as a reorderable grid sends it
/// (`ids=4,2,9`: the ids in their new order, `offset` the position of the
/// first), read with axum's `Form`:
///
/// ```
/// # use renox::prelude::*;
/// use renox::grid::RowOrder;
/// # #[derive(Model, serde::Serialize, Default)] struct Task { id: i64, position: i64 }
/// async fn reorder(State(db): State<Db>, Form(order): Form<RowOrder>) -> Result<StatusCode> {
///     order.save::<Task>(&db, "position").await?;
///     Ok(StatusCode::NO_CONTENT)
/// }
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct RowOrder {
    /// The rows' ids, in their new order (sent separated by commas).
    #[serde(default, deserialize_with = "comma_list")]
    pub ids: Vec<String>,
    /// The position of the first row (the rows before this page).
    #[serde(default)]
    pub offset: u64,
}

fn comma_list<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<String>, D::Error> {
    let text = String::deserialize(d)?;
    Ok(text
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect())
}

impl RowOrder {
    /// Writes `offset + n` into `column` of the n-th row, in one
    /// transaction. Ids that don't parse as the model's key are an error;
    /// `updated_at` is left alone.
    pub async fn save<M: Model>(&self, db: &Db, column: &str) -> Result<u64> {
        if !M::COLUMNS.contains(&column) || column == "id" {
            return Err(Error::Internal(anyhow::anyhow!(
                "`{}` has no column `{column}` to order by",
                M::TABLE
            )));
        }
        if self.ids.len() > 1000 {
            return Err(Error::BadRequest("too many rows".into()));
        }
        let mut keys = Vec::with_capacity(self.ids.len());
        for id in &self.ids {
            let key: M::Key = id
                .parse()
                .map_err(|_| Error::BadRequest(format!("`{id}` is not an id")))?;
            keys.push(key);
        }
        let sql = format!(
            "UPDATE \"{}\" SET \"{column}\" = ? WHERE \"id\" = ?",
            M::TABLE
        );
        let mut tx = db.begin().await?;
        let mut changed = 0;
        for (n, key) in keys.into_iter().enumerate() {
            let position = i64::try_from(self.offset + n as u64).unwrap_or(i64::MAX);
            changed += crate::db::sql(&sql)
                .bind(position)
                .bind(key)
                .execute(&mut tx)
                .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
}

/// The heading rows: grouped headings (`Column::under`) span the columns
/// under them, leaf headings span down to the last row. Groups don't cross
/// a frozen edge.
fn header_rows(columns: &[(&Column, Option<Pin>)]) -> Vec<Vec<Value>> {
    let depth = columns
        .iter()
        .map(|(c, _)| c.under.len() + 1)
        .max()
        .unwrap_or(1);
    let mut rows: Vec<Vec<Value>> = vec![Vec::new(); depth];
    for (level, row) in rows.iter_mut().enumerate() {
        let mut previous: Option<(Vec<String>, Option<Pin>)> = None;
        for (index, (column, pin)) in columns.iter().enumerate() {
            if level < column.under.len() {
                let path = column.under[..=level].to_vec();
                let same = previous
                    .as_ref()
                    .is_some_and(|(p, side)| *p == path && side == pin);
                if same && let Some(Value::Object(cell)) = row.last_mut() {
                    let span = cell["colspan"].as_u64().unwrap_or(1) + 1;
                    cell.insert("colspan".into(), span.into());
                    if let Some(Value::Array(keys)) = cell.get_mut("keys") {
                        keys.push(column.key.clone().into());
                    }
                } else {
                    row.push(json!({
                        "group": true,
                        "label": column.under[level],
                        "colspan": 1,
                        "rowspan": 1,
                        "keys": [column.key],
                        "pin": pin,
                    }));
                }
                previous = Some((path, *pin));
            } else {
                if level == column.under.len() {
                    row.push(json!({
                        "group": false,
                        "key": column.key,
                        "index": index,
                        "colspan": 1,
                        "rowspan": depth - level,
                        "keys": [column.key],
                        "pin": pin,
                    }));
                }
                previous = None;
            }
        }
    }
    rows
}

/// `POST /_renox/grid/{id}/prefs` (JSON [`GridPrefs`]) saves a user's
/// choices; `DELETE` forgets them.
pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/_renox/grid/{grid}/prefs",
        post(save_route).delete(reset_route),
    )
}

async fn save_route(
    State(state): State<AppState>,
    axum::extract::Path(grid): axum::extract::Path<String>,
    session: Session,
    user: Option<AuthUser>,
    axum::Json(prefs): axum::Json<GridPrefs>,
) -> Result<Response> {
    if !valid_key(&grid) || !prefs.is_valid() {
        return Ok(StatusCode::UNPROCESSABLE_ENTITY.into_response());
    }
    match user {
        Some(user) => save_prefs(&state.db, user.id, &grid, &prefs).await?,
        None => session.put(&session_key(&grid), &prefs)?,
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn reset_route(
    State(state): State<AppState>,
    axum::extract::Path(grid): axum::extract::Path<String>,
    session: Session,
    user: Option<AuthUser>,
) -> Result<Response> {
    match user {
        Some(user) => {
            crate::db::sql("DELETE FROM grid_preferences WHERE user_id = ? AND grid = ?")
                .bind(user.id)
                .bind(&grid)
                .execute(&state.db)
                .await?;
        }
        None => {
            session.remove(&session_key(&grid));
        }
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid::new("orders")
            .column(Column::text("number", "Order").frozen())
            .column(Column::text("name", "Name").under(["Customer"]))
            .column(Column::text("email", "Email").under(["Customer"]).hidden())
            .column(Column::money("total", "Total").under(["Amounts", "Sum"]))
            .column(Column::number("tax", "Tax").under(["Amounts", "Sum"]))
            .column(Column::select("status", "Status", [("new", "New"), ("paid", "Paid")]).mobile())
            .column(Column::custom("trend", "Trend"))
            .column(Column::custom("actions", "").frozen_right())
    }

    fn params(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn the_query_string_is_checked_against_the_grid() {
        let grid = grid().sort_by("-total");
        let state = State_::parse(
            &grid,
            &params(&[
                ("q.name", "kopi"),
                ("m.name", "starts"),
                ("min.total", "1000"),
                ("max.total", "x"),
                ("in.status", "paid"),
                ("q.trend", "custom columns don't filter"),
                ("q.missing", "nor unknown ones"),
                ("m.email", "starts"),
                ("page", "3"),
                ("per_page", "1000"),
                ("sort", "trend"),
            ]),
        );
        assert_eq!(state.filters.len(), 3, "{:?}", state.filters);
        assert_eq!(state.filters["name"].pattern().as_deref(), Some("kopi%"));
        assert_eq!(state.filters["total"].min, Some(1000.0));
        assert_eq!(state.filters["total"].max, None);
        assert_eq!(state.page, 3);
        assert_eq!(state.per_page, 25, "only the offered sizes");
        assert_eq!(state.sort, Some(("total".into(), true)), "the default");

        let sorted = State_::parse(&grid, &params(&[("sort", "name"), ("per_page", "50")]));
        assert_eq!(sorted.sort, Some(("name".into(), false)));
        assert_eq!(sorted.per_page, 50);
        assert!(
            sorted
                .query_string()
                .contains(&("sort".into(), "name".into()))
        );
    }

    #[test]
    fn text_filters_make_like_patterns() {
        let pattern = |text: &str, mode: Option<&str>| {
            Filter {
                text: Some(text.into()),
                mode: mode.map(Into::into),
                ..Default::default()
            }
            .pattern()
        };
        assert_eq!(pattern("kopi", None).as_deref(), Some("%kopi%"));
        assert_eq!(pattern("kopi", Some("ends")).as_deref(), Some("%kopi"));
        assert_eq!(pattern("kopi", Some("equals")).as_deref(), Some("kopi"));
        assert_eq!(
            pattern("ko%pi", Some("equals")).as_deref(),
            Some("ko%pi"),
            "% wins"
        );
        assert_eq!(pattern("  ", None), None);
    }

    #[test]
    fn headings_group_and_span() {
        let grid = grid();
        let ordered = grid.ordered(&GridPrefs::default());
        let keys: Vec<&str> = ordered.iter().map(|(c, _)| c.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "number", "name", "email", "total", "tax", "status", "trend", "actions"
            ]
        );
        let rows = header_rows(&ordered);
        assert_eq!(rows.len(), 3);
        let labels: Vec<(String, u64, u64)> = rows[0]
            .iter()
            .map(|c| {
                (
                    c["label"]
                        .as_str()
                        .or(c["key"].as_str())
                        .unwrap()
                        .to_owned(),
                    c["colspan"].as_u64().unwrap(),
                    c["rowspan"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            labels,
            [
                ("number".into(), 1, 3),
                ("Customer".into(), 2, 1),
                ("Amounts".into(), 2, 1),
                ("status".into(), 1, 3),
                ("trend".into(), 1, 3),
                ("actions".into(), 1, 3),
            ]
        );
        // Row 2: Customer's leaves span down; Amounts → Sum.
        let second: Vec<&str> = rows[1]
            .iter()
            .map(|c| c["label"].as_str().or(c["key"].as_str()).unwrap())
            .collect();
        assert_eq!(second, ["name", "email", "Sum"]);
        assert_eq!(rows[1][0]["rowspan"], 2);
        let third: Vec<&str> = rows[2].iter().map(|c| c["key"].as_str().unwrap()).collect();
        assert_eq!(third, ["total", "tax"]);
    }

    #[test]
    fn preferences_reorder_and_freeze() {
        let grid = grid();
        let prefs = GridPrefs {
            order: Some(vec!["status".into(), "total".into(), "number".into()]),
            left: Some(vec!["total".into()]),
            right: Some(vec![]),
            ..Default::default()
        };
        let ordered = grid.ordered(&prefs);
        let keys: Vec<(&str, Option<Pin>)> =
            ordered.iter().map(|(c, p)| (c.key.as_str(), *p)).collect();
        assert_eq!(keys[0], ("total", Some(Pin::Left)));
        assert_eq!(keys[1], ("status", None));
        assert_eq!(keys[2], ("number", None), "unfrozen by the user");
        assert!(keys.iter().all(|(_, p)| *p != Some(Pin::Right)));
        // A group split by the frozen edge is two groups.
        let rows = header_rows(&ordered);
        let amounts = rows[0].iter().filter(|c| c["label"] == "Amounts").count();
        assert_eq!(amounts, 2);
    }

    #[test]
    fn small_screens_start_with_a_few_columns() {
        let grid = grid();
        assert_eq!(grid.default_visible(true), ["status"]);
        assert!(!grid.default_visible(false).contains(&"email".to_string()));
        let plain = Grid::new("x")
            .column(Column::text("a", "A"))
            .column(Column::text("b", "B").hidden())
            .column(Column::text("c", "C"))
            .column(Column::text("d", "D"))
            .column(Column::text("e", "E"));
        assert_eq!(plain.default_visible(true), ["a", "c", "d"]);
    }

    #[test]
    fn merged_columns_nest() {
        let rows: Vec<Value> = [
            ("java", "Bandung"),
            ("java", "Bandung"),
            ("java", "Jakarta"),
            ("bali", "Jakarta"),
            ("bali", "Denpasar"),
            ("java", "Denpasar"),
        ]
        .iter()
        .map(|(r, c)| json!({ "region": r, "city": c }))
        .collect();
        let info = merge_spans(&rows, &["region", "city"]);
        let spans = |i: usize| {
            (
                info[i].spans.get("region").cloned(),
                info[i].spans.get("city").cloned(),
            )
        };
        assert_eq!(spans(0), (Some(json!(3)), Some(json!(2))));
        assert_eq!(info[1].covered, ["region", "city"]);
        assert_eq!(spans(2), (None, Some(json!(1))));
        assert_eq!(info[2].covered, ["region"]);
        // Jakarta again, but under another region: a new group.
        assert_eq!(spans(3), (Some(json!(2)), Some(json!(1))));
        assert_eq!(spans(4), (None, Some(json!(1))));
        assert_eq!(spans(5), (Some(json!(1)), Some(json!(1))));
        assert_eq!(info[1].groups["city"], "city-0");
        assert!(merge_spans(&rows, &[]).iter().all(|m| m.spans.is_empty()));
    }

    #[test]
    fn preferences_are_checked() {
        assert!(GridPrefs::default().is_valid());
        let bad = GridPrefs {
            order: Some(vec!["a; DROP".into()]),
            ..Default::default()
        };
        assert!(!bad.is_valid());
        assert!(valid_key("orders-2026"));
        assert!(!valid_key(""));
    }
}

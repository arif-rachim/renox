//! Dashboards: numbers over time from the database ([`Trend`] over a
//! [`Period`], giving a [`Series`]), and the `chart(…)` template function
//! that draws them (line, area, bar, pie, doughnut) as plain HTML and SVG,
//! with the UI kit's `stat`, `widget` and `period_filter` around them.
//!
//! ```
//! # use renox::prelude::*;
//! use renox::chart::{Period, Trend};
//!
//! #[derive(Model, serde::Serialize, Default)]
//! struct Order { id: i64, total: i64, status: String, created_at: Option<renox::db::DateTime> }
//!
//! // `?period=30d` (7d, 30d, 90d, 12m, mtd, ytd; 30 days without one).
//! async fn dashboard(State(state): State<AppState>, period: Period) -> Result<View> {
//!     let paid = || Order::where_eq("status", "paid");
//!     let sales = Trend::of(paid(), "created_at").over(period).sum(&state, "total").await?;
//!     let before = Trend::of(paid(), "created_at").over(period.previous()).sum(&state, "total").await?;
//!     let orders = Trend::of(Order::query(), "created_at").over(period).count(&state).await?;
//!     Ok(view("dashboard.html", context! {
//!         period,
//!         revenue => sales.total(),
//!         change => sales.change_from(&before), // percent, None without a base
//!         sales => sales.named("Sales"),
//!         orders,
//!     }))
//! }
//! ```
//!
//! ```text
//! {% from "renox/ui.html" import stats, stat, dashboard, widget, period_filter %}
//! {{ period_filter(period) }}
//! {% call stats(3) %}
//!   {{ stat("Revenue", revenue | money, delta=change, trend=sales.values) }}
//! {% endcall %}
//! {% call dashboard(2) %}
//!   {% call widget("Sales", span=2) %}{{ chart("area", sales, format="money") }}{% endcall %}
//! {% endcall %}
//! ```

use std::fmt::Write as _;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Datelike, Duration, NaiveDate, NaiveTime};
use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Error, ErrorKind, State};
use serde::{Deserialize, Serialize, Serializer};

use crate::db::{DateTime, Dialect, Model, Query};
use crate::timezone::Zone;
use crate::toast::escape;
use crate::view_filters::{format_money, format_number};
use crate::{AppState, Result};

/// How long a dashboard looks back: `7d`, `30d`, `90d` (any number of days
/// up to 366), `12m` (months up to 36), `mtd` (this month so far) or `ytd`
/// (this year so far), in `APP_TIMEZONE`. As an extractor it reads
/// `?period=`, else 30 days; it serializes as its key (`"30d"`), which
/// the kit's `period_filter` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    span: Span,
    back: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    Days(u32),
    Months(u32),
    MonthToDate,
    YearToDate,
}

/// The step of a [`Series`]: one value per day or per month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Bucket {
    /// Labels `2026-10-02`.
    Day,
    /// Labels `2026-10`.
    Month,
}

impl Default for Period {
    fn default() -> Self {
        Period::days(30)
    }
}

impl Period {
    /// The last `n` days, today included.
    pub fn days(n: u32) -> Self {
        Self {
            span: Span::Days(n.clamp(1, 366)),
            back: 0,
        }
    }

    /// The last `n` months, this one included.
    pub fn months(n: u32) -> Self {
        Self {
            span: Span::Months(n.clamp(1, 36)),
            back: 0,
        }
    }

    /// This month so far.
    pub fn month_to_date() -> Self {
        Self {
            span: Span::MonthToDate,
            back: 0,
        }
    }

    /// This year so far.
    pub fn year_to_date() -> Self {
        Self {
            span: Span::YearToDate,
            back: 0,
        }
    }

    /// `"7d"`, `"12m"`, `"mtd"`, `"ytd"`; `None` for anything else.
    pub fn parse(key: &str) -> Option<Self> {
        let key = key.trim().to_ascii_lowercase();
        match key.as_str() {
            "mtd" => return Some(Self::month_to_date()),
            "ytd" => return Some(Self::year_to_date()),
            _ => {}
        }
        let (number, unit) = key.split_at(key.len().checked_sub(1)?);
        let n: u32 = number.parse().ok()?;
        match unit {
            "d" if (1..=366).contains(&n) => Some(Self::days(n)),
            "m" if (1..=36).contains(&n) => Some(Self::months(n)),
            _ => None,
        }
    }

    /// The key it parses from: `"30d"`.
    pub fn key(&self) -> String {
        match self.span {
            Span::Days(n) => format!("{n}d"),
            Span::Months(n) => format!("{n}m"),
            Span::MonthToDate => "mtd".into(),
            Span::YearToDate => "ytd".into(),
        }
    }

    /// The period just before, as long: the 30 days before the last 30, last
    /// month to the same day, last year to the same day. For comparisons
    /// ([`Series::change_from`]).
    pub fn previous(self) -> Self {
        Self {
            back: self.back + 1,
            ..self
        }
    }

    /// Per day up to 92 days (and month to date), else per month.
    pub fn bucket(&self) -> Bucket {
        match self.span {
            Span::Days(n) if n <= 92 => Bucket::Day,
            Span::MonthToDate => Bucket::Day,
            _ => Bucket::Month,
        }
    }

    /// The first local day and the day after the last, in `zone`'s calendar.
    fn days_in(&self, zone: Zone) -> (NaiveDate, NaiveDate) {
        let today = zone.local(crate::clock::unix_secs()).date();
        let back = self.back;
        match self.span {
            Span::Days(n) => {
                let end = today + Duration::days(1) - Duration::days(i64::from(n * back));
                (end - Duration::days(i64::from(n)), end)
            }
            Span::Months(n) => {
                let this_month = first_of_month(today);
                let end = add_months(this_month, 1 - (n * back) as i32);
                (add_months(end, -(n as i32)), end)
            }
            Span::MonthToDate => {
                let start = add_months(first_of_month(today), -(back as i32));
                let length = i64::from(today.day());
                let next = add_months(start, 1);
                (start, (start + Duration::days(length)).min(next))
            }
            Span::YearToDate => {
                let year = today.year() - back as i32;
                let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(today);
                let length = i64::from(today.ordinal());
                let next = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap_or(today);
                (start, (start + Duration::days(length)).min(next))
            }
        }
    }

    /// The moments the period starts at and ends before, in `zone`.
    pub fn range(&self, zone: Zone) -> (DateTime, DateTime) {
        let (start, end) = self.days_in(zone);
        (moment(zone, start), moment(zone, end))
    }

    /// The labels of its buckets, in order: `2026-10-02` per day, `2026-10`
    /// per month.
    pub fn labels(&self, zone: Zone) -> Vec<String> {
        let (start, end) = self.days_in(zone);
        let mut labels = Vec::new();
        match self.bucket() {
            Bucket::Day => {
                let mut day = start;
                while day < end && labels.len() < 400 {
                    labels.push(day.format("%Y-%m-%d").to_string());
                    day += Duration::days(1);
                }
            }
            Bucket::Month => {
                let mut month = first_of_month(start);
                while month < end && labels.len() < 400 {
                    labels.push(month.format("%Y-%m").to_string());
                    month = add_months(month, 1);
                }
            }
        }
        labels
    }
}

impl Serialize for Period {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.key())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Period {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        Ok(form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "period")
            .and_then(|(_, value)| Period::parse(&value))
            .unwrap_or_default())
    }
}

fn first_of_month(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

fn add_months(day: NaiveDate, months: i32) -> NaiveDate {
    let total = day.year() * 12 + day.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap_or(day)
}

/// Local midnight of `day` in `zone`, as a moment.
fn moment(zone: Zone, day: NaiveDate) -> DateTime {
    let local = day.and_time(NaiveTime::MIN);
    let unix = zone
        .resolve(local)
        .unwrap_or_else(|| local.and_utc().timestamp());
    DateTime::from_timestamp(unix, 0).unwrap_or_default()
}

/// Values over time: one per label (`2026-10-02` or `2026-10`), buckets
/// without rows at 0. Serialized as `{name, labels, values}` for the
/// `chart(…)` template function and `stat(trend=…)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Series {
    /// The series' name in a legend (`named`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// One label per value.
    pub labels: Vec<String>,
    /// The values.
    pub values: Vec<f64>,
}

impl Series {
    /// A series from labels and values (as many of each).
    pub fn new(labels: Vec<String>, values: Vec<f64>) -> Self {
        Self {
            name: None,
            labels,
            values,
        }
    }

    /// Names it, for a chart's legend.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The sum of the values.
    pub fn total(&self) -> f64 {
        self.values.iter().sum()
    }

    /// The change from `before`'s total, in percent (`12.5` for +12.5%);
    /// `None` when `before` adds up to 0.
    pub fn change_from(&self, before: &Series) -> Option<f64> {
        let base = before.total();
        (base != 0.0).then(|| (self.total() - base) / base.abs() * 100.0)
    }
}

/// A [`Series`] from a model's rows: how many (`count`), or the `sum` or
/// `average` of a column, per day or month of a [`Period`], by the moment
/// in `column` (a `DateTime` column such as `created_at`). The query's
/// conditions apply (`Order::where_eq("status", "paid")`); days are cut in
/// `APP_TIMEZONE`, at its offset at the end of the period.
pub struct Trend<M> {
    query: Query<M>,
    column: String,
    period: Period,
}

impl<M: Model> Trend<M> {
    /// The rows of `query`, placed in time by `column`; 30 days unless
    /// [`over`](Trend::over) says otherwise.
    pub fn of(query: Query<M>, column: &str) -> Self {
        Self {
            query,
            column: column.to_owned(),
            period: Period::default(),
        }
    }

    /// Over `period`.
    pub fn over(mut self, period: Period) -> Self {
        self.period = period;
        self
    }

    /// How many rows per bucket.
    pub async fn count(self, state: &AppState) -> Result<Series> {
        self.run(state, "COUNT(*)".to_owned()).await
    }

    /// The sum of `column` per bucket.
    pub async fn sum(self, state: &AppState, column: &str) -> Result<Series> {
        let column = checked::<M>(column)?;
        self.run(state, format!("SUM({column})")).await
    }

    /// The average of `column` per bucket (0 where there are no rows).
    pub async fn average(self, state: &AppState, column: &str) -> Result<Series> {
        let column = checked::<M>(column)?;
        self.run(state, format!("AVG({column})")).await
    }

    async fn run(self, state: &AppState, aggregate: String) -> Result<Series> {
        let zone: Zone = state.config.timezone;
        let column = checked::<M>(&self.column)?;
        let column = column.as_str();
        let (start, end) = self.period.range(zone);
        // Days are cut at the zone's offset at the end of the period.
        let minutes = zone.offset_at(end.timestamp()) / 60;
        let month = self.period.bucket() == Bucket::Month;
        let bucket = move |dialect: Dialect| match dialect {
            Dialect::Sqlite => format!(
                "strftime('{}', {column}, '{minutes:+} minutes')",
                if month { "%Y-%m" } else { "%Y-%m-%d" }
            ),
            Dialect::Postgres => format!(
                "to_char(({column} AT TIME ZONE 'UTC') + interval '{minutes} minutes', '{}')",
                if month { "YYYY-MM" } else { "YYYY-MM-DD" }
            ),
        };
        let condition = format!("{column} >= ? AND {column} < ?");
        let rows = self
            .query
            .where_raw(&condition, [start, end])
            .buckets(&state.db, &bucket, &aggregate)
            .await?;
        let found: std::collections::HashMap<String, f64> = rows
            .into_iter()
            .map(|(label, value)| (label, value.unwrap_or(0.0)))
            .collect();
        let labels = self.period.labels(zone);
        let values = labels
            .iter()
            .map(|label| found.get(label).copied().unwrap_or(0.0))
            .collect();
        Ok(Series::new(labels, values))
    }
}

/// `column`, quoted, if the model has it.
fn checked<M: Model>(column: &str) -> Result<String> {
    if M::COLUMNS.contains(&column) {
        Ok(format!("\"{column}\""))
    } else {
        Err(anyhow::anyhow!("`{}` has no column `{column}`", M::TABLE).into())
    }
}

// ---------------------------------------------------------------------------
// The `chart(…)` template function.

/// One line, area or set of bars.
struct Line {
    name: String,
    values: Vec<Option<f64>>,
}

/// What a chart draws.
struct Data {
    labels: Vec<String>,
    series: Vec<Line>,
}

fn number_of(value: &Value) -> Option<f64> {
    if value.is_none() || value.is_undefined() {
        return None;
    }
    if let Ok(i) = i64::try_from(value.clone()) {
        return Some(i as f64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse().ok();
    }
    f64::try_from(value.clone()).ok()
}

fn strings(value: &Value) -> Vec<String> {
    match value.try_iter() {
        Ok(items) if value.kind() == ValueKind::Seq => items
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn numbers(value: &Value) -> Vec<Option<f64>> {
    match value.try_iter() {
        Ok(items) if value.kind() == ValueKind::Seq => items.map(|v| number_of(&v)).collect(),
        _ => Vec::new(),
    }
}

fn attr(value: &Value, key: &str) -> Option<Value> {
    value
        .get_attr(key)
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_none())
}

/// A series from `{name, values}` (or a [`Series`]), or a list of numbers.
fn line_of(value: &Value, fallback: &str) -> Line {
    if value.kind() == ValueKind::Map {
        Line {
            name: attr(value, "name")
                .and_then(|n| n.as_str().map(str::to_owned))
                .unwrap_or_else(|| fallback.to_owned()),
            values: attr(value, "values")
                .map(|v| numbers(&v))
                .unwrap_or_default(),
        }
    } else {
        Line {
            name: fallback.to_owned(),
            values: numbers(value),
        }
    }
}

fn read_data(
    data: Option<Value>,
    kwargs: &Kwargs,
    title: &str,
) -> std::result::Result<Data, Error> {
    let labels: Option<Value> = kwargs.get("labels")?;
    let series: Option<Value> = kwargs.get("series")?;
    let values: Option<Value> = kwargs.get("values")?;
    let name: Option<String> = kwargs.get("name")?;
    let fallback = name.clone().unwrap_or_else(|| title.to_owned());
    let mut out = Data {
        labels: labels.as_ref().map(strings).unwrap_or_default(),
        series: Vec::new(),
    };
    let list_of_series = |value: &Value, out: &mut Data| {
        if let Ok(items) = value.try_iter() {
            for (i, item) in items.enumerate() {
                out.series
                    .push(line_of(&item, &format!("{} {}", fallback, i + 1)));
            }
        }
    };
    match data {
        Some(data) if data.kind() == ValueKind::Map => {
            if out.labels.is_empty()
                && let Some(labels) = attr(&data, "labels")
            {
                out.labels = strings(&labels);
            }
            match attr(&data, "series") {
                Some(series) => list_of_series(&series, &mut out),
                None => out.series.push(line_of(&data, &fallback)),
            }
        }
        Some(data) if data.kind() == ValueKind::Seq => {
            let first_is_map = data
                .try_iter()
                .ok()
                .and_then(|mut items| items.next())
                .is_some_and(|v| v.kind() == ValueKind::Map);
            if first_is_map {
                list_of_series(&data, &mut out);
            } else {
                out.series.push(line_of(&data, &fallback));
            }
        }
        _ => {}
    }
    if let Some(series) = series {
        list_of_series(&series, &mut out);
    }
    if let Some(values) = values {
        out.series.push(Line {
            name: fallback.clone(),
            values: numbers(&values),
        });
    }
    if let Some(name) = name
        && out.series.len() == 1
    {
        out.series[0].name = name;
    }
    let longest = out.series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    if out.labels.len() < longest {
        for i in out.labels.len()..longest {
            out.labels.push((i + 1).to_string());
        }
    }
    let n = out.labels.len();
    for line in &mut out.series {
        line.values.resize(n, None);
    }
    Ok(out)
}

/// The page's language, for separators and month names.
fn locale(state: &State) -> String {
    state
        .lookup("app")
        .and_then(|app| app.get_attr("locale").ok())
        .and_then(|l| l.as_str().map(str::to_owned))
        .unwrap_or_else(|| "en".into())
}

fn text(state: &State, key: &str, fallback_locale: &str) -> String {
    state
        .lookup("t")
        .and_then(|t| t.call(state, &[Value::from(key)]).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| crate::i18n::builtin_text(fallback_locale, key))
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `2026-10-02` → `Oct 2`, `2026-10` → `Oct 2026`, or with `format`
/// (chrono's codes, e.g. `%d/%m`); other labels as they are.
fn display_label(label: &str, format: Option<&str>) -> String {
    if let Ok(day) = NaiveDate::parse_from_str(label, "%Y-%m-%d") {
        return match format {
            Some(format) => day.format(format).to_string(),
            None => format!("{} {}", MONTHS[day.month0() as usize], day.day()),
        };
    }
    if let Ok(month) = NaiveDate::parse_from_str(&format!("{label}-01"), "%Y-%m-%d") {
        return match format {
            Some(format) => month.format(format).to_string(),
            None => format!("{} {}", MONTHS[month.month0() as usize], month.year()),
        };
    }
    label.to_owned()
}

/// How values read in tooltips and the table.
struct Formatter {
    format: String,
    decimals: Option<u32>,
    currency: String,
    locale: String,
}

impl Formatter {
    fn full(&self, value: f64) -> String {
        match self.format.as_str() {
            "money" => format_money(value, &self.currency, self.decimals, &self.locale),
            "percent" => format!(
                "{}%",
                format_number(value, self.decimals.unwrap_or(1), &self.locale)
            ),
            _ => format_number(value, self.decimals.unwrap_or(0), &self.locale),
        }
    }

    /// Axis ticks: `12.5K` (`12,5K` where the locale writes a decimal
    /// comma), plain under 10,000.
    fn tick(&self, value: f64) -> String {
        let units: [(f64, &str); 4] = [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")];
        let suffix = if self.format == "percent" { "%" } else { "" };
        if value.abs() >= 10_000.0 {
            for (size, unit) in units {
                if value.abs() >= size {
                    let scaled = value / size;
                    let decimals = if scaled.fract().abs() < 1e-9 { 0 } else { 1 };
                    return format!(
                        "{}{unit}{suffix}",
                        format_number(scaled, decimals, &self.locale)
                    );
                }
            }
        }
        let decimals = if value.fract().abs() < 1e-9 { 0 } else { 1 };
        format!("{}{suffix}", format_number(value, decimals, &self.locale))
    }
}

/// Clean ticks from `lo` to `hi` (0 always included): about four steps of
/// 1, 2, 2.5 or 5 × 10ⁿ.
fn scale(lo: f64, hi: f64) -> (f64, f64, f64) {
    let (mut lo, mut hi) = (lo.min(0.0), hi.max(0.0));
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }
    let raw = (hi - lo) / 4.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let normal = raw / magnitude;
    let nice = if normal <= 1.0 {
        1.0
    } else if normal <= 2.0 {
        2.0
    } else if normal <= 2.5 {
        2.5
    } else if normal <= 5.0 {
        5.0
    } else {
        10.0
    };
    let step = nice * magnitude;
    lo = (lo / step).floor() * step;
    hi = (hi / step).ceil() * step;
    (lo, hi, step)
}

fn slot(i: usize) -> String {
    if i < 6 {
        format!("rx-series-{}", i + 1)
    } else {
        "rx-series-other".into()
    }
}

fn pct(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded}")
}

/// `chart(kind, data, …)`: see docs/ui.md "Charts".
pub(crate) fn chart(
    currency: String,
) -> impl Fn(&State, String, Option<Value>, Kwargs) -> std::result::Result<Value, Error>
+ Send
+ Sync
+ 'static {
    move |state: &State, kind: String, data: Option<Value>, kwargs: Kwargs| {
        let title: Option<String> = kwargs.get("title")?;
        let height: Option<u32> = kwargs.get("height")?;
        let format: Option<String> = kwargs.get("format")?;
        let decimals: Option<u32> = kwargs.get("decimals")?;
        let currency_kw: Option<String> = kwargs.get("currency")?;
        let stacked: Option<bool> = kwargs.get("stacked")?;
        let legend: Option<bool> = kwargs.get("legend")?;
        let table: Option<bool> = kwargs.get("table")?;
        let x_format: Option<String> = kwargs.get("x_format")?;
        let id: Option<String> = kwargs.get("id")?;
        let locale = locale(state);
        let title = title.unwrap_or_default();
        let data = read_data(data, &kwargs, &title)?;
        kwargs.assert_all_used()?;
        let formatter = Formatter {
            format: format.unwrap_or_else(|| "number".into()),
            decimals,
            currency: currency_kw
                .map(|c| c.trim().to_ascii_uppercase())
                .unwrap_or_else(|| currency.clone()),
            locale: locale.clone(),
        };
        let options = Options {
            kind: kind.clone(),
            title,
            height: height.unwrap_or(240).clamp(80, 800),
            stacked: stacked.unwrap_or(false),
            legend: legend.unwrap_or(true),
            table: table.unwrap_or(true),
            labels: data
                .labels
                .iter()
                .map(|l| display_label(l, x_format.as_deref()))
                .collect(),
            id,
            show_data: text(state, "ui.chart.show_data", &locale),
            other: text(state, "ui.chart.other", &locale),
        };
        let html = match kind.as_str() {
            "line" | "area" | "bar" => render_xy(&data, &options, &formatter),
            "pie" | "doughnut" => render_pie(&data, &options, &formatter),
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("chart: unknown kind `{other}` (line, area, bar, pie or doughnut)"),
                ));
            }
        };
        Ok(Value::from_safe_string(html))
    }
}

struct Options {
    kind: String,
    title: String,
    height: u32,
    stacked: bool,
    legend: bool,
    table: bool,
    labels: Vec<String>,
    id: Option<String>,
    show_data: String,
    other: String,
}

/// The data the page's script reads for tooltips (and a table reads too).
#[derive(Serialize)]
struct Hover<'a> {
    kind: &'a str,
    labels: &'a [String],
    series: Vec<HoverSeries>,
}

#[derive(Serialize)]
struct HoverSeries {
    name: String,
    slot: String,
    values: Vec<Option<String>>,
}

fn open_figure(out: &mut String, options: &Options, hover: &Hover) {
    let json = serde_json::to_string(hover).unwrap_or_default();
    let _ = write!(
        out,
        r#"<figure class="rx-chart rx-chart--{kind}"{id} data-rx-chart="{json}">"#,
        kind = escape(&options.kind),
        id = options
            .id
            .as_deref()
            .map(|id| format!(r#" id="{}""#, escape(id)))
            .unwrap_or_default(),
        json = escape(&json),
    );
    if !options.title.is_empty() {
        let _ = write!(
            out,
            r#"<figcaption class="rx-visually-hidden">{}</figcaption>"#,
            escape(&options.title)
        );
    }
}

fn legend(out: &mut String, names: &[(usize, &str)], key: &str) {
    out.push_str(r#"<ul class="rx-chart__legend" role="list">"#);
    for (i, name) in names {
        let _ = write!(
            out,
            r#"<li><span class="rx-chart__key rx-chart__key--{key} {slot}" aria-hidden="true"></span>{name}</li>"#,
            slot = slot(*i),
            name = escape(name),
        );
    }
    out.push_str("</ul>");
}

fn table(out: &mut String, options: &Options, heads: &[String], rows: &[(String, Vec<String>)]) {
    if !options.table {
        return;
    }
    let _ = write!(
        out,
        r#"<details class="rx-chart__table"><summary>{}</summary><div class="rx-table-wrap"><table class="rx-table"><thead><tr><th scope="col"></th>"#,
        escape(&options.show_data)
    );
    for head in heads {
        let _ = write!(
            out,
            r#"<th scope="col" class="rx-num">{}</th>"#,
            escape(head)
        );
    }
    out.push_str("</tr></thead><tbody>");
    for (label, cells) in rows {
        let _ = write!(out, r#"<tr><th scope="row">{}</th>"#, escape(label));
        for cell in cells {
            let _ = write!(out, r#"<td class="rx-num">{}</td>"#, escape(cell));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div></details>");
}

fn render_xy(data: &Data, options: &Options, formatter: &Formatter) -> String {
    let n = data.labels.len();
    let bar = options.kind == "bar";
    let stacked = bar && options.stacked;
    // The value range: per label sums for stacked bars.
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for i in 0..n {
        let (mut up, mut down) = (0.0, 0.0);
        for line in &data.series {
            if let Some(v) = line.values[i] {
                if stacked {
                    if v >= 0.0 { up += v } else { down += v }
                } else {
                    hi = hi.max(v);
                    lo = lo.min(v);
                }
            }
        }
        hi = hi.max(up);
        lo = lo.min(down);
    }
    let (lo, hi, step) = scale(lo, hi);
    let y = |v: f64| (v - lo) / (hi - lo) * 100.0;
    let zero = y(0.0);

    let hover = Hover {
        kind: &options.kind,
        labels: &options.labels,
        series: data
            .series
            .iter()
            .enumerate()
            .map(|(i, line)| HoverSeries {
                name: line.name.clone(),
                slot: slot(i),
                values: line
                    .values
                    .iter()
                    .map(|v| v.map(|v| formatter.full(v)))
                    .collect(),
            })
            .collect(),
    };
    let mut out = String::new();
    open_figure(&mut out, options, &hover);
    if options.legend && data.series.len() > 1 {
        let names: Vec<(usize, &str)> = data
            .series
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.name.as_str()))
            .collect();
        legend(&mut out, &names, if bar { "box" } else { "line" });
    }
    let _ = write!(
        out,
        r#"<div class="rx-chart__frame" style="--rx-chart-h: {}px"><div class="rx-chart__y" aria-hidden="true">"#,
        options.height
    );
    let mut ticks = Vec::new();
    let mut tick = lo;
    while tick <= hi + step / 2.0 && ticks.len() < 12 {
        ticks.push(tick);
        tick += step;
    }
    for t in &ticks {
        let _ = write!(
            out,
            r#"<span style="bottom: {}%">{}</span>"#,
            pct(y(*t)),
            escape(&formatter.tick(*t))
        );
    }
    let label = if options.title.is_empty() {
        data.series
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        options.title.clone()
    };
    let _ = write!(
        out,
        r#"</div><div class="rx-chart__plot" tabindex="0" role="img" aria-label="{}"><div class="rx-chart__grid" aria-hidden="true">"#,
        escape(&label)
    );
    for t in &ticks {
        let base = if t.abs() < step / 1e6 {
            " rx-chart__rule--base"
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<span class="rx-chart__rule{base}" style="bottom: {}%"></span>"#,
            pct(y(*t))
        );
    }
    out.push_str("</div>");

    if bar {
        out.push_str(r#"<div class="rx-chart__bars" aria-hidden="true">"#);
        for i in 0..n {
            let _ = write!(out, r#"<div class="rx-chart__band" data-index="{i}">"#);
            if stacked {
                out.push_str(r#"<div class="rx-chart__column">"#);
                let (mut up, mut down) = (0.0, 0.0);
                let tops: Vec<usize> = {
                    // The outermost segment on each side gets the rounded end.
                    let mut last_up = None;
                    let mut last_down = None;
                    for (s, line) in data.series.iter().enumerate() {
                        match line.values[i] {
                            Some(v) if v > 0.0 => last_up = Some(s),
                            Some(v) if v < 0.0 => last_down = Some(s),
                            _ => {}
                        }
                    }
                    last_up.into_iter().chain(last_down).collect()
                };
                for (s, line) in data.series.iter().enumerate() {
                    let Some(v) = line.values[i].filter(|v| *v != 0.0) else {
                        continue;
                    };
                    let (from, to) = if v > 0.0 {
                        let from = up;
                        up += v;
                        (from, up)
                    } else {
                        let from = down;
                        down += v;
                        (down, from)
                    };
                    let classes = format!(
                        "rx-chart__bar {}{}{}",
                        slot(s),
                        if v < 0.0 { " rx-chart__bar--neg" } else { "" },
                        if tops.contains(&s) {
                            " rx-chart__bar--end"
                        } else {
                            ""
                        },
                    );
                    let _ = write!(
                        out,
                        r#"<span class="{classes}" style="bottom: {}%; height: {}%"></span>"#,
                        pct(y(from)),
                        pct(y(to) - y(from)),
                    );
                }
                out.push_str("</div>");
            } else {
                for (s, line) in data.series.iter().enumerate() {
                    out.push_str(r#"<div class="rx-chart__column">"#);
                    if let Some(v) = line.values[i].filter(|v| *v != 0.0) {
                        let (from, to) = if v > 0.0 { (0.0, v) } else { (v, 0.0) };
                        let _ = write!(
                            out,
                            r#"<span class="rx-chart__bar rx-chart__bar--end {}{}" style="bottom: {}%; height: {}%"></span>"#,
                            slot(s),
                            if v < 0.0 { " rx-chart__bar--neg" } else { "" },
                            pct(y(from)),
                            pct(y(to) - y(from)),
                        );
                    }
                    out.push_str("</div>");
                }
            }
            out.push_str("</div>");
        }
        out.push_str("</div>");
    } else {
        let x = |i: usize| {
            if n > 1 {
                i as f64 / (n - 1) as f64 * 1000.0
            } else {
                500.0
            }
        };
        let sy = |v: f64| 1000.0 - y(v) * 10.0;
        out.push_str(r#"<svg class="rx-chart__svg" viewBox="0 0 1000 1000" preserveAspectRatio="none" aria-hidden="true" focusable="false">"#);
        for (s, line) in data.series.iter().enumerate() {
            // Runs of values; a missing one breaks the line.
            let mut runs: Vec<Vec<(f64, f64)>> = vec![Vec::new()];
            for (i, value) in line.values.iter().enumerate() {
                match (value, runs.last_mut()) {
                    (Some(v), Some(run)) => run.push((x(i), sy(*v))),
                    _ => runs.push(Vec::new()),
                }
            }
            for run in runs.iter().filter(|r| !r.is_empty()) {
                let path: String = run
                    .iter()
                    .enumerate()
                    .map(|(k, (px, py))| {
                        format!("{}{:.1},{:.1}", if k == 0 { "M" } else { "L" }, px, py)
                    })
                    .collect();
                if options.kind == "area" {
                    let base = 1000.0 - zero * 10.0;
                    let _ = write!(
                        out,
                        r#"<path class="rx-chart__area {}" d="{path}L{:.1},{base:.1}L{:.1},{base:.1}Z"/>"#,
                        slot(s),
                        run.last().map(|p| p.0).unwrap_or(0.0),
                        run.first().map(|p| p.0).unwrap_or(0.0),
                    );
                }
                let _ = write!(
                    out,
                    r#"<path class="rx-chart__line {}" d="{path}" vector-effect="non-scaling-stroke"/>"#,
                    slot(s)
                );
            }
        }
        out.push_str("</svg>");
        // The last point of each line, as a dot that stays round.
        for (s, line) in data.series.iter().enumerate() {
            if let Some((i, v)) = line
                .values
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, v)| v.map(|v| (i, v)))
            {
                let _ = write!(
                    out,
                    r#"<span class="rx-chart__dot {}" style="left: {}%; bottom: {}%" aria-hidden="true"></span>"#,
                    slot(s),
                    pct(x(i) / 10.0),
                    pct(y(v)),
                );
            }
        }
    }
    out.push_str(r#"<span class="rx-chart__cross" hidden></span><div class="rx-chart__tip" hidden></div></div>"#);

    // X labels: at most 8, the first and the last always.
    out.push_str(r#"<div class="rx-chart__x" aria-hidden="true">"#);
    let every = n.div_ceil(8).max(1);
    let mut shown_at: Vec<usize> = (0..n).step_by(every).collect();
    if let Some(&last) = shown_at.last()
        && last + 1 < n
    {
        // The last label always shows; one too close before it gives way.
        if n - 1 - last < every.div_ceil(2) && shown_at.len() > 1 {
            shown_at.pop();
        }
        shown_at.push(n - 1);
    }
    for (shown, &i) in shown_at.iter().enumerate() {
        let label = &options.labels[i];
        let left = if bar {
            (i as f64 + 0.5) / n as f64 * 100.0
        } else if n > 1 {
            i as f64 / (n - 1) as f64 * 100.0
        } else {
            50.0
        };
        // On phones every other label hides; the first and the last stay.
        let narrow = if shown % 2 == 1 && shown + 1 < shown_at.len() {
            " rx-chart__x--odd"
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<span class="{narrow}" style="left: {}%">{}</span>"#,
            pct(left),
            escape(label)
        );
    }
    out.push_str("</div></div>");

    let heads: Vec<String> = data.series.iter().map(|s| s.name.clone()).collect();
    let rows: Vec<(String, Vec<String>)> = (0..n)
        .map(|i| {
            (
                options.labels[i].clone(),
                data.series
                    .iter()
                    .map(|s| {
                        s.values[i]
                            .map(|v| formatter.full(v))
                            .unwrap_or_else(|| "—".into())
                    })
                    .collect(),
            )
        })
        .collect();
    table(&mut out, options, &heads, &rows);
    out.push_str("</figure>");
    out
}

fn render_pie(data: &Data, options: &Options, formatter: &Formatter) -> String {
    let values = data
        .series
        .first()
        .map(|s| s.values.clone())
        .unwrap_or_default();
    // Six slices at most: the rest is "Other".
    let mut slices: Vec<(String, f64)> = data
        .labels
        .iter()
        .zip(values)
        .map(|(label, v)| (label.clone(), v.unwrap_or(0.0).max(0.0)))
        .collect();
    let display: Vec<String> = options.labels.clone();
    for (slice, label) in slices.iter_mut().zip(&display) {
        slice.0 = label.clone();
    }
    if slices.len() > 6 {
        let rest: f64 = slices[5..].iter().map(|s| s.1).sum();
        slices.truncate(5);
        slices.push((options.other.clone(), rest));
    }
    let total: f64 = slices.iter().map(|s| s.1).sum();
    let share = |v: f64| if total > 0.0 { v / total * 100.0 } else { 0.0 };
    let percent = |v: f64| format!("{}%", format_number(share(v), 1, &formatter.locale));
    let labels: Vec<String> = slices.iter().map(|s| s.0.clone()).collect();
    let hover = Hover {
        kind: &options.kind,
        labels: &labels,
        series: vec![HoverSeries {
            name: options.title.clone(),
            slot: String::new(),
            values: slices
                .iter()
                .map(|s| Some(format!("{} ({})", formatter.full(s.1), percent(s.1))))
                .collect(),
        }],
    };
    let mut out = String::new();
    open_figure(&mut out, options, &hover);
    let label = if options.title.is_empty() {
        labels.join(", ")
    } else {
        options.title.clone()
    };
    let _ = write!(
        out,
        r#"<div class="rx-chart__pie"><div class="rx-chart__plot rx-chart__plot--pie" tabindex="0" role="img" aria-label="{}"><svg viewBox="-50 -50 100 100" aria-hidden="true" focusable="false">"#,
        escape(&label)
    );
    let inner = if options.kind == "doughnut" {
        30.0
    } else {
        0.0
    };
    let outer = 48.0;
    let mut angle = -std::f64::consts::FRAC_PI_2;
    for (i, (_, v)) in slices.iter().enumerate() {
        if *v <= 0.0 || total <= 0.0 {
            continue;
        }
        let sweep = v / total * std::f64::consts::TAU;
        let class = format!("rx-chart__slice {}", slot(i));
        if sweep >= std::f64::consts::TAU - 1e-9 {
            let _ = write!(
                out,
                r#"<circle class="{class}" data-index="{i}" r="{}" fill-rule="evenodd"/>"#,
                outer
            );
            if inner > 0.0 {
                let _ = write!(out, r#"<circle class="rx-chart__hole" r="{inner}"/>"#);
            }
        } else {
            let (a0, a1) = (angle, angle + sweep);
            let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
            let p = |r: f64, a: f64| (r * a.cos(), r * a.sin());
            let (x0, y0) = p(outer, a0);
            let (x1, y1) = p(outer, a1);
            let d = if inner > 0.0 {
                let (x2, y2) = p(inner, a1);
                let (x3, y3) = p(inner, a0);
                format!(
                    "M{x0:.3},{y0:.3}A{outer},{outer} 0 {large} 1 {x1:.3},{y1:.3}L{x2:.3},{y2:.3}A{inner},{inner} 0 {large} 0 {x3:.3},{y3:.3}Z"
                )
            } else {
                format!("M0,0L{x0:.3},{y0:.3}A{outer},{outer} 0 {large} 1 {x1:.3},{y1:.3}Z")
            };
            let _ = write!(out, r#"<path class="{class}" data-index="{i}" d="{d}"/>"#);
        }
        angle += sweep;
    }
    out.push_str(r#"</svg><div class="rx-chart__tip" hidden></div></div>"#);
    if options.legend {
        out.push_str(r#"<ul class="rx-chart__legend rx-chart__legend--pie" role="list">"#);
        for (i, (name, v)) in slices.iter().enumerate() {
            let _ = write!(
                out,
                r#"<li><span class="rx-chart__key rx-chart__key--box {}" aria-hidden="true"></span><span class="rx-chart__name">{}</span><span class="rx-chart__value">{} <span class="rx-chart__share">{}</span></span></li>"#,
                slot(i),
                escape(name),
                escape(&formatter.full(*v)),
                escape(&percent(*v)),
            );
        }
        out.push_str("</ul>");
    }
    out.push_str("</div>");
    let heads = vec![
        if options.title.is_empty() {
            "".into()
        } else {
            options.title.clone()
        },
        "%".into(),
    ];
    let rows: Vec<(String, Vec<String>)> = slices
        .iter()
        .map(|(name, v)| (name.clone(), vec![formatter.full(*v), percent(*v)]))
        .collect();
    table(&mut out, options, &heads, &rows);
    out.push_str("</figure>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_parse_and_step_back() {
        assert_eq!(Period::parse("30d"), Some(Period::days(30)));
        assert_eq!(Period::parse("12M").map(|p| p.key()), Some("12m".into()));
        assert_eq!(Period::parse("ytd"), Some(Period::year_to_date()));
        for bad in ["", "0d", "400d", "40m", "7w", "d", "-3d"] {
            assert_eq!(Period::parse(bad), None, "{bad}");
        }
        assert_eq!(Period::days(30).bucket(), Bucket::Day);
        assert_eq!(Period::days(365).bucket(), Bucket::Month);
        let zone = Zone::UTC;
        let (start, end) = Period::days(7).days_in(zone);
        assert_eq!((end - start).num_days(), 7);
        let (before_start, before_end) = Period::days(7).previous().days_in(zone);
        assert_eq!(
            (before_end, (before_end - before_start).num_days()),
            (start, 7)
        );
        assert_eq!(Period::days(7).labels(zone).len(), 7);
        assert_eq!(Period::months(12).labels(zone).len(), 12);
        let (m0, m1) = Period::months(3).previous().days_in(zone);
        assert_eq!((m0.day(), m1.day()), (1, 1));
        assert_eq!(
            add_months(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), -1),
            NaiveDate::from_ymd_opt(2025, 12, 1).unwrap()
        );
    }

    #[test]
    fn series_add_up_and_compare() {
        let now = Series::new(vec!["a".into(), "b".into()], vec![30.0, 30.0]);
        let before = Series::new(vec!["a".into(), "b".into()], vec![20.0, 20.0]);
        assert_eq!(now.change_from(&before), Some(50.0));
        assert_eq!(now.change_from(&Series::default()), None);
        assert_eq!(
            serde_json::to_value(now.named("Sales")).unwrap()["name"],
            "Sales"
        );
    }

    #[test]
    fn scales_are_clean() {
        assert_eq!(scale(0.0, 87.0), (0.0, 100.0, 25.0));
        assert_eq!(scale(0.0, 1234.0), (0.0, 1500.0, 500.0));
        assert_eq!(scale(-30.0, 70.0), (-50.0, 75.0, 25.0));
        assert_eq!(scale(0.0, 0.0), (0.0, 1.0, 0.25));
    }

    #[test]
    fn labels_and_ticks_read_well() {
        assert_eq!(display_label("2026-10-02", None), "Oct 2");
        assert_eq!(display_label("2026-08", None), "Aug 2026");
        assert_eq!(display_label("2026-10-02", Some("%d/%m")), "02/10");
        assert_eq!(display_label("Coffee", None), "Coffee");
        let f = |locale: &str| Formatter {
            format: "number".into(),
            decimals: None,
            currency: "IDR".into(),
            locale: locale.into(),
        };
        assert_eq!(f("en").tick(12_500.0), "12.5K");
        assert_eq!(f("es").tick(2_500_000.0), "2,5M");
        assert_eq!(f("en").tick(750.0), "750");
        assert_eq!(f("en").tick(2_000_000_000.0), "2B");
    }
}

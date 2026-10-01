//! A grid's exports: every row its filters match, in the user's columns, as
//! CSV, an Excel workbook (the `xlsx` feature) or a page to print.

use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{Column, Grid, GridRequest, Kind, Pin, header_rows, load_prefs};
use crate::db::{Model, Query};
use crate::{Error, Result};

/// The most rows one export holds.
pub const MAX_EXPORT_ROWS: u64 = 100_000;

/// The links of the export menu: the page's filters and sort, plus `export`.
pub(super) fn urls(path: &str, query: &[(String, String)]) -> Value {
    let link = |format: &str| {
        let mut pairs: Vec<(&str, &str)> = query
            .iter()
            .filter(|(k, _)| k != "per_page")
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        pairs.push(("export", format));
        format!(
            "{path}?{}",
            serde_html_form::to_string(&pairs).unwrap_or_default()
        )
    };
    json!({
        "csv": link("csv"),
        "xlsx": cfg!(feature = "xlsx").then(|| link("xlsx")),
        "print": link("print"),
    })
}

impl Grid {
    /// Answers the export menu's links (`?export=csv|xlsx|print`): `None`
    /// when the request isn't an export, so the handler goes on to draw the
    /// page.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// use renox::grid::{Column, Grid, GridRequest};
    /// # #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, number: String }
    /// fn orders() -> Grid {
    ///     Grid::new("orders").column(Column::text("number", "Order")).exports()
    /// }
    ///
    /// async fn index(request: GridRequest) -> Result<Response> {
    ///     if let Some(file) = orders().export(Order::query(), &request).await? {
    ///         return Ok(file);
    ///     }
    ///     let page = orders().page(Order::query(), &request).await?;
    ///     Ok(view("orders/index.html", context! { orders => page }).into_response())
    /// }
    /// ```
    ///
    /// Custom columns are left out, and so are the columns the user hid on
    /// wide screens. Up to [`MAX_EXPORT_ROWS`] rows.
    pub async fn export<M: Model + Serialize>(
        &self,
        query: Query<M>,
        request: &GridRequest,
    ) -> Result<Option<Response>> {
        let Some(format) = request.param("export") else {
            return Ok(None);
        };
        let prefs = load_prefs(request, &self.id).await;
        let wide = prefs
            .wide
            .clone()
            .unwrap_or_else(|| self.default_visible(false));
        let columns: Vec<(&Column, Option<Pin>)> = self
            .ordered(&prefs)
            .into_iter()
            .filter(|(c, _)| c.kind != Kind::Custom && wide.contains(&c.key))
            .collect();
        let items = self
            .filter(query, request)
            .limit(MAX_EXPORT_ROWS)
            .get(&request.db)
            .await?;
        let rows: Vec<Map<String, Value>> = items
            .iter()
            .map(|item| match serde_json::to_value(item) {
                Ok(Value::Object(map)) => map,
                _ => Map::new(),
            })
            .collect();
        let today = crate::db::now().format("%Y-%m-%d").to_string();
        let name = format!("{}-{today}", self.id);
        let table = Table {
            columns: &columns,
            rows: &rows,
            request,
        };
        let response = match format {
            "csv" => crate::Download::bytes(
                format!("{name}.csv"),
                "text/csv; charset=utf-8",
                table.csv(),
            )
            .into_response(),
            "xlsx" => xlsx(&table, &name)?,
            "print" => crate::view(
                "renox/grid_print.html",
                minijinja::context! {
                    title => self.title.clone().unwrap_or_else(|| self.id.clone()),
                    header => header_rows(&columns),
                    columns => columns.iter().map(|(c, _)| json!({
                        "key": c.key,
                        "label": c.label,
                        "numeric": matches!(c.kind, Kind::Number | Kind::Money),
                    })).collect::<Vec<_>>(),
                    rows => rows.iter().map(|row| columns.iter().map(|(c, _)| table.display(c, row.get(&c.key))).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    total => rows.len(),
                    printed => crate::db::now().to_rfc3339(),
                    back => format!("{}?{}", request.path, serde_html_form::to_string(
                        request.params.iter().filter(|(k, _)| k != "export").collect::<Vec<_>>()
                    ).unwrap_or_default()),
                },
            )
            .into_response(),
            other => {
                return Err(Error::BadRequest(format!(
                    "`{other}` isn't an export (csv, xlsx, print)"
                )));
            }
        };
        Ok(Some(response))
    }
}

/// The exported rows with their columns.
struct Table<'a> {
    columns: &'a [(&'a Column, Option<Pin>)],
    rows: &'a [Map<String, Value>],
    request: &'a GridRequest,
}

impl Table<'_> {
    fn yes_no(&self, yes: bool) -> String {
        let key = if yes { "ui.grid.yes" } else { "ui.grid.no" };
        match &self.request.lang {
            Some(lang) => lang.t(key, &[]),
            None => (if yes { "Yes" } else { "No" }).to_owned(),
        }
    }

    /// One heading per column: its groups and its label, `Amounts / Total`.
    fn heading(column: &Column) -> String {
        let mut parts = column.under.clone();
        parts.push(column.label.clone());
        parts.join(" / ")
    }

    /// A value as text, the way the grid shows it (without number
    /// separators, so spreadsheets read numbers).
    fn text(&self, column: &Column, value: Option<&Value>) -> String {
        let label = |v: &str| {
            column
                .options
                .iter()
                .find(|(o, _)| o == v)
                .map_or_else(|| v.to_owned(), |(_, l)| l.clone())
        };
        match value {
            None | Some(Value::Null) => String::new(),
            Some(Value::Bool(b)) => self.yes_no(*b),
            Some(Value::Number(n)) if column.kind == Kind::Bool => {
                self.yes_no(n.as_i64() != Some(0))
            }
            Some(Value::Number(n)) => match (column.decimals, n.as_f64()) {
                (Some(d), Some(f)) => format!("{f:.*}", usize::from(d)),
                _ => n.to_string(),
            },
            Some(Value::String(s)) => match column.kind {
                Kind::Select => label(s),
                Kind::DateTime => self.moment(s).unwrap_or_else(|| s.clone()),
                _ => s.clone(),
            },
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| v.as_str().map_or_else(|| v.to_string(), label))
                .collect::<Vec<_>>()
                .join(", "),
            Some(other) => other.to_string(),
        }
    }

    /// A value for people to read: numbers with the locale's separators.
    fn display(&self, column: &Column, value: Option<&Value>) -> String {
        match (column.kind, value.and_then(Value::as_f64)) {
            (Kind::Number | Kind::Money, Some(n)) if column.kind != Kind::Bool => {
                let locale = self
                    .request
                    .lang
                    .as_ref()
                    .map_or("en", |l| l.locale.as_str());
                crate::view_filters::format_number(n, column.decimals.map_or(0, u32::from), locale)
            }
            _ => self.text(column, value),
        }
    }

    /// A moment in `APP_TIMEZONE`: `2026-03-05 14:30`.
    fn moment(&self, text: &str) -> Option<String> {
        let at = chrono::DateTime::parse_from_rfc3339(text).ok()?;
        let local = self.request.zone.local(at.timestamp());
        Some(local.format("%Y-%m-%d %H:%M").to_string())
    }

    /// RFC 4180 CSV with a BOM (so Excel reads UTF-8). Text that a
    /// spreadsheet would run as a formula (`=`, `+`, `-`, `@` first) gets a
    /// `'` in front.
    fn csv(&self) -> Vec<u8> {
        let mut out = String::from('\u{feff}');
        let line = |cells: Vec<String>| -> String {
            cells
                .into_iter()
                .map(|cell| {
                    if cell.contains([',', '"', '\n', '\r']) {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
                + "\r\n"
        };
        out.push_str(&line(
            self.columns.iter().map(|(c, _)| Self::heading(c)).collect(),
        ));
        for row in self.rows {
            out.push_str(&line(
                self.columns
                    .iter()
                    .map(|(c, _)| {
                        let text = self.text(c, row.get(&c.key));
                        let numeric = matches!(c.kind, Kind::Number | Kind::Money);
                        if !numeric && text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                            format!("'{text}")
                        } else {
                            text
                        }
                    })
                    .collect(),
            ));
        }
        out.into_bytes()
    }
}

#[cfg(feature = "xlsx")]
fn xlsx(table: &Table<'_>, name: &str) -> Result<Response> {
    use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    let fail = |e: rust_xlsxwriter::XlsxError| Error::Internal(anyhow::anyhow!(e));
    let head = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_background_color("#F2F2F7");
    // The headings as on screen: groups span their columns, leaves span
    // down to the last heading row.
    let header = header_rows(table.columns);
    let depth = header.len() as u32;
    let mut column_of: std::collections::HashMap<String, u16> = Default::default();
    for (i, (c, _)) in table.columns.iter().enumerate() {
        column_of.insert(c.key.clone(), i as u16);
    }
    for (r, cells) in header.iter().enumerate() {
        for cell in cells {
            let keys: Vec<u16> = cell["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k.as_str().and_then(|k| column_of.get(k).copied()))
                .collect();
            let (Some(&first), Some(&last)) = (keys.iter().min(), keys.iter().max()) else {
                continue;
            };
            let label = match cell["key"].as_str() {
                Some(key) => table
                    .columns
                    .iter()
                    .find(|(c, _)| c.key == key)
                    .map(|(c, _)| c.label.clone())
                    .unwrap_or_default(),
                None => cell["label"].as_str().unwrap_or_default().to_owned(),
            };
            let row = r as u32;
            let bottom = row + cell["rowspan"].as_u64().unwrap_or(1) as u32 - 1;
            if first == last && row == bottom {
                sheet
                    .write_string_with_format(row, first, &label, &head)
                    .map_err(fail)?;
            } else {
                sheet
                    .merge_range(row, first, bottom, last, &label, &head)
                    .map_err(fail)?;
            }
        }
    }
    let money = Format::new().set_num_format("#,##0");
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let moment = Format::new().set_num_format("yyyy-mm-dd hh:mm");
    for (i, row) in table.rows.iter().enumerate() {
        let r = depth + i as u32;
        for (c, (column, _)) in table.columns.iter().enumerate() {
            let c = c as u16;
            let value = row.get(&column.key);
            match (column.kind, value) {
                (_, None | Some(Value::Null)) => {}
                (Kind::Number | Kind::Money, Some(Value::Number(n))) => {
                    let n = n.as_f64().unwrap_or_default();
                    match (column.kind, column.decimals) {
                        (Kind::Money, _) => sheet.write_number_with_format(r, c, n, &money),
                        (_, Some(d)) if d > 0 => sheet.write_number_with_format(
                            r,
                            c,
                            n,
                            &Format::new()
                                .set_num_format(format!("#,##0.{}", "0".repeat(usize::from(d)))),
                        ),
                        _ => sheet.write_number(r, c, n),
                    }
                    .map_err(fail)?;
                }
                (Kind::Date, Some(Value::String(s))) => {
                    match s.parse::<chrono::NaiveDate>() {
                        Ok(d) => sheet.write_datetime_with_format(r, c, d, &date),
                        Err(_) => sheet.write_string(r, c, s),
                    }
                    .map_err(fail)?;
                }
                (Kind::DateTime, Some(Value::String(s))) => {
                    match chrono::DateTime::parse_from_rfc3339(s) {
                        Ok(at) => {
                            let local = table.request.zone.local(at.timestamp());
                            sheet.write_datetime_with_format(r, c, local, &moment)
                        }
                        Err(_) => sheet.write_string(r, c, s),
                    }
                    .map_err(fail)?;
                }
                _ => {
                    sheet
                        .write_string(r, c, table.text(column, value))
                        .map_err(fail)?;
                }
            }
        }
    }
    // Headings and the frozen-left columns stay put, as in the grid.
    let frozen = table
        .columns
        .iter()
        .take_while(|(_, pin)| *pin == Some(Pin::Left))
        .count() as u16;
    sheet.set_freeze_panes(depth, frozen).map_err(fail)?;
    sheet.autofit();
    let bytes = book.save_to_buffer().map_err(fail)?;
    Ok(crate::Download::bytes(
        format!("{name}.xlsx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        bytes,
    )
    .into_response())
}

#[cfg(not(feature = "xlsx"))]
fn xlsx(_: &Table<'_>, _: &str) -> Result<Response> {
    Err(Error::BadRequest(
        "Excel exports need renox's `xlsx` feature".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_links_keep_the_filters() {
        let urls = urls(
            "/orders",
            &[
                ("q.name".into(), "kopi susu".into()),
                ("per_page".into(), "25".into()),
            ],
        );
        assert_eq!(urls["csv"], "/orders?q.name=kopi+susu&export=csv");
        assert_eq!(urls["print"], "/orders?q.name=kopi+susu&export=print");
        assert_eq!(urls["xlsx"].is_string(), cfg!(feature = "xlsx"));
    }
}

//! Filters every template gets: `number`, `money`, `date`, `since`,
//! `words` and `markdown`.

use minijinja::value::Kwargs;
use minijinja::{Error, ErrorKind, State, Value};

/// The request's locale (`app.locale`), or `en`.
fn locale(state: &State) -> String {
    state
        .lookup("app")
        .and_then(|app| app.get_attr("locale").ok())
        .and_then(|l| l.as_str().map(str::to_owned))
        .unwrap_or_else(|| "en".into())
}

/// Thousands and decimal separators for a locale.
fn separators(locale: &str) -> (&'static str, &'static str) {
    let language = locale.split(['-', '_']).next().unwrap_or(locale);
    match language {
        "id" | "de" | "nl" | "es" | "it" | "pt" | "tr" | "da" | "nb" | "ms" | "vi" => (".", ","),
        "fr" | "ru" | "pl" | "cs" | "sv" | "fi" | "uk" => ("\u{202f}", ","),
        _ => (",", "."),
    }
}

/// `{{ 75000 | number }}` → `75,000` (en) or `75.000` (es, de);
/// `{{ 3.14159 | number(2) }}` → `3.14` / `3,14`.
pub(crate) fn number(state: &State, value: Value, decimals: Option<u32>) -> Result<String, Error> {
    let n = to_number("number", &value)?;
    Ok(format_number(n, decimals.unwrap_or(0), &locale(state)))
}

/// A template value as a number: an integer, a float or numeric text.
fn to_number(filter: &str, value: &Value) -> Result<f64, Error> {
    Ok(if let Ok(i) = i64::try_from(value.clone()) {
        i as f64
    } else if let Some(s) = value.as_str() {
        s.trim().parse().map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("{filter}: `{s}` is not a number"),
            )
        })?
    } else {
        f64::try_from(value.clone()).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("{filter}: `{value}` is not a number"),
            )
        })?
    })
}

/// `{{ order.total | money }}` → `Rp 75.000` with `APP_CURRENCY=IDR`,
/// `$75.00` with `USD`, with the page's separators. Keywords: `currency`
/// (another ISO 4217 code for this amount), `decimals`, and `divide_by`
/// for amounts kept in cents (`divide_by=100`).
pub(crate) fn money(
    currency: String,
) -> impl Fn(&State, Value, Kwargs) -> Result<String, Error> + Send + Sync + 'static {
    move |state: &State, value: Value, kwargs: Kwargs| {
        let code: Option<String> = kwargs.get("currency")?;
        let decimals: Option<u32> = kwargs.get("decimals")?;
        let divide_by: Option<f64> = kwargs.get("divide_by")?;
        kwargs.assert_all_used()?;
        let mut amount = to_number("money", &value)?;
        if let Some(divisor) = divide_by.filter(|d| *d != 0.0) {
            amount /= divisor;
        }
        let code = code.map_or_else(|| currency.clone(), |c| c.trim().to_ascii_uppercase());
        Ok(format_money(amount, &code, decimals, &locale(state)))
    }
}

/// `amount` in the currency `code` (ISO 4217), with its symbol, its usual
/// decimals (or `decimals`) and the locale's separators: `Rp 75.000`,
/// `$1,250.50`, `€1.250,50` in `de` (what the `money` template filter
/// uses). An unknown code is written before the amount (`CHF 12.00`).
pub fn format_money(amount: f64, code: &str, decimals: Option<u32>, locale: &str) -> String {
    let (symbol, usual) = currency(code);
    let figures = format_number(amount.abs(), decimals.unwrap_or(usual), locale);
    let sign = if amount < 0.0 && figures.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    // `Rp 75.000`, `RM 12.00`, but `$75.00`.
    let space = if symbol.ends_with(|c: char| c.is_ascii_alphabetic()) {
        " "
    } else {
        ""
    };
    format!("{sign}{symbol}{space}{figures}")
}

/// The decimals a currency is usually written with: 2 for `USD` or `AED`,
/// 0 for `IDR` or `JPY`. An amount in the smallest unit is divided by
/// `10^decimals` to give whole units.
pub(crate) fn currency_decimals(code: &str) -> u32 {
    currency(code).1
}

/// A currency's symbol and usual decimals (the code itself and 2 when it
/// isn't one of the listed ones).
fn currency(code: &str) -> (&str, u32) {
    match code {
        "IDR" => ("Rp", 0),
        "USD" => ("$", 2),
        "EUR" => ("€", 2),
        "GBP" => ("£", 2),
        "JPY" => ("¥", 0),
        "CNY" => ("CN¥", 2),
        "SGD" => ("S$", 2),
        "MYR" => ("RM", 2),
        "AUD" => ("A$", 2),
        "CAD" => ("CA$", 2),
        "INR" => ("₹", 2),
        "KRW" => ("₩", 0),
        "THB" => ("฿", 2),
        "PHP" => ("₱", 2),
        "VND" => ("₫", 0),
        other => (other, 2),
    }
}

/// `n` with `decimals` decimals and the locale's separators: `75.000` in
/// `id`, `75,000` in `en` (what the `number` template filter uses).
pub fn format_number(n: f64, decimals: u32, locale: &str) -> String {
    let (thousands, decimal) = separators(locale);
    let fixed = format!("{:.*}", decimals as usize, n.abs());
    let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut grouped = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push_str(thousands);
        }
        grouped.push(digit);
    }
    let sign = if n < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{decimal}{fraction}")
    }
}

/// `{{ order.created_at | date }}` → `2026-10-01`; `date("%d/%m/%Y %H:%M")`
/// with chrono's format codes. A moment (`created_at`, …) is shown in
/// `APP_TIMEZONE`; a date or a local date-time is shown as it is.
pub(crate) fn date(
    zone: crate::timezone::Zone,
) -> impl Fn(Value, Option<String>, Kwargs) -> Result<String, Error> + Send + Sync + 'static {
    move |value: Value, format: Option<String>, kwargs: Kwargs| {
        kwargs.assert_all_used()?;
        let text = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        let format = format.as_deref().unwrap_or("%Y-%m-%d");
        let formatted = if let Ok(moment) = chrono::DateTime::parse_from_rfc3339(&text) {
            let offset = zone.fixed_at(moment.timestamp());
            moment.with_timezone(&offset).format(format).to_string()
        } else if let Ok(local) = text.parse::<chrono::NaiveDateTime>() {
            local.format(format).to_string()
        } else if let Ok(day) = text.parse::<chrono::NaiveDate>() {
            day.format(format).to_string()
        } else {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("date: `{text}` is not a date"),
            ));
        };
        Ok(formatted)
    }
}

/// A moment from a template value: an RFC 3339 time, or a local date-time
/// or date (both in `zone`).
fn moment(value: &Value, zone: crate::timezone::Zone) -> Option<i64> {
    let text = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    if let Ok(moment) = chrono::DateTime::parse_from_rfc3339(&text) {
        return Some(moment.timestamp());
    }
    let local = text.parse::<chrono::NaiveDateTime>().ok().or_else(|| {
        text.parse::<chrono::NaiveDate>()
            .ok()
            .and_then(|day| day.and_hms_opt(0, 0, 0))
    })?;
    zone.resolve(local)
        .or_else(|| Some(local.and_utc().timestamp()))
}

/// `{{ order.created_at | since }}` → `3 hours ago`, `in 2 days`, `just
/// now` (`3 hours ago`), from the clock `TestApp::travel`
/// moves. The texts are `ui.since.*` translations.
pub(crate) fn since(
    zone: crate::timezone::Zone,
) -> impl Fn(&State, Value) -> Result<String, Error> + Send + Sync + 'static {
    move |state: &State, value: Value| {
        let then = moment(&value, zone).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("since: `{value}` is not a date"),
            )
        })?;
        let seconds = crate::clock::unix_secs() - then;
        let (key, count) = since_unit(seconds.unsigned_abs());
        let text = |key: &str, args: &[(&str, Value)]| -> String {
            let kwargs = minijinja::value::Kwargs::from_iter(
                args.iter().map(|(k, v)| ((*k).to_owned(), v.clone())),
            );
            state
                .lookup("t")
                .and_then(|t| t.call(state, &[Value::from(key), Value::from(kwargs)]).ok())
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| {
                    let params: Vec<(&str, String)> = args
                        .iter()
                        .filter(|(k, _)| *k != "count")
                        .map(|(k, v)| (*k, v.to_string()))
                        .collect();
                    let count = args
                        .iter()
                        .find(|(k, _)| *k == "count")
                        .and_then(|(_, v)| i64::try_from(v.clone()).ok());
                    crate::i18n::format(&crate::i18n::builtin_text("en", key), &params, count)
                })
        };
        let Some(unit) = key else {
            return Ok(text("ui.since.now", &[]));
        };
        let time = text(unit, &[("count", Value::from(count))]);
        let side = if seconds >= 0 {
            "ui.since.past"
        } else {
            "ui.since.future"
        };
        Ok(text(side, &[("time", Value::from(time))]))
    }
}

/// The unit and count for a distance in seconds; `None` under 45 seconds.
fn since_unit(seconds: u64) -> (Option<&'static str>, i64) {
    let round = |n: f64| (n.round() as i64).max(1);
    let s = seconds as f64;
    match seconds {
        0..45 => (None, 0),
        45..2_700 => (Some("ui.since.minutes"), round(s / 60.0)),
        2_700..79_200 => (Some("ui.since.hours"), round(s / 3_600.0)),
        79_200..2_246_400 => (Some("ui.since.days"), round(s / 86_400.0)),
        2_246_400..27_648_000 => (Some("ui.since.months"), round(s / 2_629_746.0)),
        _ => (Some("ui.since.years"), round(s / 31_556_952.0)),
    }
}

/// `{{ post.summary | words(20) }}`: the first 20 words, then `…` (or
/// `end="…"`) when there were more.
pub(crate) fn words(value: Value, count: usize, kwargs: Kwargs) -> Result<String, Error> {
    let end: Option<String> = kwargs.get("end")?;
    kwargs.assert_all_used()?;
    let text = value.as_str().map(str::to_owned).unwrap_or_else(|| {
        if value.is_none() || value.is_undefined() {
            String::new()
        } else {
            value.to_string()
        }
    });
    let all: Vec<&str> = text.split_whitespace().collect();
    if all.len() <= count {
        return Ok(all.join(" "));
    }
    Ok(format!(
        "{}{}",
        all[..count].join(" "),
        end.as_deref().unwrap_or("…")
    ))
}

/// `{{ product.description | markdown }}`: Markdown (CommonMark with
/// tables, strikethrough and task lists) as HTML. Safe for text people
/// typed: HTML in it is shown as text, and links or images to anything but
/// http(s), mailto, tel or a relative URL point nowhere.
pub(crate) fn markdown(value: Value) -> Value {
    let text = value.as_str().map(str::to_owned).unwrap_or_else(|| {
        if value.is_none() || value.is_undefined() {
            String::new()
        } else {
            value.to_string()
        }
    });
    Value::from_safe_string(markdown_html(&text))
}

fn markdown_html(text: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events = Parser::new_ext(text, options).map(|event| match event {
        Event::Html(html) | Event::InlineHtml(html) => Event::Text(html),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        other => other,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

/// The URL if it is http(s), mailto, tel or relative, else `#`.
fn safe_url(url: pulldown_cmark::CowStr<'_>) -> pulldown_cmark::CowStr<'_> {
    // Browsers ignore control characters and spaces inside a scheme.
    let cleaned: String = url
        .chars()
        .filter(|c| !c.is_ascii_control() && !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let scheme_end = cleaned.find(':');
    let before_path = cleaned.find(['/', '?', '#']).unwrap_or(usize::MAX);
    match scheme_end {
        Some(end) if end < before_path => {
            if matches!(&cleaned[..end], "http" | "https" | "mailto" | "tel") {
                url
            } else {
                "#".into()
            }
        }
        _ => url,
    }
}

/// `class_names('btn', {'btn-active': active, 'hidden': not shown})`: the
/// strings given, plus the keys of maps whose value is true, joined with
/// spaces (Blade's `@class`). Empty strings and `none` are skipped.
///
/// ```text
/// <a class="{{ class_names('tab', {'tab-active': route_is('orders.*')}) }}">Orders</a>
/// ```
pub(crate) fn class_names(parts: minijinja::value::Rest<minijinja::Value>) -> String {
    let mut classes: Vec<String> = Vec::new();
    for part in parts.iter() {
        if let Some(text) = part.as_str() {
            classes.extend(text.split_whitespace().map(str::to_owned));
        } else if part.kind() == minijinja::value::ValueKind::Map
            && let Ok(keys) = part.try_iter()
        {
            for key in keys {
                let on = part.get_item(&key).is_ok_and(|v| v.is_true());
                if on && let Some(name) = key.as_str() {
                    classes.extend(name.split_whitespace().map(str::to_owned));
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    classes.retain(|class| seen.insert(class.clone()));
    classes.join(" ")
}

/// `{{ sparkline(values) }}`: a small chart of numbers as inline SVG, for a
/// grid cell or a card. `kind="bars"` draws bars instead of a line;
/// `width`/`height` in pixels (96×28). It takes the text color, so a
/// `class="rx-up"` around it colors it.
///
/// ```text
/// {{ sparkline(row.trend) }}  {{ sparkline([3, 5, 2, 8], kind="bars", width=60) }}
/// ```
pub(crate) fn sparkline(
    values: minijinja::Value,
    kwargs: minijinja::value::Kwargs,
) -> Result<minijinja::Value, minijinja::Error> {
    let kind: Option<String> = kwargs.get("kind")?;
    let width: Option<f64> = kwargs.get("width")?;
    let height: Option<f64> = kwargs.get("height")?;
    let label: Option<String> = kwargs.get("label")?;
    kwargs.assert_all_used()?;
    let (w, h) = (
        width.unwrap_or(96.0).max(8.0),
        height.unwrap_or(28.0).max(8.0),
    );
    let points: Vec<f64> = match values.try_iter() {
        Ok(iter) => iter.filter_map(|v| f64::try_from(v).ok()).collect(),
        Err(_) => Vec::new(),
    };
    Ok(minijinja::Value::from_safe_string(spark_svg(
        &points,
        kind.as_deref() == Some("bars"),
        w,
        h,
        label.as_deref(),
    )))
}

fn spark_svg(points: &[f64], bars: bool, w: f64, h: f64, label: Option<&str>) -> String {
    let fmt = |v: f64| format!("{:.1}", v);
    let aria = match (label, points.first(), points.last()) {
        (Some(label), ..) => label.to_owned(),
        (None, Some(first), Some(last)) => format!("{} → {}", trim(*first), trim(*last)),
        _ => String::new(),
    };
    let aria = aria
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;");
    let mut svg = format!(
        "<svg class=\"rx-spark\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" aria-label=\"{aria}\">"
    );
    if !points.is_empty() {
        let (lo, hi) = points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        let span = if hi > lo { hi - lo } else { 1.0 };
        let pad = 2.0;
        let y = |v: f64| h - pad - (v - lo) / span * (h - 2.0 * pad);
        if bars {
            let step = w / points.len() as f64;
            let base = if lo < 0.0 {
                y(0.0_f64.clamp(lo, hi))
            } else {
                h - pad
            };
            for (i, v) in points.iter().enumerate() {
                let top = if lo >= 0.0 && hi == lo { pad } else { y(*v) };
                let (top, bottom) = if top < base { (top, base) } else { (base, top) };
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1\" fill=\"currentColor\"/>",
                    fmt(i as f64 * step + step * 0.15),
                    fmt(top),
                    fmt(step * 0.7),
                    fmt((bottom - top).max(1.0)),
                ));
            }
        } else {
            let step = if points.len() > 1 {
                (w - 2.0 * pad) / (points.len() - 1) as f64
            } else {
                0.0
            };
            let coords: Vec<String> = points
                .iter()
                .enumerate()
                .map(|(i, v)| format!("{},{}", fmt(pad + i as f64 * step), fmt(y(*v))))
                .collect();
            svg.push_str(&format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>",
                coords.join(" ")
            ));
            if let Some(last) = coords.last() {
                let (cx, cy) = last.split_once(',').unwrap_or(("0", "0"));
                svg.push_str(&format!(
                    "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"2\" fill=\"currentColor\"/>"
                ));
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

fn trim(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparklines_draw_lines_and_bars() {
        let line = spark_svg(&[1.0, 3.0, 2.0], false, 96.0, 28.0, None);
        assert!(line.contains("<polyline points=\"2.0,"), "{line}");
        assert!(line.contains("aria-label=\"1 → 2\""));
        let bars = spark_svg(&[1.0, -2.0, 4.0], true, 60.0, 20.0, Some("Sales <q1>"));
        assert_eq!(bars.matches("<rect").count(), 3);
        assert!(bars.contains("aria-label=\"Sales &lt;q1>\""));
        assert!(spark_svg(&[], false, 96.0, 28.0, None).ends_with("></svg>"));
        assert!(spark_svg(&[5.0], false, 96.0, 28.0, None).contains("<circle"));
    }

    #[test]
    fn formats_money_per_currency_and_locale() {
        assert_eq!(format_money(75_000.0, "IDR", None, "es"), "Rp 75.000");
        assert_eq!(format_money(1_250.5, "USD", None, "en"), "$1,250.50");
        assert_eq!(format_money(1_250.5, "EUR", None, "de"), "€1.250,50");
        assert_eq!(format_money(-5_000.0, "IDR", None, "en"), "-Rp 5,000");
        assert_eq!(format_money(12.0, "CHF", None, "en"), "CHF 12.00");
        assert_eq!(format_money(12.0, "MYR", Some(0), "en"), "RM 12");
        assert_eq!(format_money(-0.001, "USD", None, "en"), "$0.00");
    }

    #[test]
    fn picks_units_for_distances() {
        assert_eq!(since_unit(10), (None, 0));
        assert_eq!(since_unit(60), (Some("ui.since.minutes"), 1));
        assert_eq!(since_unit(44 * 60), (Some("ui.since.minutes"), 44));
        assert_eq!(since_unit(3 * 3_600), (Some("ui.since.hours"), 3));
        assert_eq!(since_unit(23 * 3_600), (Some("ui.since.days"), 1));
        assert_eq!(since_unit(40 * 86_400), (Some("ui.since.months"), 1));
        assert_eq!(since_unit(400 * 86_400), (Some("ui.since.years"), 1));
    }

    #[test]
    fn markdown_keeps_html_and_scripts_out() {
        let html = markdown_html(
            "**Bold** <script>alert(1)</script>\n\n[x](javascript:alert(1)) [y](https://renox.dev) [z](/a:b) ![i](JaVaScRiPt:x) [w](%20data:text/html,x)",
        );
        assert!(html.contains("<strong>Bold</strong>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(
            !html.to_lowercase().contains("script:") && !html.contains("data:"),
            "{html}"
        );
        assert!(
            html.contains("href=\"https://renox.dev\"") && html.contains("href=\"/a:b\""),
            "{html}"
        );
        assert!(
            html.contains("href=\"#\"") && html.contains("src=\"#\""),
            "{html}"
        );
        let table = markdown_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n~~old~~");
        assert!(
            table.contains("<table>") && table.contains("checkbox") && table.contains("<del>"),
            "{table}"
        );
    }

    #[test]
    fn formats_numbers_per_locale() {
        assert_eq!(format_number(75_000.0, 0, "es"), "75.000");
        assert_eq!(format_number(1_234_567.891, 2, "en"), "1,234,567.89");
        assert_eq!(format_number(-1234.5, 1, "de"), "-1.234,5");
        assert_eq!(format_number(999.0, 0, "en"), "999");
        assert_eq!(format_number(-0.001, 2, "en"), "0.00");
    }
}

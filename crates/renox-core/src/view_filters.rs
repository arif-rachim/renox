//! Filters every template gets: `number` and `date`.

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

/// `{{ 75000 | number }}` → `75,000` (en) or `75.000` (id);
/// `{{ 3.14159 | number(2) }}` → `3.14` / `3,14`.
pub(crate) fn number(state: &State, value: Value, decimals: Option<u32>) -> Result<String, Error> {
    let n: f64 = if let Ok(i) = i64::try_from(value.clone()) {
        i as f64
    } else if let Some(s) = value.as_str() {
        s.trim().parse().map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("number: `{s}` is not a number"),
            )
        })?
    } else {
        f64::try_from(value.clone()).map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("number: `{value}` is not a number"),
            )
        })?
    };
    Ok(format_number(n, decimals.unwrap_or(0), &locale(state)))
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
    fn formats_numbers_per_locale() {
        assert_eq!(format_number(75_000.0, 0, "id"), "75.000");
        assert_eq!(format_number(1_234_567.891, 2, "en"), "1,234,567.89");
        assert_eq!(format_number(-1234.5, 1, "id"), "-1.234,5");
        assert_eq!(format_number(999.0, 0, "en"), "999");
        assert_eq!(format_number(-0.001, 2, "en"), "0.00");
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers_per_locale() {
        assert_eq!(format_number(75_000.0, 0, "id"), "75.000");
        assert_eq!(format_number(1_234_567.891, 2, "en"), "1,234,567.89");
        assert_eq!(format_number(-1234.5, 1, "id"), "-1.234,5");
        assert_eq!(format_number(999.0, 0, "en"), "999");
        assert_eq!(format_number(-0.001, 2, "en"), "0.00");
    }
}

use axum::body::Bytes;
use std::collections::HashMap;

use axum::extract::multipart::MultipartError;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::Method;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::messages::render;
use super::{Errors, Locale, Validate, ValidationError, Validator};
use crate::upload::{self, Upload};
use crate::{AppState, Error};

/// Deserializes and validates a form (urlencoded or multipart with `Upload`
/// fields), a JSON body, or the query string for GET, with the type's
/// `Validate` rules.
///
/// On failure, HTMX and JSON requests get `422` with the errors as JSON (the
/// bundled script shows them next to the form's inputs); other requests are
/// redirected back with the errors and old input flashed.
///
/// Empty form fields count as missing, like Laravel: use `Option<T>` for
/// optional fields. A missing required field or a value of the wrong type
/// becomes a validation error rather than a 400.
pub struct Valid<T>(pub T);

/// Built-in messages in the request's language, with the app's overrides.
struct Messages {
    locale: Locale,
    texts: crate::i18n::Texts,
}

impl Messages {
    fn template(&self, key: &str) -> std::borrow::Cow<'static, str> {
        super::messages::template_for(self.locale, Some(&self.texts), key)
    }

    fn label(&self, field: &str) -> String {
        self.texts
            .get(&format!("renox.validation.attributes.{field}"))
            .cloned()
            .unwrap_or_else(|| field.replace('_', " "))
    }
}

enum Parsed<T> {
    /// Parsed, possibly with placeholders standing in for fields that
    /// didn't parse; `Errors` holds those fields' errors.
    Ok(T, Errors),
    Invalid(Errors),
}

impl<T> FromRequest<AppState> for Valid<T>
where
    T: DeserializeOwned + Validate + Send,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Response> {
        let locale_name = crate::i18n::request_locale(req.extensions(), state);
        let locale = &Messages {
            locale: Locale::parse(&locale_name),
            texts: state.translator.texts(&locale_name),
        };
        let is_json = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));

        let is_multipart = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("multipart/form-data"));

        let (parsed, input) = if matches!(*req.method(), Method::GET | Method::HEAD) {
            let query = req.uri().query().unwrap_or_default().as_bytes();
            let pairs = form_urlencoded::parse(query).into_owned().collect();
            parse_pairs(pairs, &HashMap::new(), locale)
        } else if is_multipart {
            let multipart = Multipart::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            // MultipartError keeps axum's status, e.g. 413 over UPLOAD_MAX_SIZE.
            let (pairs, uploads) = read_multipart(multipart)
                .await
                .map_err(IntoResponse::into_response)?;
            parse_pairs(pairs, &uploads, locale)
        } else {
            let bytes = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            if is_json {
                parse_json(&bytes, locale).map_err(IntoResponse::into_response)?
            } else {
                let pairs = form_urlencoded::parse(&bytes).into_owned().collect();
                parse_pairs(pairs, &HashMap::new(), locale)
            }
        };

        let (data, mut errors) = match parsed {
            Parsed::Ok(data, errors) => (data, errors),
            Parsed::Invalid(errors) => {
                return Err(ValidationError::new(errors)
                    .with_input_map(input)
                    .into_response());
            }
        };
        let rule_errors = Validator::rules_with_texts(&data, locale.locale, locale.texts.clone())
            .finish(&state.db)
            .await
            .map_err(IntoResponse::into_response)?;
        // A field that didn't parse was checked with a placeholder; its own
        // error is the one to show.
        for (field, messages) in rule_errors.iter() {
            if !errors.has(field) {
                for message in messages {
                    errors.add(field, message.clone());
                }
            }
        }
        if errors.is_empty() {
            Ok(Valid(data))
        } else {
            Err(ValidationError::new(errors)
                .with_input_map(input)
                .into_response())
        }
    }
}

/// Text fields as pairs, and files replaced by tokens that `Upload`'s
/// `Deserialize` resolves. An empty file input counts as missing.
async fn read_multipart(
    mut multipart: Multipart,
) -> Result<(Vec<(String, String)>, HashMap<String, Upload>), MultipartError> {
    let mut pairs = Vec::new();
    let mut uploads = HashMap::new();
    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name().map(str::to_owned) else {
            continue;
        };
        match field.file_name().map(str::to_owned) {
            Some(file_name) => {
                let content_type = field.content_type().unwrap_or_default().to_owned();
                let bytes = field.bytes().await?;
                if file_name.is_empty() && bytes.is_empty() {
                    continue;
                }
                let token = upload::token(uploads.len());
                uploads.insert(
                    token.clone(),
                    Upload {
                        file_name,
                        content_type,
                        bytes,
                    },
                );
                pairs.push((name, token));
            }
            None => pairs.push((name, field.text().await?)),
        }
    }
    Ok((pairs, uploads))
}

fn parse_pairs<T: DeserializeOwned>(
    pairs: Vec<(String, String)>,
    uploads: &HashMap<String, Upload>,
    locale: &Messages,
) -> (Parsed<T>, Map<String, Value>) {
    let mut input = Map::new();
    for (key, value) in pairs.iter().filter(|(_, v)| !uploads.contains_key(v)) {
        match input.get_mut(key) {
            Some(Value::Array(values)) => values.push(Value::String(value.clone())),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, Value::String(value.clone())]);
            }
            None => {
                input.insert(key.clone(), Value::String(value.clone()));
            }
        }
    }

    // Empty inputs are dropped so `Option<T>` fields become `None`. A field
    // serde then reports missing is put back as "", so text fields still reach
    // the rules (and their labels); a number left blank becomes "required".
    //
    // A field that doesn't parse (`price=abc` for an i64) gets its error, then
    // a placeholder so the rest of the form still parses and every other
    // field's rules run too: all errors show at once.
    let mut filled: Vec<(String, String)> = pairs
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .cloned()
        .collect();
    let mut errors = Errors::new();
    let mut tries: HashMap<String, usize> = HashMap::new();
    let mut coerced: std::collections::HashSet<String> = std::collections::HashSet::new();
    let parsed = loop {
        let encoded = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&filled)
            .finish();
        // serde_html_form, unlike serde_urlencoded, reads repeated names (a
        // multi-select, a group of checkboxes) into a `Vec`.
        let deserializer = serde_html_form::Deserializer::from_bytes(encoded.as_bytes());
        match upload::with_uploads(uploads, || serde_path_to_error::deserialize(deserializer)) {
            Ok(data) => break Parsed::Ok(data, errors),
            Err(err) => {
                let message = err.inner().to_string();
                if let Some(field) = missing_field(&message)
                    && !filled.iter().any(|(k, _)| k == field)
                {
                    filled.push((field.to_owned(), String::new()));
                    continue;
                }
                let path = err.path().to_string();
                if coerce_browser_value(&mut filled, &path, &message, &mut coerced) {
                    continue;
                }
                let blank = filled.iter().any(|(k, v)| *k == path && v.is_empty());
                let tried = tries.entry(path.clone()).or_default();
                if *tried == 0 {
                    for (field, messages) in field_error(&path, &message, blank, locale).iter() {
                        for message in messages {
                            errors.add(field, message.clone());
                        }
                    }
                }
                // Stand-ins that parse as most field types: an enum's first
                // variant (named in the error), numbers and text, booleans.
                let placeholder = match *tried {
                    0 => expected_variant(&message)
                        .or_else(|| PLACEHOLDERS.first().map(|p| (*p).to_owned())),
                    n => PLACEHOLDERS.get(n).map(|p| (*p).to_owned()),
                };
                *tried += 1;
                match placeholder {
                    Some(value) if filled.iter().any(|(k, _)| *k == path) => {
                        filled.retain(|(k, _)| *k != path);
                        filled.push((path, value));
                    }
                    _ => break Parsed::Invalid(errors),
                }
            }
        }
    };
    (parsed, input)
}

const PLACEHOLDERS: &[&str] = &["0", "false"];

/// The first valid value an enum's error names: serde's "unknown variant
/// `x`, expected one of `a`, `b`" or DbEnum's "expected one of: a, b".
fn expected_variant(message: &str) -> Option<String> {
    let rest = message.split("expected one of").nth(1)?;
    let rest = rest.trim_start_matches([':', ' ']);
    let first = rest.split(',').next()?.trim().trim_matches('`');
    (!first.is_empty()).then(|| first.to_owned())
}

/// Rewrites what browsers send into what Rust types parse, once per field:
/// a checkbox's `on` (or `1`, `yes`) is `true`, an unchecked one (missing)
/// is `false`; `<input type="datetime-local">` leaves out the seconds.
/// Returns whether `path` was rewritten, so deserializing can try again.
fn coerce_browser_value(
    filled: &mut [(String, String)],
    path: &str,
    message: &str,
    coerced: &mut std::collections::HashSet<String>,
) -> bool {
    if coerced.contains(path) {
        return false;
    }
    let mut changed = false;
    for (_, value) in filled.iter_mut().filter(|(k, _)| k == path) {
        let new = if message.contains("`true` or `false`") {
            match value.trim().to_ascii_lowercase().as_str() {
                "on" | "1" | "yes" | "checked" => Some("true".to_owned()),
                "" | "off" | "0" | "no" => Some("false".to_owned()),
                _ => None,
            }
        } else if is_minute_datetime(value) {
            Some(format!("{value}:00"))
        } else {
            None
        };
        if let Some(new) = new {
            *value = new;
            changed = true;
        }
    }
    if changed {
        coerced.insert(path.to_owned());
    }
    changed
}

/// `2026-10-01T10:30`, as `<input type="datetime-local">` sends it.
fn is_minute_datetime(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 16
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13) || c.is_ascii_digit())
}

fn parse_json<T: DeserializeOwned>(
    bytes: &[u8],
    locale: &Messages,
) -> Result<(Parsed<T>, Map<String, Value>), Error> {
    let input = match serde_json::from_slice(bytes) {
        Ok(Value::Object(map)) => map,
        Ok(_) => return Err(Error::BadRequest("The JSON body must be an object.".into())),
        Err(err) => return Err(Error::BadRequest(format!("Invalid JSON: {err}"))),
    };
    // As with forms: a missing field is put back as "" so its rules (and
    // `required`) run, and a field of the wrong type gets its error and a
    // placeholder, so every field's errors show at once.
    let mut body = input.clone();
    let mut errors = Errors::new();
    let mut tries: HashMap<String, usize> = HashMap::new();
    let parsed = loop {
        match serde_path_to_error::deserialize(Value::Object(body.clone())) {
            Ok(data) => break Parsed::Ok(data, errors),
            Err(err) => {
                let message = err.inner().to_string();
                if let Some(field) = missing_field(&message)
                    && !body.contains_key(field)
                {
                    body.insert(field.to_owned(), Value::String(String::new()));
                    continue;
                }
                let path = err.path().to_string();
                let blank = body.get(&path).is_some_and(|v| {
                    v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty())
                });
                let tried = tries.entry(path.clone()).or_default();
                if *tried == 0 {
                    for (field, messages) in field_error(&path, &message, blank, locale).iter() {
                        for message in messages {
                            errors.add(field, message.clone());
                        }
                    }
                }
                let placeholder = match (*tried, expected_variant(&message)) {
                    (0, Some(variant)) => Some(Value::String(variant)),
                    (n, _) => JSON_PLACEHOLDERS.get(n).map(|p| p()),
                };
                *tried += 1;
                match placeholder {
                    Some(value) if body.contains_key(&path) => {
                        body.insert(path, value);
                    }
                    _ => break Parsed::Invalid(errors),
                }
            }
        }
    };
    Ok((parsed, input))
}

/// JSON stand-ins for a field of the wrong type, tried in turn.
const JSON_PLACEHOLDERS: &[fn() -> Value] = &[
    || Value::from(0),
    || Value::Bool(false),
    || Value::String(String::new()),
    || Value::Null,
];

fn missing_field(message: &str) -> Option<&str> {
    message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.split('`').next())
}

/// Turns a deserialization error into a message for the field it concerns.
/// `blank` means the field was submitted empty.
fn field_error(path: &str, message: &str, blank: bool, locale: &Messages) -> Errors {
    let mut errors = Errors::new();
    if let Some(field) = missing_field(message) {
        let field = if path == "." {
            field.to_owned()
        } else {
            format!("{path}.{field}")
        };
        errors.add(
            &field,
            render(&locale.template("required"), &locale.label(&field), &[]),
        );
        return errors;
    }

    let field = if path == "." { "_form" } else { path };
    let numeric = [
        "invalid digit",
        "invalid float",
        "cannot parse integer",
        "number too large",
        "expected i",
        "expected u",
        "expected f",
    ]
    .iter()
    .any(|needle| message.contains(needle));
    let key = match (blank, numeric) {
        (true, _) => "required",
        _ if message.contains(upload::NOT_A_FILE) => "file",
        (false, true) => "numeric",
        (false, false) => "invalid",
    };
    errors.add(
        field,
        render(&locale.template(key), &locale.label(field), &[]),
    );
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    fn plain(locale: Locale) -> Messages {
        Messages {
            locale,
            texts: Default::default(),
        }
    }

    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Form {
        nama: String,
        harga: i64,
        catatan: Option<String>,
    }

    fn parse(body: &str) -> Result<Form, Errors> {
        let pairs = form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect();
        match parse_pairs::<Form>(pairs, &HashMap::new(), &plain(Locale::Id)).0 {
            Parsed::Ok(form, errors) if errors.is_empty() => Ok(form),
            Parsed::Ok(_, errors) | Parsed::Invalid(errors) => Err(errors),
        }
    }

    #[derive(Deserialize, Debug)]
    struct Browser {
        agree: bool,
        news: bool,
        starts_at: chrono::NaiveDateTime,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        sizes: Vec<i64>,
    }

    fn parse_browser(body: &str) -> Result<Browser, Errors> {
        let pairs = form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect();
        match parse_pairs::<Browser>(pairs, &HashMap::new(), &plain(Locale::En)).0 {
            Parsed::Ok(form, errors) if errors.is_empty() => Ok(form),
            Parsed::Ok(_, errors) | Parsed::Invalid(errors) => Err(errors),
        }
    }

    #[test]
    fn reads_what_browsers_send() {
        // A checked checkbox sends "on", an unchecked one nothing;
        // datetime-local has no seconds; multi-selects repeat the name.
        let form =
            parse_browser("agree=on&starts_at=2026-10-01T10%3A30&tags=a&tags=b&sizes=1&sizes=2")
                .unwrap();
        assert!(form.agree);
        assert!(!form.news);
        assert_eq!(form.starts_at.to_string(), "2026-10-01 10:30:00");
        assert_eq!(form.tags, ["a", "b"]);
        assert_eq!(form.sizes, [1, 2]);
        let form = parse_browser("agree=1&news=true&starts_at=2026-10-01T10%3A30%3A15").unwrap();
        assert!(form.agree && form.news);
        assert!(form.tags.is_empty());
        let errors = parse_browser("agree=maybe&starts_at=soon").unwrap_err();
        assert!(errors.has("agree") && errors.has("starts_at"));
    }

    #[test]
    fn empty_fields_are_missing() {
        let form = parse("nama=Kopi&harga=5&catatan=").unwrap();
        assert!(form.catatan.is_none());
        // Text left empty reaches the rules as "".
        assert_eq!(parse("nama=&harga=5").unwrap().nama, "");
        assert_eq!(parse("harga=5").unwrap().nama, "");
        let errors = parse("nama=Kopi&harga=").unwrap_err();
        assert_eq!(errors.first("harga"), Some("Harga wajib diisi."));
    }

    #[test]
    fn wrong_types_name_the_field() {
        let errors = parse("nama=Kopi&harga=murah").unwrap_err();
        assert_eq!(errors.first("harga"), Some("Harga harus berupa angka."));
    }

    #[test]
    fn keeps_every_input_for_old_values() {
        let pairs = form_urlencoded::parse(b"nama=Kopi&tag=a&tag=b&harga=")
            .into_owned()
            .collect();
        let (_, input) = parse_pairs::<Form>(pairs, &HashMap::new(), &plain(Locale::En));
        assert_eq!(input["nama"], "Kopi");
        assert_eq!(input["tag"], serde_json::json!(["a", "b"]));
        assert_eq!(input["harga"], "");
    }
}

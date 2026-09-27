use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::Method;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::messages::{render, template};
use super::{Errors, Locale, Validate, ValidationError, Validator};
use crate::{AppState, Error};

/// Deserializes and validates a form (or JSON body, or query string for GET)
/// with the type's `Validate` rules.
///
/// On failure, HTMX and JSON requests get `422` with the errors as JSON (the
/// bundled script shows them next to the form's inputs); other requests are
/// redirected back with the errors and old input flashed.
///
/// Empty form fields count as missing, like Laravel: use `Option<T>` for
/// optional fields. A missing required field or a value of the wrong type
/// becomes a validation error rather than a 400.
pub struct Valid<T>(pub T);

enum Parsed<T> {
    Ok(T),
    Invalid(Errors),
}

impl<T> FromRequest<AppState> for Valid<T>
where
    T: DeserializeOwned + Validate + Send,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Response> {
        let locale = Locale::parse(&state.config.locale);
        let is_json = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/json"));

        let (parsed, input) = if matches!(*req.method(), Method::GET | Method::HEAD) {
            let query = req.uri().query().unwrap_or_default().as_bytes().to_vec();
            parse_form(&query, locale)
        } else {
            let bytes = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            if is_json {
                parse_json(&bytes, locale).map_err(IntoResponse::into_response)?
            } else {
                parse_form(&bytes, locale)
            }
        };

        let data = match parsed {
            Parsed::Ok(data) => data,
            Parsed::Invalid(errors) => {
                return Err(ValidationError::new(errors)
                    .with_input_map(input)
                    .into_response());
            }
        };
        let errors = Validator::rules_of(&data, locale)
            .finish(&state.db)
            .await
            .map_err(IntoResponse::into_response)?;
        if errors.is_empty() {
            Ok(Valid(data))
        } else {
            Err(ValidationError::new(errors)
                .with_input_map(input)
                .into_response())
        }
    }
}

fn parse_form<T: DeserializeOwned>(
    bytes: &[u8],
    locale: Locale,
) -> (Parsed<T>, Map<String, Value>) {
    let pairs: Vec<(String, String)> = form_urlencoded::parse(bytes).into_owned().collect();

    let mut input = Map::new();
    for (key, value) in &pairs {
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
    let mut filled: Vec<(String, String)> = pairs
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .cloned()
        .collect();
    let parsed = loop {
        let encoded = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&filled)
            .finish();
        let deserializer =
            serde_urlencoded::Deserializer::new(form_urlencoded::parse(encoded.as_bytes()));
        match serde_path_to_error::deserialize(deserializer) {
            Ok(data) => break Parsed::Ok(data),
            Err(err) => {
                let message = err.inner().to_string();
                if let Some(field) = missing_field(&message)
                    && !filled.iter().any(|(k, _)| k == field)
                {
                    filled.push((field.to_owned(), String::new()));
                    continue;
                }
                let path = err.path().to_string();
                let blank = filled.iter().any(|(k, v)| *k == path && v.is_empty());
                break Parsed::Invalid(field_error(&path, &message, blank, locale));
            }
        }
    };
    (parsed, input)
}

fn parse_json<T: DeserializeOwned>(
    bytes: &[u8],
    locale: Locale,
) -> Result<(Parsed<T>, Map<String, Value>), Error> {
    let input = match serde_json::from_slice(bytes) {
        Ok(Value::Object(map)) => map,
        Ok(_) => return Err(Error::BadRequest("The JSON body must be an object.".into())),
        Err(err) => return Err(Error::BadRequest(format!("Invalid JSON: {err}"))),
    };
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = match serde_path_to_error::deserialize(&mut deserializer) {
        Ok(data) => Parsed::Ok(data),
        Err(err) => Parsed::Invalid(field_error(
            &err.path().to_string(),
            &err.inner().to_string(),
            false,
            locale,
        )),
    };
    Ok((parsed, input))
}

fn missing_field(message: &str) -> Option<&str> {
    message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.split('`').next())
}

/// Turns a deserialization error into a message for the field it concerns.
/// `blank` means the field was submitted empty.
fn field_error(path: &str, message: &str, blank: bool, locale: Locale) -> Errors {
    let mut errors = Errors::new();
    if let Some(field) = missing_field(message) {
        let field = if path == "." {
            field.to_owned()
        } else {
            format!("{path}.{field}")
        };
        errors.add(
            &field,
            render(template(locale, "required"), &field.replace('_', " "), &[]),
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
        (false, true) => "numeric",
        (false, false) => "invalid",
    };
    errors.add(
        field,
        render(template(locale, key), &field.replace('_', " "), &[]),
    );
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Form {
        nama: String,
        harga: i64,
        catatan: Option<String>,
    }

    fn parse(body: &str) -> Result<Form, Errors> {
        match parse_form::<Form>(body.as_bytes(), Locale::Id).0 {
            Parsed::Ok(form) => Ok(form),
            Parsed::Invalid(errors) => Err(errors),
        }
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
        let (_, input) = parse_form::<Form>(b"nama=Kopi&tag=a&tag=b&harga=", Locale::En);
        assert_eq!(input["nama"], "Kopi");
        assert_eq!(input["tag"], serde_json::json!(["a", "b"]));
        assert_eq!(input["harga"], "");
    }
}

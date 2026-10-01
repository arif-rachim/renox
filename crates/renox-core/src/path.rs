//! `Path`, axum's path extractor with a 404 for values that don't parse.

use std::ops::{Deref, DerefMut};

use axum::extract::FromRequestParts;
use axum::extract::path::ErrorKind;
use axum::extract::rejection::PathRejection;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::Error;

/// The route's parameters (`/orders/{id}` → `Path(id): Path<i64>`), like
/// axum's `Path`, except that a value that doesn't fit (`/orders/abc`, or an
/// id too large) is a 404 page, as for a route that doesn't exist, rather
/// than a plain-text 400.
///
/// ```
/// # use renox::prelude::*;
/// async fn show(Path(id): Path<i64>) -> String { format!("order {id}") }
/// async fn line(Path((order, line)): Path<(i64, i64)>) -> String { format!("{order}/{line}") }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Path<T>(pub T);

impl<T> Deref for Path<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Path<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Error> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(value)) => Ok(Path(value)),
            // A visitor's value that doesn't parse (`/products/abc`): not found.
            // A parameter count or type that can't work is the app's mistake.
            Err(PathRejection::FailedToDeserializePathParams(err)) => match err.kind() {
                ErrorKind::WrongNumberOfParameters { .. } | ErrorKind::UnsupportedType { .. } => {
                    Err(anyhow::anyhow!("{err}").into())
                }
                _ => Err(Error::NotFound),
            },
            // The route has no such parameters: a mistake in the app.
            Err(other) => Err(anyhow::anyhow!("{other}").into()),
        }
    }
}

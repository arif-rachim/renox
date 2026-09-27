use std::collections::HashMap;
use std::fmt::{Display, Write};

use std::convert::Infallible;

use anyhow::{anyhow, bail};
use axum::Router;
use axum::extract::Request;
use axum::handler::Handler;
use axum::middleware::from_fn;
use axum::response::IntoResponse;
use axum::routing::{self, MethodRouter, Route};
use tower::{Layer, Service};

use crate::AppState;

/// A module's routes, with optional names for URL generation.
///
/// ```ignore
/// Routes::new()
///     .get("/produk", index).name("produk.index")
///     .get("/produk/{id}", show).name("produk.show")
///     .post("/produk", store).name("produk.store")
/// ```
#[derive(Default)]
pub struct Routes {
    router: Router<AppState>,
    names: Vec<(String, String)>,
    last_path: Option<String>,
}

macro_rules! method {
    ($($method:ident),*) => {$(
        pub fn $method<H, T>(self, path: &str, handler: H) -> Self
        where
            H: Handler<T, AppState>,
            T: 'static,
        {
            self.route(path, routing::$method(handler))
        }
    )*};
}

impl Routes {
    pub fn new() -> Self {
        Self::default()
    }

    method!(get, post, put, patch, delete);

    /// Adds a route with any axum method router, e.g. `get(show).post(update)`.
    pub fn route(mut self, path: &str, method_router: MethodRouter<AppState>) -> Self {
        self.router = self.router.route(path, method_router);
        self.last_path = Some(path.to_owned());
        self
    }

    /// Names the route added just before, for use with `url()` and `route()` in templates.
    ///
    /// # Panics
    ///
    /// If no route has been added yet.
    pub fn name(mut self, name: &str) -> Self {
        let path = self
            .last_path
            .clone()
            .expect("Routes::name() must follow a route");
        self.names.push((name.to_owned(), path));
        self
    }

    /// Only logged-in users may use the routes added so far; guests are sent
    /// to the `login` route. Call it after adding the routes it should cover.
    pub fn require_auth(self) -> Self {
        self.route_layer(from_fn(crate::auth::require_auth))
    }

    /// Like `require_auth`, and the user must have verified their email;
    /// others are sent to the `verification.notice` route.
    pub fn require_verified(self) -> Self {
        self.route_layer(from_fn(crate::auth::require_verified))
    }

    /// Only guests may use the routes added so far; logged-in users are sent
    /// to the `home` route (e.g. for login and registration pages).
    pub fn guest_only(self) -> Self {
        self.route_layer(from_fn(crate::auth::guest_only))
    }

    /// Wraps the routes added so far in a tower layer (axum's `route_layer`).
    pub fn route_layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        self.router = self.router.route_layer(layer);
        self
    }

    pub fn merge(mut self, other: impl Into<Routes>) -> Self {
        let other = other.into();
        self.router = self.router.merge(other.router);
        self.names.extend(other.names);
        self.last_path = None;
        self
    }

    pub(crate) fn into_parts(self) -> (Router<AppState>, Vec<(String, String)>) {
        (self.router, self.names)
    }
}

impl From<Router<AppState>> for Routes {
    fn from(router: Router<AppState>) -> Self {
        Self {
            router,
            ..Self::default()
        }
    }
}

/// Every named route in the application.
#[derive(Debug, Default)]
pub struct RouteTable {
    paths: HashMap<String, String>,
}

impl RouteTable {
    pub(crate) fn insert(&mut self, name: String, path: String) -> anyhow::Result<()> {
        if let Some(existing) = self.paths.get(&name) {
            bail!("route name `{name}` is used for both `{existing}` and `{path}`");
        }
        self.paths.insert(name, path);
        Ok(())
    }

    /// The path pattern of a named route, e.g. `/produk/{id}`.
    pub fn path(&self, name: &str) -> Option<&str> {
        self.paths.get(name).map(String::as_str)
    }

    /// All named routes as `(name, path)`, sorted by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        let mut routes: Vec<_> = self
            .paths
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_str()))
            .collect();
        routes.sort();
        routes.into_iter()
    }

    /// Builds the URL path of a named route, filling its parameters in order.
    pub fn url(&self, name: &str, params: &[&dyn Display]) -> anyhow::Result<String> {
        let pattern = self
            .path(name)
            .ok_or_else(|| anyhow!("route `{name}` is not defined"))?;
        fill(pattern, params).map_err(|err| anyhow!("route `{name}`: {err}"))
    }
}

fn fill(pattern: &str, params: &[&dyn Display]) -> anyhow::Result<String> {
    let mut out = String::with_capacity(pattern.len());
    let mut params = params.iter();
    let mut rest = pattern;

    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find('}')
            .ok_or_else(|| anyhow!("unclosed `{{` in `{pattern}`"))?
            + start;
        let placeholder = &rest[start + 1..end];
        let value = params
            .next()
            .ok_or_else(|| anyhow!("missing value for `{{{placeholder}}}`"))?
            .to_string();
        encode(&mut out, &value, placeholder.starts_with('*'));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);

    if params.next().is_some() {
        bail!("too many parameters for `{pattern}`");
    }
    Ok(out)
}

pub(crate) fn encode(out: &mut String, value: &str, keep_slashes: bool) {
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            b'/' if keep_slashes => out.push('/'),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> RouteTable {
        let mut t = RouteTable::default();
        t.insert("home".into(), "/".into()).unwrap();
        t.insert("produk.show".into(), "/produk/{id}".into())
            .unwrap();
        t.insert("docs".into(), "/docs/{*path}".into()).unwrap();
        t
    }

    #[test]
    fn builds_urls() {
        let t = table();
        assert_eq!(t.url("home", &[]).unwrap(), "/");
        assert_eq!(t.url("produk.show", &[&42]).unwrap(), "/produk/42");
        assert_eq!(
            t.url("produk.show", &[&"a b/c"]).unwrap(),
            "/produk/a%20b%2Fc"
        );
        assert_eq!(
            t.url("docs", &[&"guide/intro"]).unwrap(),
            "/docs/guide/intro"
        );
    }

    #[test]
    fn rejects_bad_calls() {
        let t = table();
        assert!(t.url("missing", &[]).is_err());
        assert!(t.url("produk.show", &[]).is_err());
        assert!(t.url("home", &[&1]).is_err());
    }

    #[test]
    fn rejects_duplicate_names() {
        let mut t = table();
        assert!(t.insert("home".into(), "/home".into()).is_err());
    }
}

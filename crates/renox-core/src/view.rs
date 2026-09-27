use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, REFERER};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use minijinja::value::{Rest, merge_maps};
use minijinja::{Environment, ErrorKind, Value, context};
use minijinja_autoreload::AutoReloader;
use serde::Serialize;

use crate::error::{ErrorPage, reason};
use crate::validation::ValidationError;
use crate::{AppState, Config, Error, Htmx, RouteTable, Session, assets};

/// Templates that ship with Renox. An app overrides one by creating a file
/// with the same name in its views directory.
const BUILTIN: &[(&str, &str)] = &[
    ("renox/error.html", include_str!("../views/error.html")),
    (
        "renox/pagination.html",
        include_str!("../views/pagination.html"),
    ),
];

/// The template engine (MiniJinja), reading from `VIEWS_PATH`.
///
/// Templates are reloaded when they change while `APP_DEBUG` is on.
#[derive(Clone)]
pub struct Views {
    reloader: Arc<AutoReloader>,
}

impl Views {
    pub(crate) fn new(config: &Config, routes: Arc<RouteTable>) -> Self {
        let dir = config.views_path.clone();
        let watch = config.debug && dir.is_dir();
        let reloader = AutoReloader::new(move |notifier| {
            let mut env = Environment::new();
            let loader_dir = dir.clone();
            env.set_loader(move |name| load(&loader_dir, name));

            let routes = routes.clone();
            env.add_function(
                "route",
                move |name: String, params: Rest<Value>| -> Result<Value, minijinja::Error> {
                    let params: Vec<&dyn std::fmt::Display> =
                        params.iter().map(|p| p as &dyn std::fmt::Display).collect();
                    // Percent-encoded, so safe to use in HTML without escaping `/`.
                    routes
                        .url(&name, &params)
                        .map(Value::from_safe_string)
                        .map_err(|err| {
                            minijinja::Error::new(ErrorKind::InvalidOperation, err.to_string())
                        })
                },
            );
            env.add_function("asset", |path: String| {
                let mut url = String::from("/");
                crate::routing::encode(&mut url, path.trim_start_matches('/'), true);
                Value::from_safe_string(url)
            });

            if watch {
                notifier.watch_path(&dir, true);
            }
            Ok(env)
        });
        Self {
            reloader: Arc::new(reloader),
        }
    }

    /// Renders a template with the given context and no request globals.
    pub fn render(&self, name: &str, ctx: impl Serialize) -> anyhow::Result<String> {
        let env = self.reloader.acquire_env()?;
        Ok(env.get_template(name)?.render(ctx)?)
    }

    fn render_view(&self, view: &View, globals: Value, htmx: &Htmx) -> anyhow::Result<String> {
        let env = self.reloader.acquire_env()?;
        let template = env.get_template(&view.name)?;
        let ctx = merge_maps([view.ctx.clone(), globals]);
        match &view.fragment {
            Some(block) if htmx.wants_fragment() => {
                let mut captured = template.render_captured_to(ctx, std::io::sink())?;
                Ok(captured.with_state_mut(|state| state.render_block(block))?)
            }
            _ => Ok(template.render(ctx)?),
        }
    }

    fn render_error(&self, page: &ErrorPage) -> anyhow::Result<String> {
        let env = self.reloader.acquire_env()?;
        let specific = format!("errors/{}.html", page.status.as_u16());
        let template = match env.get_template(&specific) {
            Ok(template) => template,
            Err(err) if err.kind() == ErrorKind::TemplateNotFound => {
                env.get_template("renox/error.html")?
            }
            Err(err) => return Err(err.into()),
        };
        Ok(template.render(context! {
            status => page.status.as_u16(),
            reason => reason(page.status),
            detail => page.detail,
        })?)
    }
}

fn load(dir: &Path, name: &str) -> Result<Option<String>, minijinja::Error> {
    if let Some(path) = safe_join(dir, name) {
        match std::fs::read_to_string(&path) {
            Ok(source) => return Ok(Some(source)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(minijinja::Error::new(
                    ErrorKind::InvalidOperation,
                    format!("could not read template {}", path.display()),
                )
                .with_source(err));
            }
        }
    }
    Ok(BUILTIN
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .map(|(_, source)| (*source).to_owned()))
}

fn safe_join(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut path = dir.to_path_buf();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }
    Some(path)
}

/// A template response, rendered by Renox with the request's globals:
/// `app`, `request`, `flash`, `errors`, `error()`, `old()`, `csrf_token`,
/// `csrf_field()` and `renox_head()`.
///
/// ```ignore
/// async fn index() -> View {
///     view("produk/index.html", context! { produk => list }).fragment("list")
/// }
/// ```
#[derive(Clone)]
pub struct View {
    name: String,
    ctx: Value,
    fragment: Option<String>,
    status: StatusCode,
}

/// Renders `name` from the views directory with `ctx` (anything serializable,
/// usually `context! { ... }`).
pub fn view(name: impl Into<String>, ctx: impl Serialize) -> View {
    View {
        name: name.into(),
        ctx: Value::from_serialize(ctx),
        fragment: None,
        status: StatusCode::OK,
    }
}

impl View {
    /// For HTMX requests (except `hx-boost`), render only this `{% block %}`.
    pub fn fragment(mut self, block: impl Into<String>) -> Self {
        self.fragment = Some(block.into());
        self
    }

    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }
}

impl IntoResponse for View {
    fn into_response(self) -> Response {
        let mut res = self.status.into_response();
        res.extensions_mut().insert(self);
        res
    }
}

/// Renders `View` and error responses once the handler has returned, and
/// turns validation errors into a redirect back for regular form posts.
pub(crate) async fn middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let session = req.extensions().get::<Session>().cloned();
    let htmx = Htmx::from_headers(req.headers());
    let path = req.uri().path().to_owned();
    let (wants_json, referer) = {
        let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
        let wants_json = header(ACCEPT).is_some_and(|v| v.contains("application/json"))
            || header(CONTENT_TYPE).is_some_and(|v| v.starts_with("application/json"));
        (wants_json, header(REFERER).map(str::to_owned))
    };

    let mut res = next.run(req).await;

    if let Some(failed) = res.extensions_mut().remove::<ValidationError>() {
        if htmx.request || wants_json {
            return res;
        }
        if let Some(session) = &session {
            let flashed = session
                .flash_errors(&failed.errors)
                .and_then(|()| session.flash_input(&failed.input));
            if let Err(err) = flashed {
                return err.into_response();
            }
        }
        return Redirect::to(referer.as_deref().unwrap_or("/")).into_response();
    }

    if let Some(view) = res.extensions_mut().remove::<View>() {
        let globals = globals(&state, session.as_ref(), &htmx, &path);
        return match state.views.render_view(&view, globals, &htmx) {
            Ok(html) => with_html(res, html),
            Err(err) => {
                Error::Internal(err.context(format!("rendering {}", view.name))).into_response()
            }
        };
    }

    if let Some(page) = res.extensions_mut().remove::<ErrorPage>() {
        match state.views.render_error(&page) {
            Ok(html) => return with_html(res, html),
            Err(err) => tracing::error!(error = ?err, "could not render the error page"),
        }
    }
    res
}

fn with_html(mut res: Response, html: String) -> Response {
    let headers = res.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.remove(CONTENT_LENGTH);
    *res.body_mut() = Body::from(html);
    res
}

fn globals(state: &AppState, session: Option<&Session>, htmx: &Htmx, path: &str) -> Value {
    let config = &state.config;
    let token = session.map(Session::token).unwrap_or_default();

    let head = Value::from_safe_string(assets::head_tags(&token));
    let field = Value::from_safe_string(format!(
        "<input type=\"hidden\" name=\"{}\" value=\"{token}\">",
        crate::csrf::CSRF_FIELD
    ));
    let old_input = session.cloned();
    let errors = session.map(Session::errors).unwrap_or_default();
    let first_errors: std::collections::BTreeMap<String, String> = errors
        .iter()
        .filter_map(|(field, messages)| {
            messages
                .get(0)
                .and_then(|m| m.as_str())
                .map(|m| (field.clone(), m.to_owned()))
        })
        .collect();

    context! {
        app => context! {
            name => config.name,
            env => format!("{:?}", config.env).to_lowercase(),
            debug => config.debug,
            url => config.url,
        },
        request => context! {
            path => path,
            htmx => htmx.request,
            boosted => htmx.boosted,
        },
        csrf_token => token,
        flash => session.map(Session::flashed).unwrap_or_default(),
        errors => errors,
        error => Value::from_function(move |field: String| {
            first_errors.get(&field).cloned().unwrap_or_default()
        }),
        renox_head => Value::from_function(move || head.clone()),
        csrf_field => Value::from_function(move || field.clone()),
        old => Value::from_function(move |field: String, default: Option<Value>| {
            old_input
                .as_ref()
                .and_then(|s| s.old(&field))
                .map(Value::from_serialize)
                .or(default)
                .unwrap_or_else(|| Value::from(""))
        }),
    }
}

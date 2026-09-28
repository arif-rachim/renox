use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use minijinja::value::{Rest, merge_maps};
use minijinja::{Environment, ErrorKind, Value, context};
use minijinja_autoreload::AutoReloader;
use serde::Serialize;

use crate::auth::CurrentUser;
use crate::error::{ErrorPage, reason};
use crate::storage::Storage;
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
    (
        "renox/auth/layout.html",
        include_str!("../views/auth/layout.html"),
    ),
    (
        "renox/auth/login.html",
        include_str!("../views/auth/login.html"),
    ),
    (
        "renox/auth/register.html",
        include_str!("../views/auth/register.html"),
    ),
    (
        "renox/auth/forgot-password.html",
        include_str!("../views/auth/forgot-password.html"),
    ),
    (
        "renox/auth/reset-password.html",
        include_str!("../views/auth/reset-password.html"),
    ),
    (
        "renox/auth/verify-email.html",
        include_str!("../views/auth/verify-email.html"),
    ),
    (
        "renox/mail/layout.html",
        include_str!("../views/mail/layout.html"),
    ),
    (
        "renox/mail/button.html",
        include_str!("../views/mail/button.html"),
    ),
    (
        "renox/queue/dashboard.html",
        include_str!("../views/queue/dashboard.html"),
    ),
    (
        "renox/mail/components.html",
        include_str!("../views/mail/components.html"),
    ),
    (
        "renox/auth/account.html",
        include_str!("../views/auth/account.html"),
    ),
    (
        "renox/auth/confirm-password.html",
        include_str!("../views/auth/confirm-password.html"),
    ),
    (
        "renox/mail/auth/reset-password.html",
        include_str!("../views/mail/auth/reset-password.html"),
    ),
    (
        "renox/mail/auth/reset-password.txt",
        include_str!("../views/mail/auth/reset-password.txt"),
    ),
    (
        "renox/mail/auth/verify-email.html",
        include_str!("../views/mail/auth/verify-email.html"),
    ),
    (
        "renox/mail/auth/verify-email.txt",
        include_str!("../views/mail/auth/verify-email.txt"),
    ),
];

/// The template engine (MiniJinja), reading from `VIEWS_PATH`.
///
/// Templates are reloaded when they change while `APP_DEBUG` is on.
#[derive(Clone)]
pub struct Views {
    reloader: Arc<AutoReloader>,
}

/// What an `App::share` function knows about the request being rendered.
#[non_exhaustive]
#[derive(Clone)]
pub struct ViewContext {
    pub state: AppState,
    /// The logged-in user, if any.
    pub user: Option<Arc<crate::auth::User>>,
    /// The request's language, e.g. `id`.
    pub locale: String,
    /// The request's path, e.g. `/products`.
    pub path: String,
}

pub(crate) type ShareFn = Arc<
    dyn Fn(
            ViewContext,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<Value>> + Send>>
        + Send
        + Sync,
>;

pub(crate) fn share_fn<F, Fut, T>(compute: F) -> ShareFn
where
    F: Fn(ViewContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = crate::Result<T>> + Send + 'static,
    T: Serialize,
{
    let compute = Arc::new(compute);
    Arc::new(move |ctx| {
        let compute = compute.clone();
        Box::pin(async move { Ok(Value::from_serialize(compute(ctx).await?)) })
    })
}

/// Adds functions, filters or globals to the template environment; see
/// `App::templates`.
pub(crate) type TemplateHook = Arc<dyn Fn(&mut Environment<'static>) + Send + Sync>;

impl Views {
    /// `embedded`: templates compiled into the binary, used instead of
    /// `VIEWS_PATH` when given.
    pub(crate) fn new(
        config: &Config,
        routes: Arc<RouteTable>,
        storage: Storage,
        embedded: Option<&'static [(&'static str, &'static str)]>,
        hooks: Arc<Vec<TemplateHook>>,
        zone: crate::timezone::Zone,
        versions: Arc<crate::embedded::AssetVersions>,
    ) -> Self {
        let dir = config.views_path.clone();
        let watch = config.debug && embedded.is_none() && dir.is_dir();
        let debug = config.debug;
        let reloader = AutoReloader::new(move |notifier| {
            let mut env = Environment::new();
            env.set_formatter(format_value);
            // While developing, printing a misspelled variable is an error
            // instead of an empty string (`{% if x %}` on a missing one is fine).
            if debug {
                env.set_undefined_behavior(minijinja::UndefinedBehavior::SemiStrict);
            }
            let loader_dir = dir.clone();
            env.set_loader(move |name| load(&loader_dir, embedded, name));

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
            // The current URL's query string with `page` set to `page`, for
            // pagination links that keep filters such as `?q=kopi`.
            env.add_function("page_url", |state: &minijinja::State, page: u32| -> Value {
                let query = state
                    .lookup("request")
                    .and_then(|request| request.get_attr("query").ok())
                    .and_then(|query| query.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let mut url = form_urlencoded::Serializer::new(String::new());
                for (key, value) in form_urlencoded::parse(query.as_bytes()) {
                    if key != "page" {
                        url.append_pair(&key, &value);
                    }
                }
                url.append_pair("page", &page.to_string());
                Value::from(format!("?{}", url.finish()))
            });
            env.add_function("method_field", |method: String| {
                let method: String = method.chars().filter(char::is_ascii_alphabetic).collect();
                Value::from_safe_string(format!(
                    "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
                    crate::method::METHOD_FIELD,
                    method.to_ascii_uppercase()
                ))
            });
            // `/app.css?v=1a2b3c4d`: a new URL whenever the file changes.
            let versions = versions.clone();
            env.add_function("asset", move |path: String| {
                let mut url = String::from("/");
                crate::routing::encode(&mut url, path.trim_start_matches('/'), true);
                if let Some(version) = versions.version(&path) {
                    url.push_str("?v=");
                    url.push_str(&version);
                }
                Value::from_safe_string(url)
            });
            let storage = storage.clone();
            env.add_function("storage_url", move |key: String| {
                Value::from_safe_string(storage.url(&key))
            });
            env.add_filter("number", crate::view_filters::number);
            env.add_filter("date", crate::view_filters::date(zone));
            // The app's own functions and filters (`App::templates`).
            for hook in hooks.iter() {
                hook(&mut env);
            }

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

    fn render_view(
        &self,
        view: &View,
        shared: Value,
        globals: Value,
        htmx: &Htmx,
    ) -> anyhow::Result<String> {
        let env = self.reloader.acquire_env()?;
        let template = env.get_template(&view.name)?;
        // The last map wins: shared values, then the handler's, then Renox's.
        let ctx = merge_maps([shared, view.ctx.clone(), globals]);
        match &view.fragment {
            Some(block) if htmx.wants_fragment() => {
                let mut captured = template.render_captured_to(ctx, std::io::sink())?;
                Ok(captured.with_state_mut(|state| state.render_block(block))?)
            }
            _ => Ok(template.render(ctx)?),
        }
    }

    fn render_error(&self, page: &ErrorPage, debug: bool, request: &str) -> anyhow::Result<String> {
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
            detail => page.shown_detail(debug),
            // Only while developing: what was asked, and where a template failed.
            debug => debug,
            request => debug.then_some(request),
            template => page.template.as_deref().filter(|_| debug),
        })?)
    }
}

/// Like MiniJinja's default formatter, but HTML escaping leaves `/` alone:
/// escaping `& < > " '` is enough for text and quoted attributes, and URLs
/// (links in pages and mail) stay readable.
fn format_value(
    out: &mut minijinja::Output,
    state: &minijinja::State,
    value: &Value,
) -> Result<(), minijinja::Error> {
    use std::fmt::Write;
    if let (minijinja::AutoEscape::Html, false, Some(text)) =
        (state.auto_escape(), value.is_safe(), value.as_str())
    {
        for c in text.chars() {
            let written = match c {
                '&' => out.write_str("&amp;"),
                '<' => out.write_str("&lt;"),
                '>' => out.write_str("&gt;"),
                '"' => out.write_str("&quot;"),
                '\'' => out.write_str("&#39;"),
                c => out.write_char(c),
            };
            written.map_err(|_| {
                minijinja::Error::new(ErrorKind::WriteFailure, "could not write output")
            })?;
        }
        return Ok(());
    }
    minijinja::escape_formatter(out, state, value)
}

fn load(
    dir: &Path,
    embedded: Option<&'static [(&'static str, &'static str)]>,
    name: &str,
) -> Result<Option<String>, minijinja::Error> {
    if let Some(files) = embedded {
        if let Some((_, source)) = files.iter().find(|(file, _)| *file == name) {
            return Ok(Some((*source).to_owned()));
        }
    } else if let Some(path) = safe_join(dir, name) {
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
/// `app` (with the request's `app.locale`), `request`, `auth` (`auth.check`,
/// `auth.user`), `t()`, `can()`, `flash`, `errors`, `error()`, `old()`,
/// `csrf_token`, `csrf_field()` and `renox_head()`.
///
/// ```
/// # use renox::prelude::*;
/// # let list: Vec<String> = Vec::new();
/// # let _ = move || {
/// async fn index() -> View {
/// #   let list: Vec<String> = Vec::new();
///     view("produk/index.html", context! { produk => list }).fragment("list")
/// }
/// # };
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
    let current_user = req.extensions().get::<CurrentUser>().cloned();
    let locale = crate::i18n::request_locale(req.extensions(), &state);
    let htmx = Htmx::from_headers(req.headers());
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or_default().to_owned();
    let nonce = req
        .extensions()
        .get::<crate::security::CspNonce>()
        .map(|n| n.0.clone())
        .unwrap_or_default();
    let (wants_json, referer) = {
        let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
        let wants_json = header(ACCEPT).is_some_and(|v| v.contains("application/json"))
            || header(CONTENT_TYPE).is_some_and(|v| v.starts_with("application/json"));
        // Back to the form's page, but never to another site.
        (wants_json, crate::htmx::same_site_referer(req.headers()))
    };

    let request_line = if query.is_empty() {
        format!("{method} {path}")
    } else {
        format!("{method} {path}?{query}")
    };
    let mut res = next.run(req).await;

    if let Some(failed) = res.extensions_mut().remove::<ValidationError>() {
        if htmx.request || wants_json {
            return res;
        }
        if let Some(session) = &session {
            // A `ValidationError` made after `Valid` (a model hook, the
            // handler) carries no input: refill with what `Valid` read.
            let input = if failed.input.is_empty() {
                crate::context::get::<crate::validation::extract::SubmittedInput>()
                    .map(|submitted| submitted.0)
                    .unwrap_or_default()
            } else {
                failed.input
            };
            let flashed = session
                .flash_errors(&failed.errors)
                .and_then(|()| session.flash_input(&input));
            if let Err(err) = flashed {
                return err.into_response();
            }
        }
        return Redirect::to(referer.as_deref().unwrap_or("/")).into_response();
    }

    // Analytics events: with an htmx swap, in its HX-Trigger; with a page,
    // in its head (below); otherwise they wait for the next page.
    if let Some(session) = &session
        && crate::analytics::has_pending(session)
        && htmx.request
        && crate::analytics::deliverable_by_htmx(&res)
    {
        crate::analytics::add_trigger(&mut res, crate::analytics::take(session));
    }

    if let Some(view) = res.extensions_mut().remove::<View>() {
        let mut shared = std::collections::BTreeMap::new();
        for (key, compute) in state.shares.iter() {
            let ctx = ViewContext {
                state: state.clone(),
                user: current_user.as_ref().and_then(|c| c.user.clone()),
                locale: locale.clone(),
                path: path.clone(),
            };
            match compute(ctx).await {
                Ok(value) => {
                    shared.insert(key.clone(), value);
                }
                Err(err) => {
                    let err = match err {
                        Error::Internal(err) => err,
                        other => anyhow::anyhow!("{other:?}"),
                    };
                    return Error::Internal(err.context(format!("sharing `{key}` with views")))
                        .into_response();
                }
            }
        }
        let events = match &session {
            Some(session) if !htmx.request => crate::analytics::take(session),
            _ => Vec::new(),
        };
        let globals = globals(
            &state,
            session.as_ref(),
            current_user,
            &htmx,
            &Requested {
                path: &path,
                query: &query,
                nonce: &nonce,
                events: &events,
            },
            &locale,
        );
        return match state
            .views
            .render_view(&view, Value::from_serialize(&shared), globals, &htmx)
        {
            Ok(html) => with_html(res, html),
            Err(err) => {
                let mut failed = Error::Internal(err.context(format!("rendering {}", view.name)))
                    .into_response();
                match failed.extensions_mut().remove::<ErrorPage>() {
                    Some(page) => {
                        error_response(&state, page, failed, wants_json, &htmx, &request_line)
                    }
                    None => failed,
                }
            }
        };
    }

    if let Some(page) = res.extensions_mut().remove::<ErrorPage>() {
        return error_response(&state, page, res, wants_json, &htmx, &request_line);
    }
    res
}

/// The error page (or JSON for API clients) for `page`.
fn error_response(
    state: &AppState,
    page: ErrorPage,
    res: Response,
    wants_json: bool,
    htmx: &Htmx,
    request_line: &str,
) -> Response {
    let debug = state.config.debug;
    if wants_json && !htmx.request {
        return page.json(debug);
    }
    let html = state
        .views
        .render_error(&page, debug, request_line)
        .unwrap_or_else(|err| {
            tracing::error!(error = ?err, "could not render the error page");
            crate::error::error_page(page.status, page.shown_detail(debug))
        });
    with_html(res, html)
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

/// Flashed values; a key that wasn't flashed reads as `""`, so
/// `{{ flash.status }}` needs no `if`, even with strict templates.
#[derive(Debug)]
struct Flashed(serde_json::Map<String, serde_json::Value>);

impl minijinja::value::Object for Flashed {
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = key.as_str()?;
        Some(
            self.0
                .get(key)
                .map(Value::from_serialize)
                .unwrap_or_else(|| Value::from("")),
        )
    }

    fn enumerate(self: &Arc<Self>) -> minijinja::value::Enumerator {
        minijinja::value::Enumerator::Values(
            self.0.keys().map(|k| Value::from(k.as_str())).collect(),
        )
    }

    fn repr(self: &Arc<Self>) -> minijinja::value::ObjectRepr {
        minijinja::value::ObjectRepr::Map
    }
}

/// What templates see of the request, besides the session and user.
struct Requested<'a> {
    path: &'a str,
    query: &'a str,
    nonce: &'a str,
    /// Analytics events for this page's head.
    events: &'a [crate::analytics::Event],
}

/// The templates' `t(key, name=…, count=…)` in `locale`.
pub(crate) fn translate_function(state: &AppState, locale: &str) -> Value {
    let translator = state.translator.clone();
    let (locale, fallback) = (locale.to_owned(), state.config.fallback_locale.clone());
    Value::from_function(
        move |key: String, kwargs: minijinja::value::Kwargs| -> Result<Value, minijinja::Error> {
            let mut params = Vec::new();
            let mut count = None;
            for name in kwargs.args() {
                let value: Value = kwargs.get(name)?;
                if name == "count" {
                    count = i64::try_from(value).ok();
                } else {
                    params.push((name.to_owned(), value.to_string()));
                }
            }
            kwargs.assert_all_used()?;
            let params: Vec<(&str, String)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            let text = translator.get(&locale, &fallback, &key);
            Ok(Value::from(crate::i18n::format(&text, &params, count)))
        },
    )
}

fn globals(
    state: &AppState,
    session: Option<&Session>,
    current_user: Option<CurrentUser>,
    htmx: &Htmx,
    requested: &Requested,
    locale: &str,
) -> Value {
    let config = &state.config;
    let token = session.map(Session::token).unwrap_or_default();

    let strict = state.security.mode == crate::CspMode::Strict;
    let head = Value::from_safe_string(format!(
        "{}\n{}",
        assets::head_tags(&token, state.live.is_some(), strict),
        crate::seo::head_tags(&state.config, requested.nonce, requested.events)
    ));
    let seo = {
        let (config, path, locale) = (
            state.config.clone(),
            requested.path.to_owned(),
            locale.to_owned(),
        );
        Value::from_function(
            move |kwargs: minijinja::value::Kwargs| -> Result<Value, minijinja::Error> {
                crate::seo::tags(&config, &path, &locale, &kwargs).map(Value::from_safe_string)
            },
        )
    };
    let nonce = requested.nonce.to_owned();
    let field = Value::from_safe_string(format!(
        "<input type=\"hidden\" name=\"{}\" value=\"{token}\">",
        crate::csrf::CSRF_FIELD
    ));
    let old_input = session.cloned();
    let user = current_user.as_ref().and_then(|c| c.user.clone());
    let roles = current_user
        .as_ref()
        .map(|c| c.grants.roles.clone())
        .unwrap_or_default();
    let gate_user = current_user.and_then(|c| Some((c.user?, c.gates, c.grants)));
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
            locale => locale,
        },
        auth => context! {
            check => user.is_some(),
            user => user.as_deref(),
            roles => roles,
        },
        t => translate_function(state, locale),
        // `can('admin')` asks a gate; `can('update', product)` reads the
        // abilities `auth::Can` attached to the model in the handler.
        can => Value::from_function(move |ability: String, target: Option<Value>| {
            match target {
                Some(target) => target
                    .get_attr("_can")
                    .and_then(|can| can.get_attr(&ability))
                    .is_ok_and(|allowed| allowed.is_true()),
                None => gate_user
                    .as_ref()
                    .is_some_and(|(user, gates, grants)| gates.check(user, grants, &ability)),
            }
        }),
        request => context! {
            path => requested.path,
            query => requested.query,
            htmx => htmx.request,
            boosted => htmx.boosted,
        },
        csrf_token => token,
        flash => Value::from_object(Flashed(session.map(Session::flashed).unwrap_or_default())),
        errors => errors,
        // `error('photos')` also shows the first error of an item (`photos.1`).
        error => Value::from_function(move |field: String| {
            first_errors
                .get(&field)
                .or_else(|| {
                    let prefix = format!("{field}.");
                    first_errors
                        .iter()
                        .find(|(key, _)| key.starts_with(&prefix))
                        .map(|(_, message)| message)
                })
                .cloned()
                .unwrap_or_default()
        }),
        renox_head => Value::from_function(move || head.clone()),
        seo => seo,
        csp_nonce => Value::from_function(move || nonce.clone()),
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

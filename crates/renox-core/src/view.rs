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
    ("renox/ui.html", include_str!("../views/ui.html")),
    ("renox/grid.html", include_str!("../views/grid.html")),
    (
        "renox/grid_print.html",
        include_str!("../views/grid_print.html"),
    ),
    ("renox/debug.html", include_str!("../views/debug.html")),
    (
        "renox/notifications.html",
        include_str!("../views/notifications.html"),
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
    /// The application state.
    pub state: AppState,
    /// The logged-in user, if any.
    pub user: Option<Arc<crate::auth::User>>,
    /// The request's language, e.g. `en`.
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
        let currency = config.currency.clone();
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
                    // `route('products.index', page=2)`: named arguments are
                    // the query string (`?page=2`).
                    let (query, params): (Vec<&Value>, Vec<&Value>) =
                        params.iter().partition(|p| p.is_kwargs());
                    let params: Vec<&dyn std::fmt::Display> = params
                        .iter()
                        .map(|p| *p as &dyn std::fmt::Display)
                        .collect();
                    // Percent-encoded, so safe to use in HTML without escaping `/`.
                    let mut url = routes.url(&name, &params).map_err(|err| {
                        minijinja::Error::new(ErrorKind::InvalidOperation, err.to_string())
                    })?;
                    if let Some(kwargs) = query.first() {
                        let mut pairs = form_urlencoded::Serializer::new(String::new());
                        for key in kwargs.try_iter()? {
                            let value = kwargs.get_item(&key)?;
                            if value.is_none() || value.is_undefined() {
                                continue;
                            }
                            pairs.append_pair(&key.to_string(), &value.to_string());
                        }
                        let pairs = pairs.finish();
                        if !pairs.is_empty() {
                            url.push(if url.contains('?') { '&' } else { '?' });
                            url.push_str(&pairs.replace('&', "&amp;"));
                        }
                    }
                    Ok(Value::from_safe_string(url))
                },
            );
            // The current URL's query string with `page` set to `page`, for
            // pagination links that keep filters such as `?q=coffee`.
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
            // The current URL's query string with some keys set (or removed
            // with `none`), and `page` dropped: `query_with(period="7d")`
            // for filters that keep the others.
            env.add_function(
                "query_with",
                |state: &minijinja::State,
                 kwargs: minijinja::value::Kwargs|
                 -> Result<Value, minijinja::Error> {
                    let query = state
                        .lookup("request")
                        .and_then(|request| request.get_attr("query").ok())
                        .and_then(|query| query.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    let keys: Vec<String> = kwargs.args().map(str::to_owned).collect();
                    let mut url = form_urlencoded::Serializer::new(String::new());
                    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
                        if key != "page" && !keys.iter().any(|k| *k == key) {
                            url.append_pair(&key, &value);
                        }
                    }
                    for key in &keys {
                        let value: Value = kwargs.get(key)?;
                        if !value.is_none() && !value.is_undefined() {
                            url.append_pair(key, &value.to_string());
                        }
                    }
                    kwargs.assert_all_used()?;
                    Ok(Value::from(format!("?{}", url.finish())))
                },
            );
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
            // The request's values for components: `old`, `error`, `t`, `can`,
            // `csrf_field`, `auth`, `request`, `flash`… work inside imported
            // macros too, not only in the rendered template.
            for name in REQUEST_GLOBALS {
                env.add_global(*name, Value::from_object(RequestGlobal(name)));
            }
            crate::view_stack::register(&mut env);
            env.add_function("renox_ui", |kwargs: minijinja::value::Kwargs| {
                let styles: Option<bool> = kwargs.get("styles")?;
                kwargs.assert_all_used()?;
                Ok::<_, minijinja::Error>(Value::from_safe_string(crate::assets::ui_tags(
                    styles.unwrap_or(true),
                )))
            });
            env.add_function("renox_calendar", || {
                Value::from_safe_string(crate::assets::calendar_tags())
            });
            env.add_function("renox_grid", || {
                Value::from_safe_string(crate::assets::grid_tags())
            });
            env.add_function("sparkline", crate::view_filters::sparkline);
            env.add_filter("number", crate::view_filters::number);
            env.add_filter("date", crate::view_filters::date(zone));
            env.add_filter("since", crate::view_filters::since(zone));
            env.add_filter("money", crate::view_filters::money(currency.clone()));
            env.add_filter("words", crate::view_filters::words);
            env.add_filter("markdown", crate::view_filters::markdown);
            env.add_function("chart", crate::chart::chart(currency.clone()));
            env.add_function("class_names", crate::view_filters::class_names);
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

    /// Whether a template of this name exists (the app's or a built-in).
    pub fn exists(&self, name: &str) -> bool {
        self.reloader
            .acquire_env()
            .is_ok_and(|env| env.get_template(name).is_ok())
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
        // Components (macros imported from other templates) don't see this
        // context; they reach the same values through `RequestGlobal`s.
        let _current = CurrentGlobals::set(globals.clone());
        // The last map wins: shared values, then the handler's, then Renox's.
        let ctx = merge_maps([shared, view.ctx.clone(), globals]);
        let stacks = crate::view_stack::Scope::begin();
        let html = match &view.fragment {
            Some(block) if htmx.wants_fragment() => {
                let mut captured = template.render_captured_to(ctx, std::io::sink())?;
                let mut out = captured.with_state_mut(|state| state.render_block(block))?;
                for extra in &view.also {
                    out.push_str(&captured.with_state_mut(|state| state.render_block(extra))?);
                }
                out
            }
            _ => template.render(ctx)?,
        };
        Ok(stacks.finish(html))
    }

    /// The error page: the app's `errors/{status}.html`, else its
    /// `errors/default.html`, else Renox's. With `globals` (a request's), an
    /// app's page can extend its layout; a page that fails falls back to
    /// Renox's, so an error in the layout doesn't hide the first error.
    fn render_error(
        &self,
        page: &ErrorPage,
        debug: bool,
        request: &str,
        globals: Option<Value>,
    ) -> anyhow::Result<String> {
        let env = self.reloader.acquire_env()?;
        let ctx = context! {
            status => page.status.as_u16(),
            reason => reason(page.status),
            detail => page.shown_detail(debug),
            // Only while developing: what was asked, and where a template failed.
            debug => debug,
            request_line => debug.then_some(request),
            template => page.template.as_deref().filter(|_| debug),
        };
        for name in [
            format!("errors/{}.html", page.status.as_u16()),
            "errors/default.html".to_owned(),
        ] {
            let template = match env.get_template(&name) {
                Ok(template) => template,
                Err(err) if err.kind() == ErrorKind::TemplateNotFound => continue,
                Err(err) => return Err(err.into()),
            };
            let rendered = match &globals {
                Some(globals) => {
                    let _current = CurrentGlobals::set(globals.clone());
                    let stacks = crate::view_stack::Scope::begin();
                    template
                        .render(merge_maps([globals.clone(), ctx.clone()]))
                        .map(|html| stacks.finish(html))
                }
                None => template.render(ctx.clone()),
            };
            match rendered {
                Ok(html) => return Ok(html),
                Err(err) => {
                    tracing::error!(error = ?err, template = %name, "the error page failed; showing Renox's");
                    break;
                }
            }
        }
        Ok(env.get_template("renox/error.html")?.render(ctx)?)
    }
}

/// The template a response was rendered from.
#[derive(Debug, Clone)]
pub(crate) struct RenderedView(pub String);

/// The globals Renox gives each rendered page (see `globals`), which
/// components reach through the environment.
const REQUEST_GLOBALS: &[&str] = &[
    "app",
    "auth",
    "t",
    "can",
    "request",
    "route_is",
    "csrf_token",
    "csrf_field",
    "flash",
    "errors",
    "error",
    "old",
    "has_old",
    "renox_head",
    "seo",
    "csp_nonce",
    "toasts",
    "once",
];

thread_local! {
    /// The globals of the page being rendered on this thread.
    static CURRENT: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
}

/// Makes `globals` the current page's while alive (rendering is synchronous).
struct CurrentGlobals(Option<Value>);

impl CurrentGlobals {
    fn set(globals: Value) -> Self {
        Self(CURRENT.with(|c| c.borrow_mut().replace(globals)))
    }
}

impl Drop for CurrentGlobals {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// A request global as seen from the environment: forwards to the value of
/// the page being rendered, so an imported macro gets the same `old()`,
/// `auth`, `t()`… as the template that called it.
#[derive(Debug)]
struct RequestGlobal(&'static str);

impl RequestGlobal {
    fn current(&self) -> Option<Value> {
        CURRENT
            .with(|c| c.borrow().clone())
            .and_then(|globals| globals.get_attr(self.0).ok())
            .filter(|v| !v.is_undefined())
    }
}

impl minijinja::value::Object for RequestGlobal {
    fn repr(self: &Arc<Self>) -> minijinja::value::ObjectRepr {
        minijinja::value::ObjectRepr::Map
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.current()?
            .get_item(key)
            .ok()
            .filter(|v| !v.is_undefined())
    }

    fn enumerate(self: &Arc<Self>) -> minijinja::value::Enumerator {
        match self
            .current()
            .and_then(|v| v.try_iter().ok().map(|keys| keys.collect::<Vec<_>>()))
        {
            Some(keys)
                if self
                    .current()
                    .is_some_and(|v| v.kind() == minijinja::value::ValueKind::Map) =>
            {
                minijinja::value::Enumerator::Values(keys)
            }
            _ => minijinja::value::Enumerator::Empty,
        }
    }

    fn is_true(self: &Arc<Self>) -> bool {
        self.current().is_some_and(|v| v.is_true())
    }

    fn call(
        self: &Arc<Self>,
        state: &minijinja::State<'_, '_>,
        args: &[Value],
    ) -> Result<Value, minijinja::Error> {
        match self.current() {
            Some(value) => value.call(state, args),
            None => Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                format!("`{}` is only available while rendering a page", self.0),
            )),
        }
    }

    fn render(self: &Arc<Self>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.current() {
            Some(value) => std::fmt::Display::fmt(&value, f),
            None => Ok(()),
        }
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
///     view("products/index.html", context! { products => list }).fragment("list")
/// }
/// # };
/// ```
#[derive(Clone)]
pub struct View {
    name: String,
    ctx: Value,
    fragment: Option<String>,
    /// More blocks sent with the fragment, for out-of-band swaps.
    also: Vec<String>,
    status: StatusCode,
}

/// Renders `name` from the views directory with `ctx` (anything serializable,
/// usually `context! { ... }`).
pub fn view(name: impl Into<String>, ctx: impl Serialize) -> View {
    View {
        name: name.into(),
        ctx: Value::from_serialize(ctx),
        fragment: None,
        also: Vec::new(),
        status: StatusCode::OK,
    }
}

impl View {
    /// For HTMX requests (except `hx-boost`), render only this `{% block %}`.
    pub fn fragment(mut self, block: impl Into<String>) -> Self {
        self.fragment = Some(block.into());
        self
    }

    /// For the same HTMX requests, also render `block` after the fragment,
    /// to update other parts of the page in one response: give the block's
    /// root element an `id` and `hx-swap-oob="true"` and htmx swaps it into
    /// the element with that id.
    ///
    /// ```html
    /// {% block row %}<tr id="order-{{ order.id }}">…</tr>{% endblock %}
    /// {% block count %}<span id="order-count" hx-swap-oob="true">{{ count }}</span>{% endblock %}
    /// ```
    ///
    /// `view("orders/index.html", ctx).fragment("row").also("count")`
    pub fn also(mut self, block: impl Into<String>) -> Self {
        self.also.push(block.into());
        self
    }

    /// The response status (200 by default).
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
    let route = crate::routing::CurrentRoute::of(req.extensions(), &state)
        .name()
        .map(str::to_owned);
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

    if let Some(crate::toast::PendingToasts(toasts)) =
        res.extensions_mut().remove::<crate::toast::PendingToasts>()
    {
        // An htmx swap shows them now; a page, an htmx redirect or refresh
        // on the next page, from the session (the reload would lose them).
        if htmx.request
            && !res.headers().contains_key("hx-redirect")
            && !res.headers().contains_key("hx-refresh")
        {
            crate::htmx::add_trigger(
                &mut res,
                crate::toast::EVENT,
                serde_json::json!({ "toasts": toasts }),
            );
        } else if let Some(session) = &session {
            let mut waiting: Vec<crate::Toast> =
                session.get(crate::toast::SESSION_KEY).unwrap_or_default();
            waiting.extend(toasts);
            if let Err(err) = session.put(crate::toast::SESSION_KEY, &waiting) {
                return err.into_response();
            }
        }
    }

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
            current_user.clone(),
            &htmx,
            &Requested {
                path: &path,
                query: &query,
                route: route.as_deref(),
                nonce: &nonce,
                events: &events,
            },
            &locale,
        );
        return match state
            .views
            .render_view(&view, Value::from_serialize(&shared), globals, &htmx)
        {
            Ok(html) => {
                let mut page = with_html(res, html);
                // For `TestResponse::assert_view`.
                page.extensions_mut()
                    .insert(RenderedView(view.name.clone()));
                page
            }
            Err(err) => {
                let mut failed = Error::Internal(err.context(format!("rendering {}", view.name)))
                    .into_response();
                match failed.extensions_mut().remove::<ErrorPage>() {
                    Some(page) => {
                        error_response(&state, page, failed, wants_json, &htmx, &request_line, None)
                    }
                    None => failed,
                }
            }
        };
    }

    if let Some(page) = res.extensions_mut().remove::<ErrorPage>() {
        // The app's error pages may extend its layout: give them what pages
        // get, the values `App::share`s included (a layout's cart count).
        let globals = if !wants_json || htmx.request {
            let mut shared = std::collections::BTreeMap::new();
            for (key, compute) in state.shares.iter() {
                let ctx = ViewContext {
                    state: state.clone(),
                    user: current_user.as_ref().and_then(|c| c.user.clone()),
                    locale: locale.clone(),
                    path: path.clone(),
                };
                // A share that fails here only leaves its value out: the page
                // is about another error, which it mustn't hide.
                match compute(ctx).await {
                    Ok(value) => {
                        shared.insert(key.clone(), value);
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, key = %key, "a shared view value failed on an error page")
                    }
                }
            }
            let page_globals = globals(
                &state,
                session.as_ref(),
                current_user,
                &htmx,
                &Requested {
                    path: &path,
                    query: &query,
                    route: route.as_deref(),
                    nonce: &nonce,
                    events: &[],
                },
                &locale,
            );
            Some(merge_maps([page_globals, Value::from_serialize(&shared)]))
        } else {
            None
        };
        return error_response(&state, page, res, wants_json, &htmx, &request_line, globals);
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
    globals: Option<Value>,
) -> Response {
    let debug = state.config.debug;
    if wants_json && !htmx.request {
        // Keep what the error response carried (`Retry-After` on a 429, …).
        let mut json = page.json(debug);
        for (name, value) in res.headers() {
            if name != CONTENT_TYPE && name != CONTENT_LENGTH {
                json.headers_mut().insert(name.clone(), value.clone());
            }
        }
        return json;
    }
    let html = state
        .views
        .render_error(&page, debug, request_line, globals)
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
    /// The matched route's name, if it has one.
    route: Option<&'a str>,
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
    let has_old = session.is_some_and(Session::has_old_input);
    let toast_session = session.cloned();
    let dismiss_label = state
        .translator
        .get(locale, &state.config.fallback_locale, "ui.dismiss");
    let seen = std::sync::Mutex::new(std::collections::HashSet::<String>::new());
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
            route => requested.route,
            htmx => htmx.request,
            boosted => htmx.boosted,
        },
        // `route_is('admin.*')`, `route_is('products.index', 'products.show')`.
        route_is => {
            let route = requested.route.map(str::to_owned);
            Value::from_function(move |patterns: minijinja::value::Rest<String>| {
                route.as_deref().is_some_and(|name| {
                    patterns
                        .iter()
                        .any(|pattern| crate::routing::route_name_matches(name, pattern))
                })
            })
        },
        csrf_token => token,
        flash => Value::from_object(Flashed(session.map(Session::flashed).unwrap_or_default())),
        errors => errors,
        // `error('photos')` also shows the first error of an item (`photos.1`).
        error => Value::from_function(move |field: String| {
            // `items[0][name]`'s errors are keyed `items.0.name`.
            let field = crate::validation::nested::normalize(&field);
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
        // `{{ toasts() }}`: the toast region, with the toasts waiting for
        // this page (taken from the session: shown once).
        // `toasts(position="bottom-end")` moves them (see toast::POSITIONS).
        toasts => Value::from_function(move |kwargs: minijinja::value::Kwargs| {
            let position: Option<String> = kwargs.get("position")?;
            kwargs.assert_all_used()?;
            let waiting: Vec<crate::Toast> = toast_session
                .as_ref()
                .and_then(|s| s.pull(crate::toast::SESSION_KEY))
                .unwrap_or_default();
            Ok::<_, minijinja::Error>(Value::from_safe_string(crate::toast::region(
                &waiting,
                &dismiss_label,
                position.as_deref().unwrap_or("top"),
            )))
        }),
        // `{% if once('date-picker') %}…{% endif %}`: true the first time a key
        // is asked for on a page, e.g. for a component's script.
        once => Value::from_function(move |key: String| {
            seen.lock().unwrap_or_else(|e| e.into_inner()).insert(key)
        }),
        // `has_old()`: the previous request was a failed submit, so a field
        // missing from `old()` was sent empty (an unticked checkbox).
        has_old => Value::from_function(move || has_old),
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

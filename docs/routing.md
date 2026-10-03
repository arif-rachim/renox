# Routing, middleware and requests

How a request reaches your code and what it can carry on the way: routes and their names,
the extractors a handler takes, the responses it returns, the middleware around it, and the
session, CSRF, cookie and signed-URL plumbing underneath. For the short version of every API,
see the [cheat-sheet](../CHEATSHEET.md) ("App, module, routes", "Your own shared values and
middleware", "Cookies and downloads", "Signed URLs and private files" and "Security").
Forms and their rules have their own guide ([validation.md](validation.md)), and so do htmx
responses and toasts ([ui.md](ui.md)) and who may do what ([authorization.md](authorization.md)).

| You want to… | Use |
|---|---|
| Add a page | `Routes::new().get("/path", handler).name("x")` in a `Module` |
| Prefix paths and names | `.group("/admin", "admin.", routes)` |
| The seven CRUD routes | `.resource("/products", "products", Resource::new()…)` |
| Another host | `.domain("admin.example.com", routes)`, `DomainParams` |
| Link to a route | `route('x', id)` in templates, `state.url("x", &[&id])` in Rust |
| Send somewhere | `Redirect::route("x", &[&id])`, `Redirect::intended`, `Back` |
| Run code around requests | `App::layer` (every route), `Routes::route_layer` (some) |
| Guard routes | `.require_auth()`, `.guest_only()`, `.require_gate(…)`, … |
| Limit request rates | `.throttle(60, …)`, `.throttle_by("api")` + `App::rate_limiter` |
| Remember something per visitor | `Session` (`put`, `flash`, `push`, `increment`) |
| A link that can't be altered | `state.signed_url(…)` + `ValidSignature` |

## Apps, modules and routes

An app is a list of modules; a module is a name, its routes, its migrations and the jobs,
listeners and commands it registers. `route:list` shows which module each route came from.

```rust
use renox::prelude::*;

pub fn app() -> App {
    App::new()
        .module(Auth::new()) // Renox's login, registration and password pages
        .module(Products)
}

pub struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/products", index).name("products.index")
            .get("/products/{id}", show).name("products.show") // {id}; a wildcard is {*rest}
            .post("/products", store).name("products.store")
            .put("/products/{id}", update)
            .patch("/products/{id}", update)
            .delete("/products/{id}", destroy)
    }
}

async fn index() -> View {
    view("products/index.html", context! {})
}

async fn show(Path(id): Path<i64>) -> String {
    format!("product {id}")
}

async fn store() -> Redirect {
    Redirect::to("/products")
}

async fn update(Path(id): Path<i64>) -> String {
    format!("updated {id}")
}

async fn destroy(Path(id): Path<i64>) -> StatusCode {
    let _ = id;
    StatusCode::NO_CONTENT
}
```

- `src/main.rs` is only `fn main() -> renox::Result { my_app::app().run() }`; the app lives in
  the library so tests can boot it. The binary is also the app's CLI (`migrate`, `route:list`,
  `down`, …).
- `.name("…")` names the route added just before it. Use dotted names (`products.show`), as
  Laravel does.
- `.route(path, method_router)` takes any axum method router, e.g.
  `renox::axum::routing::get(show).post(update)`; `route:list` shows its method as `*`.
- A group's index is `.get("/", …)`, never `""` (which panics).

### Groups, resources and merging

```rust
use renox::prelude::*;

# async fn index() -> &'static str { "" }
# async fn create() -> &'static str { "" }
# async fn store() -> &'static str { "" }
# async fn show(Path(id): Path<i64>) -> String { id.to_string() }
# async fn edit(Path(id): Path<i64>) -> String { id.to_string() }
# async fn update(Path(id): Path<i64>) -> String { id.to_string() }
# async fn destroy(Path(id): Path<i64>) -> String { id.to_string() }
# async fn dashboard() -> &'static str { "" }
fn routes() -> Routes {
    let shop = Routes::new().resource(
        "/products",
        "products",
        Resource::new()
            .index(index)     // GET    /products            products.index
            .create(create)   // GET    /products/new        products.create
            .store(store)     // POST   /products            products.store
            .show(show)       // GET    /products/{id}       products.show
            .edit(edit)       // GET    /products/{id}/edit  products.edit
            .update(update)   // PUT and PATCH /products/{id}  products.update
            .destroy(destroy), // DELETE /products/{id}      products.destroy
    );
    let admin = Routes::new().group(
        "/admin", // path prefix: starts with `/`, never ends with one
        "admin.", // name prefix
        Routes::new()
            .get("/", dashboard).name("dashboard") // GET /admin, `admin.dashboard`
            .require_auth(),                       // covers the group's routes only
    );
    shop.merge(admin) // the other routes, with their names and layers
}
```

A resource has only the actions you give it. `rnx make:module products --resource` writes one
with its handlers, views and tests.

Pages that need no handler (Laravel's `Route::view` and `Route::redirect`):

```rust
use renox::prelude::*;

fn routes() -> Routes {
    Routes::new()
        .view("/about", "pages/about.html").name("about") // GET, rendered with the globals
        .redirect("/about-us", "/about")                  // 302, any method
        .permanent_redirect("/old-shop", "/products")     // 301
}
```

### Other hosts and the fallback

`Routes::domain` serves routes only on hosts matching a pattern; `{name}` matches one label of
the host and is read with `DomainParams`. A host that matches a domain gets that domain's
routes only (plus Renox's own: `/health`, the scripts, public files), never the others; every
other host gets the routes without a domain. So `/` can be a different page on each host.

```rust
use renox::prelude::*;
use renox::DomainParams;
use renox::axum::http::Uri;

async fn home() -> &'static str {
    "the main site"
}

async fn team_home(domain: DomainParams) -> String {
    format!("Welcome to {}", domain.get("account").unwrap_or("?"))
}

async fn missing(uri: Uri) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("Nothing at {}. Try the search.", uri.path()))
}

fn routes() -> Routes {
    Routes::new()
        .get("/", home).name("home")
        .domain("{account}.example.com", Routes::new().get("/", team_home).name("team.home"))
        .fallback(missing) // when no route and no public file answers (instead of the 404 page)
}
```

- Matching uses the `Host` header. Behind a proxy, pass it through (Caddy does; nginx needs
  `proxy_set_header Host $host`).
- One fallback per app, or one per domain inside `Routes::domain`.
- [examples/teams](../examples/teams) serves public team pages on their own host.

### Route URLs and the current route

| Where | Path of a named route |
|---|---|
| Templates | `{{ route('products.show', product.id) }}`; named arguments become the query string: `route('products.index', page=2)` → `/products?page=2` |
| Handlers, jobs, commands | `state.url("products.show", &[&id])?`, `state.absolute_url(…)` (with `APP_URL`) |
| Redirects | `Redirect::route("products.show", &[&id])?` |

Parameters fill the `{…}` placeholders in order and are percent-encoded; a missing or extra
one is an error, as is an unknown name. Which route is answering:

```rust
use renox::prelude::*;
use renox::CurrentRoute;

async fn menu(route: CurrentRoute) -> String {
    // The matched route's name and path pattern; `*` in `is` stands for anything.
    format!("{} {} {}", route.name().unwrap_or("-"), route.path().unwrap_or("-"), route.is("admin.*"))
}

async fn links(State(state): State<AppState>, Path(id): Path<i64>) -> Result<String> {
    state.url("products.show", &[&id]) // "/products/42"
}
```

In templates: `{% if route_is('admin.*') %}` (several patterns allowed) and `request.route`.
`my-app route:list` (or `rnx route:list`) prints every route with its method, path, name,
module and guards (`auth`, `throttle:60/60s`, …), plus the domain when there are domains.

## What a handler can take

Handlers are async functions whose arguments are extractors. Order doesn't matter, except that
something that reads the body (`Valid<T>`, `Form`, `Json`) must come last.

| Extractor | Gives | Notes |
|---|---|---|
| `Path<T>` | route parameters: `Path(id): Path<i64>`, `Path((a, b)): Path<(i64, i64)>` | `renox::Path` (in the prelude): a value that doesn't parse (`/products/abc`) is a **404**, not axum's 400; a parameter the route lacks is a 500 (the app's bug) |
| `Found<M>` | the model a route parameter names, loaded (route model binding) | 404 when there's no such row; below |
| `Query<T>` | the query string, deserialized | axum's; `Option` fields for optional ones |
| `Valid<T>` | a validated form, JSON body or (for GET) query string | errors: redirect back, or 422 for htmx/JSON; see [validation.md](validation.md) |
| `Form<T>`, `Json<T>` | the body, unvalidated | axum's |
| `AuthUser` | the logged-in user (derefs to `User`) | guests: redirect to `login`, or 401 JSON for API clients; `Option<AuthUser>` for either |
| `Session` | the visitor's session | below |
| `Htmx` | `request`, `boosted`, `target`, `trigger`, `current_url` | see [ui.md](ui.md) |
| `Lang` | the request's locale (`lang.locale`), `t`, `choice` | |
| `ClientIp` | `ClientIp(Option<IpAddr>)` | through trusted proxies, below |
| `renox::RequestId` | this request's id | below |
| `renox::CurrentRoute` | the matched route | above |
| `renox::DomainParams` | `{name}` parts of the host | above |
| `renox::Provided<T>` | a value given to `App::provide` | 500 naming the type if none was |
| `renox::Cookies` | the request's own cookies | below |
| `renox::signed::ValidSignature` | proof the URL was signed and hasn't expired | 403 otherwise |
| `State<AppState>` | the whole app: `db`, `config`, `cache`, `storage`, `url`, … | `State<Db>` for just the database |

```rust
use renox::prelude::*;
use renox::{Provided, RequestId};

#[derive(serde::Deserialize)]
struct Filter {
    q: Option<String>,
}

#[derive(Clone)]
struct Payments {
    api_key: String,
}

async fn search(
    user: Option<AuthUser>,
    Query(filter): Query<Filter>,
    ClientIp(ip): ClientIp,
    id: RequestId,
    lang: Lang,
    payments: Provided<Payments>,
) -> String {
    format!(
        "{} searched {:?} from {:?} ({id}, {}, key {}…)",
        user.map_or("a guest".to_owned(), |u| u.email.clone()),
        filter.q,
        ip,
        lang.locale,
        &payments.api_key[..3],
    )
}

fn app() -> App {
    // `Provided<T>` in handlers; `state.provided::<T>()` in jobs, listeners and commands.
    App::new().provide(Payments { api_key: "sk_test_123".into() })
}
```

### Route model binding: `Found<M>`

`Found(product): Found<Product>` loads the row a route parameter names, or answers 404 like a
missing route. The parameter is the one named after the model's table (`{product}` for
`Product`), else the route's only one. It is read as the model's key (`{product}`, `{id}`, a
ULID or UUID too), or matched against a column when its name is one (`/blog/{slug}` finds the
post whose `slug` matches). The query is the model's own, so a default scope (the current
team) and soft deletes apply: another team's id, or a trashed row, is a 404.

```rust
use renox::prelude::*;
# #[derive(Model, serde::Serialize, Default)] struct Team { id: i64, name: String }
# #[derive(Model, serde::Serialize, Default)] struct Post { id: i64, slug: String, title: String }

// GET /teams/{team}/posts/{post}: each model takes the parameter named after it.
async fn show(Found(team): Found<Team>, Found(post): Found<Post>) -> String {
    format!("{}: {}", team.name, post.title)
}

// GET /blog/{slug}: `slug` is a column, so the post is found by it.
async fn by_slug(Found(post): Found<Post>) -> String {
    post.title
}
```

Authorization stays in the handler (`user.authorize("update", &post)?`), as with `find_or_404`.

## What a handler can return

Anything axum turns into a response, and Renox's own:

| Return | Response |
|---|---|
| `View` (`view("x.html", context! {…})`) | a rendered page; `.fragment("block")` for htmx, `.also(…)` for out-of-band swaps |
| `Redirect::to("/x")` | 303 |
| `Redirect::route("x", &[&id])?` | 303 to a named route |
| `Redirect::intended(&session, "/dashboard")` | 303 to the page a guard sent the user away from (once, same site only), else the fallback |
| `Back` (extractor and response) | 303 to the `Referer`, or `/` when it's missing or on another site |
| `Json(json!({…}))`, `String`, `Html(…)`, `StatusCode` | axum's |
| `renox::Download` | a file: `bytes`, `file` (streamed), `from_storage`, `stream`; `.inline()` to show it in the browser |
| `renox::Toast::success("…")` in a tuple | a toast with the response (or the next page after a redirect) |
| `HxRedirect`, `HxRefresh`, `HxTrigger`, `HxRetarget`, `HxReswap`, `HxPushUrl` | htmx response headers; see [ui.md](ui.md) |
| `Err(…)` from `Result<T>` | an error page (`errors/{status}.html`), or JSON for API clients |

```rust
use renox::prelude::*;
use renox::{Download, Toast};

async fn store(session: Session) -> Result<(Toast, Redirect)> {
    session.flash("status", "Saved")?; // or a toast, which needs no template code
    let _back = Redirect::intended(&session, "/products");
    Ok((Toast::success("Product saved."), Redirect::route("products.show", &[&42])?))
}

async fn invoice(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Download> {
    abort_if(id <= 0, StatusCode::NOT_FOUND, "No such invoice.")?; // any status with a message
    Download::from_storage(&state.storage, &format!("invoices/{id}.pdf"), "invoice.pdf")
        .await
        .map(Download::inline) // a missing key is a 404
}

async fn ping() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}
```

A filename given to `Download` is made safe for `Content-Disposition`. `abort(status, msg)`,
`abort_if` and `abort_unless` stop a handler with a status and a message visitors see.

## Middleware

### The order around your handler

Every request passes these, outermost first. You rarely need the details; they explain what
your own middleware can rely on.

1. **Security headers** (nosniff, Referrer-Policy, X-Frame-Options, HSTS, the CSP) and the
   client IP.
2. **Method spoofing**: a POST with `_method=PUT|PATCH|DELETE` is routed as that method.
3. **Request id**, then the request's log span; then the **body limit** (`UPLOAD_MAX_SIZE`).
4. Renox's scripts, `/health`, `/robots.txt`, `/favicon.ico` and the local disk's public files
   answer here, before sessions and maintenance mode.
5. The request's context (`renox::context`), the **session**, the **locale**, the **user**
   (from the session or an `Authorization: Bearer` token), **CSRF**, **views** (renders `View`s
   and error pages), **maintenance mode**, and a guard that turns a panic or a run past
   `REQUEST_TIMEOUT` into a 500 page.
6. Your `App::layer` layers, first added = outermost.
7. The route's own layers (guards, throttles, `route_layer`), then the handler.

So an `App::layer` middleware already sees the session, `AuthUser`, `Lang` and the context,
and its response still gets the security headers and error pages.

### Your own middleware

Write a function with axum's `from_fn` signature. `App::layer` wraps every route of the app's
modules; `Routes::route_layer` wraps only the routes added **before** it in that `Routes`, like
every guard below.

```rust
use renox::prelude::*;
use renox::axum::extract::Request;
use renox::axum::middleware::{Next, from_fn};

async fn stamp(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await; // the handler (and inner layers) run here
    let who = if user.is_some() { "member" } else { "guest" };
    res.headers_mut().insert("x-visitor", who.parse().unwrap());
    res
}

fn app() -> App {
    App::new().layer(from_fn(stamp)) // every route
}

fn routes() -> Routes {
    Routes::new()
        .get("/reports", || async { "reports" })
        .route_layer(from_fn(stamp)) // only /reports
        .get("/about", || async { "about" }) // not wrapped: added after
}
```

Don't keep a closure that borrows `req` alive across `next.run(req).await` (scope it in a
block), or the handler future isn't `Send` and won't compile as a route. To hand a value to
handlers, use `renox::context::set(value)` in the middleware and the `Current<T>` extractor
(see [authorization.md](authorization.md) "Tenants").

### Guards

Each covers the routes added before it, so put it after them. Guests asking for a page are
sent to the `login` route (and back afterwards, through `Redirect::intended`); API clients
(`Accept: application/json` or an `Authorization` header) get 401 JSON instead.

| Guard | Lets through | Others |
|---|---|---|
| `.require_auth()` | logged-in users | guests → `login` |
| `.require_verified()` | users with a verified email | → `verification.notice` |
| `.guest_only()` | guests (login, register pages) | users → `home` |
| `.require_gate("x")` | users a gate (or permission) allows | 403 |
| `.require_role("admin")`, `.require_permission("x")` | the `Permissions` module | 403 |
| `.require_ability("orders:write")` | API tokens with the ability; sessions and unrestricted tokens | 403 |
| `.require_password_confirmed()` | users who typed their password in the last three hours | → `/confirm-password` |

```rust
use renox::prelude::*;

# async fn index() -> &'static str { "" }
# async fn store() -> &'static str { "" }
# async fn login() -> &'static str { "" }
fn routes() -> Routes {
    let public = Routes::new().get("/posts", index);
    let members = Routes::new().post("/posts", store).require_auth(); // POST /posts only
    let guests = Routes::new().get("/signin", login).guest_only();
    public.merge(members).merge(guests)
}
```

A route added after a guard in the same `Routes` is **not** covered. Splitting routes into
separate `Routes` values and merging them, as above, keeps that visible.

### Rate limits

`.throttle(max, per)` counts per logged-in user, or per IP for guests; over the limit the
answer is 429 with `Retry-After`, and allowed responses carry `X-RateLimit-Limit` and
`X-RateLimit-Remaining`. For limits that depend on the request, name a limiter:

```rust
use renox::prelude::*;
use renox::rate_limit::Limit;
use std::time::Duration;

# async fn orders() -> &'static str { "" }
# async fn login() -> &'static str { "" }
fn app() -> App {
    App::new().rate_limiter("api", |req| match req.user {
        Some(user) if user.has_role("partner") => Limit::none(),
        Some(_) => Limit::per_minute(600),
        None => Limit::per_minute(60).by(format!("ip:{:?}", req.ip)), // `by`: your own key
    })
}

fn routes() -> Routes {
    Routes::new()
        .get("/api/orders", orders).throttle_by("api")
        .post("/contact", login).throttle(5, Duration::from_secs(60))
}
```

A rule sees `req.user`, `req.ip`, `req.method`, `req.path` and `req.headers`. Counters live in
memory, so each server counts on its own; with `CACHE_STORE=database` they are shared
through the `cache` table (and so is the login lock).

### CORS

`.cors(&["https://app.example.com"])` (or `&["*"]`) lets browsers on those origins `fetch` the
routes added before it: preflights are answered and the `Access-Control-Allow-*` headers
added. For credentials or other headers, build a `renox::cors::CorsLayer` and pass it to
`.cors_layer(…)`.

### ETags

`.etag()` gives the routes added before it an `ETag` header, a hash of the page as it is sent,
and answers **304 Not Modified** without the body when the browser's `If-None-Match` names it:
for feeds, sitemaps, catalogues and API lists that are fetched again and again but rarely
change. Only `GET`/`HEAD` answers with status 200 and a body of at most 2 MB get one.

```rust
use renox::prelude::*;
# async fn feed() -> &'static str { "" }
# async fn sitemap() -> &'static str { "" }

fn routes() -> Routes {
    Routes::new()
        .get("/feed.xml", feed)
        .get("/sitemap.xml", sitemap)
        .etag() // covers the two routes above
}
```

## Sessions

`Session` is the visitor's session. By default it's an encrypted, signed cookie (key derived
from `APP_KEY`), so it needs no database; keep it small (browsers drop cookies over 4 KB, and
Renox logs a warning). `SESSION_DRIVER=database` keeps only an id in the cookie and the data in
the `sessions` table; prune expired rows with `my-app session:prune`.

```rust
use renox::prelude::*;

async fn cart(session: Session) -> Result<String> {
    session.put("cart", vec![1, 2, 3])?;
    session.push("recent", 42)?;                 // append to a list
    let visits = session.increment("visits", 1)?; // 1, 2, …
    let cart: Option<Vec<i64>> = session.get("cart");
    let coupon: Option<String> = session.pull("coupon"); // read and remove
    session.flash("status", "Added to the cart")?; // the next request only
    Ok(format!("{visits} {cart:?} {coupon:?}"))
}
```

| Method | Does |
|---|---|
| `get`, `has`, `put`, `remove`, `pull` | read and write values (any `Serialize` type) |
| `push`, `increment` | append to a list; add to a number |
| `flash`, `reflash`, `flashed` | values for the next request only ("Saved!" after a redirect) |
| `keep(&["status"])`, `now(key, value)` | keep some flashed values for one more request; a flash value for the page rendered now only (Laravel's `flash()->now()`) |
| `old(field)`, `errors()` | the previous form's input and validation errors (filled by `Valid<T>`) |
| `has_old_input` | whether the previous request flashed its input (a failed submit), even with no field in it |
| `set_lifetime(duration)` | this session lasts longer than `SESSION_LIFETIME` |
| `token`, `regenerate_token`, `flush` | the CSRF token; a new one; empty everything |

- Lifetime: `SESSION_LIFETIME` minutes of inactivity (120). "Remember me" on Renox's login
  page sets `REMEMBER_LIFETIME` (30 days) with `set_lifetime`.
- Logging in and out gives the session a new id (with the database driver) and a new CSRF
  token. Logging out ends this device's session (the user's other devices stay logged in;
  `auth::logout_other_devices` ends those); changing the password ends the others.
- In templates: `flash.status`, `old('email')`, `errors`, `csrf_field()`.

## CSRF

Every POST, PUT, PATCH and DELETE must carry the session's token; without it the answer is
**419 Page Expired**.

- **Forms:** `{{ csrf_field() }}` inside the `<form>` (a hidden `_token` field; urlencoded and
  multipart both work).
- **htmx:** nothing to do. `{{ renox_head() }}` puts the token in a `<meta name="csrf-token">`
  and Renox's script sends it as `X-CSRF-Token` on every htmx request.
- **`fetch`:** send the `X-CSRF-Token` header yourself (read the meta tag).
- **API tokens** (`Authorization: Bearer`) skip CSRF: browsers never send them on their own.
  A Bearer token that doesn't authenticate gets 401.
- **No session at all** (a payment gateway): `.without_csrf()` on those routes, and check the
  request's signature instead. `Routes::webhook::<W>("/webhooks/stripe")` does both for you.
- **A JavaScript client on the same site** (axios, a small SPA): `App::new().xsrf_cookie()`
  also sends the token as an `XSRF-TOKEN` cookie that scripts can read, and accepts it back in
  an `X-XSRF-TOKEN` header, as Laravel does. axios sends it by itself.

## Method spoofing

HTML forms can only GET and POST. A POST with `_method=PUT|PATCH|DELETE` (a form field, or the
`X-HTTP-Method-Override` header) is routed as that method; it happens before routing, so
`.put(…)` and `.delete(…)` routes match. htmx can send the real method (`hx-delete`).

```html
<form method="post" action="{{ route('products.update', product.id) }}">
  {{ csrf_field() }}{{ method_field('PUT') }}
  …
</form>
```

## Cookies

The session covers most needs. For a cookie of your own (a theme, a consent banner), read with
the `Cookies` extractor and set with `SetCookie` in the response:

```rust
use renox::prelude::*;
use renox::{Cookies, SetCookie};
use std::time::Duration;

async fn remember_theme(State(state): State<AppState>) -> (SetCookie, Redirect) {
    let cookie = SetCookie::new(&state, "theme", "dark").max_age(Duration::from_secs(365 * 86_400));
    (cookie, Redirect::to("/"))
}

async fn theme(cookies: Cookies) -> String {
    cookies.get("theme").unwrap_or_default()
}

async fn secret(State(state): State<AppState>, cookies: Cookies) -> (SetCookie, String) {
    let seen = cookies.get_encrypted("seen").unwrap_or_default(); // tampered: None
    (SetCookie::encrypted(&state, "seen", "yes"), seen)
}
```

Defaults: `Path=/`, `HttpOnly`, `SameSite=Lax`, `Secure` when `APP_URL` is https, until the
browser closes. Change them with `max_age`, `path`, `readable_by_scripts` (no `HttpOnly`) and
`strict` (`SameSite=Strict`); `SetCookie::remove(&state, name)` deletes one.
`SetCookie::encrypted` seals the value with `APP_KEY`: the browser can neither read nor
change it.

## Signed URLs

A signed URL can't be altered and expires: download links in mail, unsubscribe links,
one-off invitations.

```rust
use renox::prelude::*;
use renox::signed::ValidSignature;
use std::time::Duration;

fn routes() -> Routes {
    Routes::new().get("/invoices/{id}", show_invoice).name("invoices.show")
}

async fn share(State(state): State<AppState>, Path(id): Path<i64>) -> Result<String> {
    // An absolute URL (with APP_URL) valid for an hour.
    state.signed_url("invoices.show", &[&id], Duration::from_secs(3600))
}

async fn show_invoice(_: ValidSignature, Path(id): Path<i64>) -> String {
    format!("Invoice {id}") // an altered or expired link gets 403
}
```

The signature covers the path and expiry (HMAC-SHA256 with `APP_KEY`), so the link carries no
other query string. `state.sign_path(path, ttl)` signs a path that isn't a named route.
Rotating `APP_KEY` invalidates every link. Private storage files get temporary links with
`state.storage.temporary_url(…)`.

## Maintenance mode

```text
my-app down --secret s3cr3t --retry 60   # every request answers 503 (Retry-After: 60)
my-app up                                # back to normal
```

The 503 uses `errors/503.html` (or the built-in error page). Visiting `/s3cr3t` sets a cookie
that lets you in while the site is down. `/health`, Renox's scripts and webhook routes keep
working. The state is the file `storage/framework/down`, so every process on the machine sees
it.

## Request ids and client IPs

- **Request id:** every request gets one, kept from an incoming `X-Request-Id` when it looks
  sane (8 to 64 letters, digits, `.`, `_`, `-`) or made up. It is on the response's
  `X-Request-Id`, in every log line of the request and in error reports; read it with the
  `RequestId` extractor to show it on an error page.
- **Client IP:** behind Caddy, nginx or a load balancer every connection comes from the proxy,
  which passes the visitor's address in `X-Forwarded-For` or `Forwarded`. Those headers are
  believed only from `TRUSTED_PROXIES` (addresses and CIDR ranges, or `*` for whoever
  connects, e.g. a platform's load balancer). `ClientIp`, rate limits, the login lock and the
  logs all use the same address.

```text
TRUSTED_PROXIES=127.0.0.1,10.0.0.0/8
```

- **Trusted hosts:** with `TRUSTED_HOSTS=example.com,*.example.com`, a request for any other
  host gets a 400, so links built from the `Host` header (password reset mails, redirects)
  can't point at someone else's site. `APP_URL`'s host is always allowed, and `/health`
  answers whatever the host, since load balancers check it by IP. Unset: any host.

See [operations.md](operations.md) for timeouts, proxies and `/health` in production.

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `routes/web.php` | `Module::routes()`, one module per area |
| `Route::get('/x', …)->name('x')` | `.get("/x", handler).name("x")` |
| `Route::prefix('admin')->name('admin.')->group(…)` | `.group("/admin", "admin.", routes)` |
| `Route::resource('products', …)` | `.resource("/products", "products", Resource::new()…)` |
| `Route::domain('{account}.example.com')` | `.domain("{account}.example.com", routes)` + `DomainParams` |
| `Route::fallback` | `.fallback(handler)` |
| `route('x', $id)`, `url()` | `route('x', id)` in templates, `state.url("x", &[&id])` |
| `request()->routeIs('admin.*')`, `Route::currentRouteName()` | `route_is('admin.*')`, `CurrentRoute` |
| `php artisan route:list` | `my-app route:list` |
| Route model binding, `{post:slug}` | `Found<Post>` (`{post}`, `{id}`, or a column's name such as `{slug}`) |
| `Route::view`, `Route::redirect`, `Route::permanentRedirect` | `.view(path, template)`, `.redirect(from, to)`, `.permanent_redirect(from, to)` |
| `SetCacheHeaders` with `etag` | `.etag()` |
| The `XSRF-TOKEN` cookie | `App::xsrf_cookie()` |
| `TrustHosts` | `TRUSTED_HOSTS` |
| `$request->query()`, `$request->ip()` | `Query<T>`, `ClientIp` |
| Middleware, `Kernel::$middleware` | `App::layer(from_fn(…))` |
| Route middleware | `.route_layer(…)` after the routes |
| `auth`, `guest`, `verified`, `can:`, `password.confirm` | `.require_auth()`, `.guest_only()`, `.require_verified()`, `.require_gate(…)`, `.require_password_confirmed()` |
| `throttle:60,1`, `RateLimiter::for` | `.throttle(60, Duration::from_secs(60))`, `App::rate_limiter` + `.throttle_by` |
| `config/cors.php` | `.cors(&[…])`, `.cors_layer(…)` |
| `redirect()->route()`, `->intended()`, `back()` | `Redirect::route`, `Redirect::intended`, `Back` |
| `response()->download()` | `Download` |
| `session()->put/push/increment/flash` | `Session::put/push/increment/flash` |
| `@csrf`, `@method('PUT')` | `{{ csrf_field() }}`, `{{ method_field('PUT') }}` |
| `VerifyCsrfToken::$except` | `.without_csrf()` |
| `Cookie::queue`, encrypted cookies | `SetCookie::new` / `SetCookie::encrypted`, `Cookies` |
| `URL::temporarySignedRoute`, `signed` middleware | `state.signed_url(…)`, `ValidSignature` |
| `php artisan down --secret` | `my-app down --secret …` |
| `TrustProxies` | `TRUSTED_PROXIES` |
| Service container bindings | `App::provide` + `Provided<T>` |

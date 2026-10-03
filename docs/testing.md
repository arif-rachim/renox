# Testing a Renox app

Tests boot the whole app in memory with `TestApp` and talk to it like a browser does: requests
go through the same router, middleware, sessions and CSRF checks as in production. The database
is in-memory SQLite, or PostgreSQL when `TEST_DATABASE_URL` is set, and it is migrated for each
test.

```rust
use renox::prelude::*;
use renox::testing::TestApp;

fn app() -> App {
    App::new().module(Auth::new())
}

#[renox::test]
async fn members_see_their_account() {
    let app = TestApp::new(app()).await;
    let user = User::register(app.db(), "Ana", "ana@example.com", "password123").await.unwrap();
    app.get("/account").await.assert_redirect("/login");
    app.acting_as(&user);
    app.get("/account").await.assert_ok().assert_see("ana@example.com");
}
```

`rnx make:test checkout` writes a test file to start from. `rnx make:module products --resource`
writes the tests of a whole resource (create, list, show, edit, update, delete, invalid input).

## Requests and responses

- **Requests:** `get`, `post(uri, &[(field, value)])`, `put`, `patch`, `delete`, `post_json`,
  `post_multipart`, `post_body`.
- **Request options:** `app.htmx()` sends as htmx does; `app.request().header(…)` adds headers,
  `.json()` asks for JSON (`Accept: application/json`, so a guest gets a 401 and invalid input a
  422 instead of redirects), and `.without_csrf()` sends without the token. `app.csrf_token()`
  is the session's token, for a request you build yourself.
- **Status:** `assert_ok`, `assert_status(n)`, `assert_redirect(to)`, `assert_hx_redirect(to)`,
  `assert_not_found`, `assert_forbidden`, `assert_unauthorized`.
- **Body:** `assert_see` / `assert_dont_see` (HTML as sent, escaped), `assert_invalid("field")`,
  `assert_header`, `text()`, `json::<T>()`, `header(name)` (`Option<&str>`). The fields `status`,
  `headers`, `body` and `view` are public too.
- **Invalid input:** `assert_invalid("field")` expects a 422 with an error on that field, which
  only htmx and JSON requests get: send with `app.htmx()` or `app.request().json()`. A plain form
  post is answered with a 303 back to the form (errors and old input flashed), so for that one
  check `assert_status(303)`, then `get` the form and `assert_see` the message.
- **JSON:**
  - `assert_json_path("data.0.name", "Coffee")` checks the value at a path of keys and indexes;
  - `json_path(path)` reads it;
  - `assert_json(json!({ … }))` checks the body contains the expected keys (other keys may
    be there too).
- **Views:** `assert_view("products/index.html")` checks the template a page was rendered from.

## Configuration and the app's parts

`TestApp::new` starts from `Config::default()` (in-memory database, the memory mailer, no
workers or scheduler, debug on) and doesn't read `.env`. Change it with
`TestApp::with_config(app, |c| …)`; `app.state()` is the `AppState` handlers get,
`app.db()` its database and `app.mailer()` its mailer (`mailer().sent()` lists the mail).

```rust
use renox::prelude::*;
use renox::http::FakeResponse;
use renox::testing::TestApp;

#[renox::test]
async fn rates_come_from_the_api() {
    let app = TestApp::with_config(App::new(), |c| {
        c.vars.insert("RATES_KEY".into(), "test-key".into()); // what config.var reads
        c.debug = false; // production error pages
    })
    .await;
    assert_eq!(app.state().config.var("RATES_KEY").as_deref(), Some("test-key"));

    let http = app.fake_http(); // `*` matches anything; "POST https://…" for one method
    http.on("https://api.example.com/*", FakeResponse::json(200, json!({ "idr": 16000.0 })));
    let res = app.state().http.get("https://api.example.com/rates").send().await.unwrap();
    assert_eq!(res.status(), 200);
    http.assert_sent(|r| r.method == "GET" && r.url.starts_with("https://api.example.com/rates"));
    http.assert_not_sent(|r| r.method == "POST");
    http.assert_sent_count(1); // http.sent() lists them: method, url, headers, body, json()
}
```

Also `FakeResponse::text(status, body)`, `FakeResponse::status(n)`, `.header(…)`, and
`FakeResponse::connection_error()`; several answers for one pattern are given in turn.

## Session and login

- `acting_as(&user)` logs in; `logout()` forgets the session; `confirm_password()` passes
  `require_password_confirmed`.
- `assert_authenticated(Some(&user))` / `assert_authenticated(None)` / `assert_guest()`.
- `assert_session_has("cart")`, `assert_session_missing("cart")`, `session_get::<T>("cart")`.
- `session_cookie()` and `use_session_cookie(…)` play a second device.

## The database

- `assert_database_has("orders", &[("status", &"paid")])`, `assert_database_missing(…)`,
  `assert_database_count("orders", 3)`.
- `renox::db::capture_queries(future)` returns what the future ran, requests included, so a
  test can catch an N+1:
  `let (res, queries) = capture_queries(app.get("/posts")).await; assert!(queries.len() <= 3);`
- Factories fill tables: `Product::create_one(app.db()).await`, `Product::create_many(app.db(),
  20).await`, or with states and sequences: `Product::factory().count(3).state(sold_out)
  .sequence(|i, p| p.name = format!("Coffee {i}")).create(app.db()).await` (`make()` for unsaved
  models; `factory().state(…).make_one()` / `.create_one(db)` for a single one).

## Jobs, events, notifications, mail, HTTP

| Tool | What it does |
|---|---|
| `app.queued_jobs()` | The names of the queued jobs. |
| `app.run_jobs()` | Runs the jobs that are due. |
| `app.run_all_jobs()` | Also runs delayed jobs and retries still waiting for their backoff. |
| `app.fake_events()` | Records events instead of running their listeners. Check them with `assert_emitted::<OrderPlaced>(\|e\| e.id == 7)`, `emitted::<E>()` or `assert_not_emitted::<E>()`. |
| `app.fake_notifications()` | Records notifications instead of sending them. Check them with `assert_notified(&user, "order-shipped")`, `assert_notified_to("a@b.c", kind)`, `notifications()` or `assert_nothing_notified()`. |
| `app.sent_mail()`, `app.assert_mail_sent(to, subject)` | The mail sent so far (the test mailer keeps it). |
| `app.fake_http()` | Answers `state.http` requests with fakes and records them. A request without a fake is an error, so nothing reaches the network. |
| `app.kernel().run_scheduled("report")` | Runs a scheduled task now. |
| `app.kernel().call("products:import", ["a.csv"])` | Runs one of the app's own commands. |
| `renox::prompt::answering(["a.csv", "yes"], app.kernel().call("products:import", [""; 0])).await` | Runs a command that asks questions (`renox::prompt::ask`, `confirm`, …), answering them in order. |
| `my_app::app().run_args(["migrate:status"]).await` | Runs any command the binary has, built-ins included (`migrate`, `queue:failed`, `down`…), as `my-app migrate:status` would. Give the app a file database: each call boots it anew. |

## Time

`app.travel(Duration::from_secs(3600))` moves the clock forward for what the `TestApp` does
next: requests, `run_jobs` and `run_all_jobs`. `renox::db::now()`, sessions, signed URLs, the
queue, the cache, rate limits, the login lock and password confirmation all follow it, and
so do `TestApp`'s own session helpers (past `SESSION_LIFETIME`, the next request starts a new
session with its own CSRF token). `app.at_travelled_time(fut)` runs other code, such as a
model call, a scheduled task (`app.kernel().run_scheduled(..)`) or a command, at that time.
`app.travel_back()` returns to the present. Travel adds up.

Travel reaches only time read through Renox: `renox::db::now()` in app code, not
`SystemTime::now()` or `chrono::Utc::now()`. Prefer it to rewriting `created_at` with SQL or
sleeping; examples/shop, jobs, api and hello show it.

```rust
# use renox::prelude::*;
# use std::time::Duration;
# async fn demo(app: renox::testing::TestApp) {
app.confirm_password();
app.travel(Duration::from_secs(4 * 60 * 60)); // confirmation lasts three hours
app.delete("/account").await.assert_redirect("/confirm-password");
# }
```

## Browser tests

Most of an app is covered by `TestApp` requests. For what only a browser shows (a sheet opening,
live validation, a layout at phone width), serve the app on a real port and drive a browser over
the Chrome DevTools Protocol:

```rust
# async fn demo(app: renox::testing::TestApp) {
let url = app.serve().await; // http://127.0.0.1:PORT, until the test ends
// Point a WebDriver or CDP client at `url` (fantoccini, chromiumoxide, or a Node script),
// e.g. open `{url}/products/new`, type, press Tab, and read `[data-error-for=price]`.
# let _ = url; }
```

Renox's own browser checks run headless Chrome with `--remote-debugging-port` and a short script
per page. The steps:

1. Log in through the form, then open the page.
2. Do what a person would: click, type, press keys with `Input.dispatchKeyEvent` (synthetic
   events don't trigger every handler).
3. Read the DOM, and take screenshots at 1100 px, 390 px (a phone) and in dark mode.
4. Check the console for errors and CSP violations.

Look at the screenshots, not only the numbers: layout problems (an element pushed off screen, a
misaligned dialog) pass every assertion.

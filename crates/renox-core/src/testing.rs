//! Testing helpers, in the spirit of Laravel's HTTP tests.
//!
//! ```
//! # use renox::prelude::*;
//! # mod toko { pub fn app() -> renox::App { renox::App::new() } }
//! use renox::testing::TestApp;
//!
//! #[renox::test]
//! async fn creating_a_product() {
//!     let app = TestApp::new(toko::app()).await;           // in-memory DB, migrated
//!     let user = User::register(app.db(), "Arif", "arif@example.com", "rahasia123").await.unwrap();
//!
//!     app.acting_as(&user)
//!         .post("/produk", &[("nama", "Kopi"), ("harga", "18000")])
//!         .await
//!         .assert_redirect("/produk");
//!     app.assert_database_has("produk", &[("nama", &"Kopi")]).await;
//!     app.get("/produk").await.assert_ok().assert_see("Kopi");
//! }
//! ```
//!
//! A `TestApp` runs the app's router in memory with a test configuration:
//! an in-memory SQLite database with all migrations run, the `memory` mail
//! driver, no background workers or scheduler, storage in a temporary
//! directory, and the app's own `resources/` and `public/` directories. It
//! keeps the session cookie between requests like a browser and sends the
//! CSRF token itself, so tests read like user actions.

use std::sync::Mutex;

use axum::body::{Body, Bytes};
use axum::http::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

use crate::auth::{User, login};
use crate::db::{Db, DbValue, ToDbValue, quote};
use crate::mail::{Mail, Mailer};
use crate::{App, AppState, Config, Environment, Kernel};

/// The app under test, with a browser-like client.
pub struct TestApp {
    kernel: Kernel,
    cookie: Mutex<Option<String>>,
    /// Seconds the clock is moved by (`travel`).
    offset: std::sync::atomic::AtomicI64,
    _storage: tempfile::TempDir,
}

impl TestApp {
    /// Boots `app` with the test configuration and runs its migrations.
    pub async fn new(app: App) -> Self {
        Self::with_config(app, |_| {}).await
    }

    /// Like `new`, after `configure` adjusts the test configuration.
    pub async fn with_config(app: App, configure: impl FnOnce(&mut Config)) -> Self {
        let storage = tempfile::tempdir().expect("a temporary storage directory");
        let mut config = Config {
            env: Environment::Testing,
            key: Some(crate::generate_key()),
            storage_path: storage.path().to_path_buf(),
            ..Config::default()
        };
        configure(&mut config);
        let kernel = app.config(config).boot().await.expect("the app boots");
        kernel.migrate().await.expect("migrations run");
        Self {
            kernel,
            cookie: Mutex::new(None),
            offset: std::sync::atomic::AtomicI64::new(0),
            _storage: storage,
        }
    }

    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    pub fn state(&self) -> &AppState {
        self.kernel.state()
    }

    pub fn db(&self) -> &Db {
        self.kernel.db()
    }

    pub fn mailer(&self) -> &Mailer {
        self.kernel.mailer()
    }

    /// Answers `state.http` requests with fakes and records them, instead
    /// of reaching the network; see [`crate::http::FakeHttp`].
    pub fn fake_http(&self) -> crate::http::FakeHttp {
        self.state().http.fake()
    }

    /// Mail sent so far.
    pub fn sent_mail(&self) -> Vec<Mail> {
        self.kernel.mailer().sent()
    }

    /// Runs the jobs queued so far; returns how many ran.
    pub async fn run_jobs(&self) -> usize {
        self.at_travelled_time(self.kernel.run_jobs())
            .await
            .expect("the queue runs")
    }

    /// Moves the clock forward by `by` for what this `TestApp` does next:
    /// requests, `run_jobs`, and code run in [`TestApp::at_travelled_time`]
    /// (`renox::db::now()`, sessions, signed URLs, the queue, the cache).
    /// Adds up; [`TestApp::travel_back`] returns to the present.
    ///
    /// ```
    /// # use renox::prelude::*;
    /// # use std::time::Duration;
    /// # async fn demo(app: renox::testing::TestApp) {
    /// app.travel(Duration::from_secs(3 * 60 * 60)); // past the password confirmation
    /// app.get("/account/delete").await.assert_redirect("/confirm-password");
    /// # }
    /// ```
    pub fn travel(&self, by: std::time::Duration) -> &Self {
        let seconds = i64::try_from(by.as_secs()).unwrap_or(i64::MAX);
        self.offset
            .fetch_add(seconds, std::sync::atomic::Ordering::SeqCst);
        self
    }

    /// Returns the clock to the present.
    pub fn travel_back(&self) -> &Self {
        self.offset.store(0, std::sync::atomic::Ordering::SeqCst);
        self
    }

    /// Runs `fut` with the clock where [`TestApp::travel`] moved it, e.g.
    /// to call a model or a job's code directly.
    pub async fn at_travelled_time<F: std::future::Future>(&self, fut: F) -> F::Output {
        let offset = self.offset.load(std::sync::atomic::Ordering::SeqCst);
        crate::clock::with_offset(offset, fut).await
    }

    /// Records events instead of running their listeners, from now on; read
    /// them with [`TestApp::emitted`] or [`TestApp::assert_emitted`].
    pub fn fake_events(&self) -> &Self {
        let mut events = self
            .state()
            .fakes
            .events
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        events.get_or_insert_with(Vec::new);
        self
    }

    /// The events of type `E` emitted since `fake_events`, oldest first.
    pub fn emitted<E: crate::events::Event>(&self) -> Vec<E> {
        let events = self
            .state()
            .fakes
            .events
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        events
            .iter()
            .flatten()
            .filter_map(|event| event.downcast_ref::<E>().cloned())
            .collect()
    }

    /// Panics unless an `E` matching `check` was emitted.
    #[track_caller]
    pub fn assert_emitted<E: crate::events::Event>(&self, check: impl Fn(&E) -> bool) -> &Self {
        let emitted = self.emitted::<E>();
        assert!(
            emitted.iter().any(check),
            "no matching {} was emitted ({} of that type)",
            std::any::type_name::<E>(),
            emitted.len()
        );
        self
    }

    #[track_caller]
    pub fn assert_not_emitted<E: crate::events::Event>(&self) -> &Self {
        let emitted = self.emitted::<E>();
        assert!(
            emitted.is_empty(),
            "{} {} emitted",
            emitted.len(),
            std::any::type_name::<E>()
        );
        self
    }

    /// Records notifications instead of sending them (mail, database,
    /// channels), from now on.
    pub fn fake_notifications(&self) -> &Self {
        let mut sent = self
            .state()
            .fakes
            .notifications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sent.get_or_insert_with(Vec::new);
        self
    }

    /// The notifications recorded since `fake_notifications`, oldest first.
    pub fn notifications(&self) -> Vec<crate::SentNotification> {
        let sent = self
            .state()
            .fakes
            .notifications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sent.iter().flatten().cloned().collect()
    }

    /// Panics unless `user` was sent a notification of `kind`.
    #[track_caller]
    pub fn assert_notified(&self, user: &User, kind: &str) -> &Self {
        let sent = self.notifications();
        assert!(
            sent.iter()
                .any(|n| n.kind == kind && n.to.user.as_ref().is_some_and(|u| u.id == user.id)),
            "user {} got no `{kind}` notification; sent: {:?}",
            user.id,
            sent.iter().map(|n| n.kind).collect::<Vec<_>>()
        );
        self
    }

    /// Panics unless someone with `address` (on any channel, or a user's
    /// email) was sent a notification of `kind`.
    #[track_caller]
    pub fn assert_notified_to(&self, address: &str, kind: &str) -> &Self {
        let sent = self.notifications();
        assert!(
            sent.iter().any(|n| n.kind == kind
                && (n.to.routes.values().any(|a| a == address)
                    || n.to.email().as_deref() == Some(address))),
            "`{address}` got no `{kind}` notification"
        );
        self
    }

    #[track_caller]
    pub fn assert_nothing_notified(&self) -> &Self {
        let sent = self.notifications();
        assert!(sent.is_empty(), "{} notification(s) were sent", sent.len());
        self
    }

    /// A value in the session as the next request will see it.
    pub fn session_get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.session().get(key)
    }

    #[track_caller]
    pub fn assert_session_has(&self, key: &str) -> &Self {
        assert!(
            self.session().get::<serde_json::Value>(key).is_some(),
            "the session has no `{key}`"
        );
        self
    }

    #[track_caller]
    pub fn assert_session_missing(&self, key: &str) -> &Self {
        assert!(
            self.session().get::<serde_json::Value>(key).is_none(),
            "the session has `{key}`"
        );
        self
    }

    /// Panics unless the session is logged in (as `user`, when given).
    #[track_caller]
    pub fn assert_authenticated(&self, user: Option<&User>) -> &Self {
        let id: Option<i64> = self.session().get(crate::auth::AUTH_ID);
        match (id, user) {
            (None, _) => panic!("expected a logged-in session, it's a guest"),
            (Some(id), Some(user)) => assert_eq!(id, user.id, "logged in as another user"),
            _ => {}
        }
        self
    }

    #[track_caller]
    pub fn assert_guest(&self) -> &Self {
        let id: Option<i64> = self.session().get(crate::auth::AUTH_ID);
        assert!(id.is_none(), "expected a guest, logged in as user {id:?}");
        self
    }

    /// Serves the app on a free local port and returns its base URL
    /// (`http://127.0.0.1:…`), e.g. for a browser test (docs/testing.md).
    /// The server runs until the test's runtime ends.
    pub async fn serve(&self) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let router = self.kernel.router();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        url
    }

    /// Runs every queued job, including those waiting for a delay or a
    /// retry's backoff, until none is left (at most 1,000 rounds); returns
    /// how many attempts ran. For tests of retries and `failed` hooks.
    pub async fn run_all_jobs(&self) -> usize {
        let mut ran = 0;
        for _ in 0..1000 {
            crate::db::sql("UPDATE jobs SET available_at = 0 WHERE reserved_at IS NULL")
                .execute(self.db())
                .await
                .expect("the jobs table can be written");
            let now = self.run_jobs().await;
            if now == 0 {
                break;
            }
            ran += now;
        }
        ran
    }

    /// Names of the jobs waiting in the queue, oldest first.
    pub async fn queued_jobs(&self) -> Vec<String> {
        crate::db::sql("SELECT job FROM jobs ORDER BY id")
            .scalars(self.db())
            .await
            .expect("the jobs table can be read")
    }

    /// Logs `user` in for the following requests.
    pub fn acting_as(&self, user: &User) -> &Self {
        let session = self.session();
        login(&session, user, None).expect("the session accepts the login");
        self.set_cookie(crate::session::cookie_pair(self.state(), &session));
        self
    }

    /// Marks the password as just typed, so routes behind
    /// `require_password_confirmed` let the user through.
    pub fn confirm_password(&self) -> &Self {
        let session = self.session();
        crate::auth::account::mark_confirmed(&session).expect("the session accepts it");
        self.set_cookie(crate::session::cookie_pair(self.state(), &session));
        self
    }

    /// Forgets the session, like a browser with cookies cleared.
    pub fn logout(&self) -> &Self {
        *self.cookie.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self
    }

    /// The session cookie as the browser holds it (`name=value`), e.g. to
    /// play a second device: save one, log in again, switch back with
    /// [`TestApp::use_session_cookie`].
    pub fn session_cookie(&self) -> Option<String> {
        self.cookie
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Sends this session cookie from now on (see [`TestApp::session_cookie`]).
    pub fn use_session_cookie(&self, cookie: Option<String>) -> &Self {
        *self.cookie.lock().unwrap_or_else(|e| e.into_inner()) = cookie;
        self
    }

    fn session(&self) -> crate::Session {
        let cookie = self
            .cookie
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        crate::session::from_cookie(self.state(), cookie.as_deref())
    }

    fn set_cookie(&self, pair: String) {
        *self.cookie.lock().unwrap_or_else(|e| e.into_inner()) = Some(pair);
    }

    /// The CSRF token of the current session, creating the session if needed.
    pub fn csrf_token(&self) -> String {
        let session = self.session();
        self.set_cookie(crate::session::cookie_pair(self.state(), &session));
        session.token()
    }

    /// A request with extra headers, e.g. `app.request().htmx().post(..)`.
    pub fn request(&self) -> TestRequest<'_> {
        TestRequest {
            app: self,
            headers: Vec::new(),
            csrf: true,
        }
    }

    /// Shorthand for `request().htmx()`.
    pub fn htmx(&self) -> TestRequest<'_> {
        self.request().htmx()
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        self.request().get(uri).await
    }

    /// A form post (urlencoded), with the CSRF token.
    pub async fn post(&self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.request().post(uri, form).await
    }

    pub async fn put(&self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.request().put(uri, form).await
    }

    pub async fn patch(&self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.request().patch(uri, form).await
    }

    pub async fn delete(&self, uri: &str) -> TestResponse {
        self.request().delete(uri).await
    }

    /// A JSON post, with the CSRF token.
    pub async fn post_json(&self, uri: &str, body: &impl Serialize) -> TestResponse {
        self.request().post_json(uri, body).await
    }

    /// A multipart form with files; see `TestRequest::post_multipart`.
    pub async fn post_multipart(
        &self,
        uri: &str,
        fields: &[(&str, &str)],
        files: &[(&str, &str, &[u8])],
    ) -> TestResponse {
        self.request().post_multipart(uri, fields, files).await
    }

    /// A POST with exactly these bytes (e.g. a signed webhook); no CSRF token.
    pub async fn post_body(
        &self,
        uri: &str,
        content_type: &str,
        body: impl Into<Vec<u8>>,
    ) -> TestResponse {
        self.request()
            .without_csrf()
            .post_body(uri, content_type, body)
            .await
    }

    async fn where_count(&self, table: &str, values: &[(&str, &(dyn ToDbValue + Sync))]) -> i64 {
        let mut sql = format!("SELECT COUNT(*) FROM {}", quote(table));
        let clauses: Vec<String> = values
            .iter()
            .map(|(column, value)| match value.to_db_value() {
                DbValue::Null => format!("{} IS NULL", quote(column)),
                _ => format!("{} = ?", quote(column)),
            })
            .collect();
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        crate::db::sql(sql)
            .bind_all(
                values
                    .iter()
                    .map(|(_, v)| v.to_db_value())
                    .filter(|v| *v != DbValue::Null),
            )
            .scalar(self.db())
            .await
            .unwrap_or_else(|err| panic!("could not query `{table}`: {err}"))
    }

    /// Fails unless a row of `table` has all these column values.
    pub async fn assert_database_has(
        &self,
        table: &str,
        values: &[(&str, &(dyn ToDbValue + Sync))],
    ) {
        if self.where_count(table, values).await == 0 {
            panic!("expected `{table}` to have a row with {}", describe(values));
        }
    }

    /// Fails if a row of `table` has all these column values.
    pub async fn assert_database_missing(
        &self,
        table: &str,
        values: &[(&str, &(dyn ToDbValue + Sync))],
    ) {
        let count = self.where_count(table, values).await;
        if count > 0 {
            panic!(
                "expected `{table}` to have no row with {}, found {count}",
                describe(values)
            );
        }
    }

    pub async fn assert_database_count(&self, table: &str, expected: i64) {
        let count = self.where_count(table, &[]).await;
        assert_eq!(count, expected, "rows in `{table}`");
    }

    /// Fails unless a mail went to `to` with `subject` in its subject.
    pub fn assert_mail_sent(&self, to: &str, subject: &str) {
        let sent = self.sent_mail();
        if !sent
            .iter()
            .any(|m| m.is_for(to) && m.subject.contains(subject))
        {
            let list: Vec<String> = sent
                .iter()
                .map(|m| format!("{} ({})", m.subject, m.to.join(", ")))
                .collect();
            panic!("no mail to {to} about \"{subject}\"; sent: {list:?}");
        }
    }
}

fn describe(values: &[(&str, &(dyn ToDbValue + Sync))]) -> String {
    values
        .iter()
        .map(|(c, v)| format!("{c} = {:?}", v.to_db_value()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A request being built: `app.request().header("accept", "application/json").get(..)`.
pub struct TestRequest<'a> {
    app: &'a TestApp,
    headers: Vec<(String, String)>,
    csrf: bool,
}

impl TestRequest<'_> {
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Marks the request as made by HTMX.
    pub fn htmx(self) -> Self {
        self.header("hx-request", "true")
    }

    /// Asks for JSON, e.g. to get 401/422 as JSON instead of redirects.
    pub fn json(self) -> Self {
        self.header("accept", "application/json")
    }

    /// Sends no CSRF token, to test that a form is protected.
    pub fn without_csrf(mut self) -> Self {
        self.csrf = false;
        self
    }

    pub async fn get(self, uri: &str) -> TestResponse {
        self.send(Method::GET, uri, None, Body::empty()).await
    }

    pub async fn post(self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.form(Method::POST, uri, form).await
    }

    pub async fn put(self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.form(Method::PUT, uri, form).await
    }

    pub async fn patch(self, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        self.form(Method::PATCH, uri, form).await
    }

    pub async fn delete(self, uri: &str) -> TestResponse {
        self.send(Method::DELETE, uri, None, Body::empty()).await
    }

    /// A multipart form, as a browser sends one with a file input:
    /// `post_multipart("/photos", &[("title", "Kopi")], &[("photo", "kopi.png", &bytes)])`.
    pub async fn post_multipart(
        self,
        uri: &str,
        fields: &[(&str, &str)],
        files: &[(&str, &str, &[u8])],
    ) -> TestResponse {
        const BOUNDARY: &str = "renox-test-boundary-7d1f";
        let mut body = Vec::new();
        for (name, value) in fields {
            body.extend_from_slice(
                format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                    .as_bytes(),
            );
        }
        for (name, file_name, bytes) in files {
            body.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n\
                     Content-Type: application/octet-stream\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
        let content_type = format!("multipart/form-data; boundary={BOUNDARY}");
        self.send(Method::POST, uri, Some(&content_type), Body::from(body))
            .await
    }

    /// A POST with exactly these bytes, e.g. a webhook whose signature
    /// covers the raw body.
    pub async fn post_body(
        self,
        uri: &str,
        content_type: &str,
        body: impl Into<Vec<u8>>,
    ) -> TestResponse {
        self.send(
            Method::POST,
            uri,
            Some(content_type),
            Body::from(body.into()),
        )
        .await
    }

    pub async fn post_json(self, uri: &str, body: &impl Serialize) -> TestResponse {
        let body = serde_json::to_vec(body).expect("the body serializes");
        self.send(
            Method::POST,
            uri,
            Some("application/json"),
            Body::from(body),
        )
        .await
    }

    async fn form(self, method: Method, uri: &str, form: &[(&str, &str)]) -> TestResponse {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        self.send(
            method,
            uri,
            Some("application/x-www-form-urlencoded"),
            Body::from(body),
        )
        .await
    }

    async fn send(
        self,
        method: Method,
        uri: &str,
        content_type: Option<&str>,
        body: Body,
    ) -> TestResponse {
        let app = self.app;
        let token = (self.csrf && method != Method::GET).then(|| app.csrf_token());
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(cookie) = app.cookie.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            req = req.header(COOKIE, cookie);
        }
        if let Some(token) = token {
            req = req.header(crate::CSRF_HEADER, token);
        }
        if let Some(content_type) = content_type {
            req = req.header(CONTENT_TYPE, content_type);
        }
        for (name, value) in &self.headers {
            req = req.header(name.as_str(), value.as_str());
        }
        let request = app
            .kernel
            .router()
            .oneshot(req.body(body).expect("a valid request"));
        let res = app
            .at_travelled_time(request)
            .await
            .expect("the router answers");
        let view = res
            .extensions()
            .get::<crate::view::RenderedView>()
            .map(|v| v.0.clone());

        let session_cookie = format!("{}=", app.state().config.session_cookie);
        for set in res.headers().get_all(SET_COOKIE) {
            let pair = set
                .to_str()
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default();
            if pair.starts_with(&session_cookie) {
                app.set_cookie(pair.to_owned());
            }
        }
        let status = res.status();
        let headers = res.headers().clone();
        let body = res
            .into_body()
            .collect()
            .await
            .expect("the body can be read")
            .to_bytes();
        TestResponse {
            status,
            headers,
            body,
            view,
        }
    }
}

/// A response with Laravel-style assertions; they panic with the status and
/// the start of the body, and return `&Self` so they can be chained.
#[derive(Debug, Clone)]
pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    /// The template the page was rendered from, if it was a view.
    pub view: Option<String>,
}

impl TestResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body as JSON; panics if it isn't.
    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).unwrap_or_else(|err| {
            panic!(
                "the body is not the expected JSON ({err}):\n{}",
                self.excerpt()
            )
        })
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(HeaderName::from_bytes(name.as_bytes()).ok()?)
            .and_then(|v: &HeaderValue| v.to_str().ok())
    }

    fn excerpt(&self) -> String {
        let text = self.text();
        match text.char_indices().nth(600) {
            Some((i, _)) => format!("{}…", &text[..i]),
            None => text,
        }
    }

    #[track_caller]
    pub fn assert_status(&self, expected: u16) -> &Self {
        if self.status.as_u16() != expected {
            panic!(
                "expected status {expected}, got {}:\n{}",
                self.status,
                self.excerpt()
            );
        }
        self
    }

    #[track_caller]
    pub fn assert_ok(&self) -> &Self {
        self.assert_status(200)
    }

    #[track_caller]
    pub fn assert_not_found(&self) -> &Self {
        self.assert_status(404)
    }

    #[track_caller]
    pub fn assert_forbidden(&self) -> &Self {
        self.assert_status(403)
    }

    #[track_caller]
    pub fn assert_unauthorized(&self) -> &Self {
        self.assert_status(401)
    }

    /// A 3xx redirect whose `Location` is `to`.
    #[track_caller]
    pub fn assert_redirect(&self, to: &str) -> &Self {
        if !self.status.is_redirection() {
            panic!(
                "expected a redirect to {to}, got {}:\n{}",
                self.status,
                self.excerpt()
            );
        }
        let location = self.header(LOCATION.as_str()).unwrap_or_default();
        if location != to {
            panic!("expected a redirect to {to}, got one to {location}");
        }
        self
    }

    /// An `HX-Redirect` to `to` (HTMX requests).
    #[track_caller]
    pub fn assert_hx_redirect(&self, to: &str) -> &Self {
        let location = self.header("hx-redirect").unwrap_or_default();
        if location != to {
            panic!(
                "expected HX-Redirect to {to}, got `{location}` ({})",
                self.status
            );
        }
        self
    }

    /// The body contains `text` (as written, e.g. already HTML-escaped).
    #[track_caller]
    pub fn assert_see(&self, text: &str) -> &Self {
        if !self.text().contains(text) {
            panic!("expected to see {text:?} in:\n{}", self.excerpt());
        }
        self
    }

    #[track_caller]
    pub fn assert_dont_see(&self, text: &str) -> &Self {
        if self.text().contains(text) {
            panic!("expected not to see {text:?} in:\n{}", self.excerpt());
        }
        self
    }

    /// Panics unless the page was rendered from `name` (e.g.
    /// `"products/index.html"`).
    #[track_caller]
    pub fn assert_view(&self, name: &str) -> &Self {
        assert_eq!(self.view.as_deref(), Some(name), "the view rendered");
        self
    }

    /// The value at `path` in the JSON body: keys and indexes separated by
    /// dots (`data.0.name`); `null` when it isn't there.
    pub fn json_path(&self, path: &str) -> serde_json::Value {
        let body: serde_json::Value = serde_json::from_slice(&self.body).unwrap_or_default();
        path.split('.')
            .filter(|part| !part.is_empty())
            .fold(body, |value, part| match part.parse::<usize>() {
                Ok(i) if value.is_array() => value.get(i).cloned().unwrap_or_default(),
                _ => value.get(part).cloned().unwrap_or_default(),
            })
    }

    /// Panics unless the JSON body has `expected` at `path`
    /// (`assert_json_path("data.0.name", "Kopi")`).
    #[track_caller]
    pub fn assert_json_path(&self, path: &str, expected: impl Serialize) -> &Self {
        let expected = serde_json::to_value(expected).expect("a JSON value");
        assert_eq!(self.json_path(path), expected, "JSON at `{path}`");
        self
    }

    /// Panics unless the JSON body contains `expected`: every key of an
    /// object in `expected` must be there with that value (other keys may be
    /// too); arrays must match item by item.
    #[track_caller]
    pub fn assert_json(&self, expected: serde_json::Value) -> &Self {
        let body: serde_json::Value = serde_json::from_slice(&self.body).unwrap_or_default();
        assert!(
            json_contains(&body, &expected),
            "the JSON body doesn't contain {expected}; it's {body}"
        );
        self
    }

    #[track_caller]
    pub fn assert_header(&self, name: &str, value: &str) -> &Self {
        let actual = self.header(name);
        if actual != Some(value) {
            panic!("expected header {name}: {value}, got {actual:?}");
        }
        self
    }

    /// A 422 validation response (HTMX or JSON) with an error for `field`.
    #[track_caller]
    pub fn assert_invalid(&self, field: &str) -> &Self {
        self.assert_status(422);
        let body: serde_json::Value = self.json();
        if body["errors"][field]
            .as_array()
            .is_none_or(|e| e.is_empty())
        {
            panic!(
                "expected a validation error for `{field}`, got {}",
                body["errors"]
            );
        }
        self
    }
}

/// Whether `actual` has everything `expected` has.
fn json_contains(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|av| json_contains(av, v))),
        (Value::Array(a), Value::Array(e)) => {
            a.len() == e.len() && a.iter().zip(e).all(|(av, ev)| json_contains(av, ev))
        }
        _ => actual == expected,
    }
}

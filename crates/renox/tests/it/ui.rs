//! M21b: components that see the request, the UI kit, toasts, fragments
//! with out-of-band swaps, htmx response headers, live validation.

use renox::prelude::*;
use renox::testing::TestApp;
use renox::{HxPushUrl, HxReswap, HxRetarget, Toast};

#[derive(serde::Deserialize)]
struct Signup {
    name: String,
    email: String,
}

impl Validate for Signup {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(10);
        v.field("email", &self.email).required().email();
    }
}

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/form", || async { view("form.html", context! {}) })
            .post("/signup", |Valid(form): Valid<Signup>| async move {
                (
                    Toast::success(format!("Welcome, {}!", form.name)),
                    Redirect::to("/form"),
                )
            })
            .post("/save", || async {
                (
                    Toast::info("Saved <draft>"),
                    Toast::error("But the mail bounced"),
                    HxRetarget("#summary".into()),
                    HxReswap("outerHTML".into()),
                    HxPushUrl("/saved".into()),
                    "ok",
                )
            })
            .get("/list", || async {
                view("list.html", context! { count => 3 })
                    .fragment("rows")
                    .also("count")
            })
            .get("/components", || async {
                view("components.html", context! { note => "hi" })
            })
    }
}

fn views() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("components")).unwrap();
    let write = |name: &str, body: &str| std::fs::write(dir.path().join(name), body).unwrap();
    write(
        "form.html",
        r#"{% from "renox/ui.html" import input, button, form_errors %}{{ renox_ui() }}
<form method="post" action="/signup" data-live-validate>{{ csrf_field() }}{{ form_errors() }}
{{ input("name", "Name", required=true, hint="Up to 10 letters") }}
{{ input("email", "Email", type="email", required=true) }}
{{ input("nickname", "Nickname") }}{{ button("Sign up") }}</form>{{ toasts() }}"#,
    );
    write(
        "list.html",
        r#"<h1>List</h1>{% block rows %}<tr id="r1"><td>one</td></tr>{% endblock %}
{% block count %}<span id="count" hx-swap-oob="true">{{ count }}</span>{% endblock %}"#,
    );
    // A component of the app's own: a macro in another file using request
    // helpers directly.
    write(
        "components/greeting.html",
        r#"{% macro greeting() %}<p class="greet">{{ t('ui.cancel') }}|{% if auth.check %}in{% else %}out{% endif %}|{{ request.path }}|{{ csrf_field() }}|{% if once('x') %}first{% endif %}{% if once('x') %}again{% endif %}</p>{% endmacro %}"#,
    );
    write(
        "components.html",
        r#"{% from "components/greeting.html" import greeting %}{{ greeting() }}{{ greeting() }}
{% from "renox/ui.html" import alert, badge, confirm, menu, menu_link, tabs, tab_panel, table, empty, checkbox, select, textarea %}
{{ alert("Heads up", kind="warning", title="Note") }}{{ badge("new", kind="success") }}
{{ confirm("del-1", "Delete", "/items/1", "Delete it?", "Can't be undone.") }}
{% call menu("More", id="m1") %}{{ menu_link("/a", "Open") }}{% endcall %}
{{ tabs("t", [["a", "A"], ["b", "B"]], selected="b") }}{% call tab_panel("t", "a") %}pa{% endcall %}{% call tab_panel("t", "b", selected=true) %}pb{% endcall %}
{% call table(["Name", ["Total", "num"]]) %}<tr><td>x</td><td class="rx-num">1</td></tr>{% endcall %}
{{ empty("Nothing yet", "Add one") }}{{ checkbox("agree", "I agree", switch=true) }}
{{ select("size", "Size", [["s", "Small"], ["m", "Medium"]], selected="m") }}{{ textarea("bio", "Bio", value="about") }}"#,
    );
    dir
}

async fn app() -> (TestApp, tempfile::TempDir) {
    let dir = views();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(Auth::new()).module(Pages), move |c| {
        c.views_path = path
    })
    .await;
    (app, dir)
}

#[renox::test]
async fn components_see_the_request_and_refill_forms() {
    let (app, _dir) = app().await;
    let page = app.get("/form").await;
    page.assert_ok()
        .assert_see(r#"<label class="rx-label" for="rx-name">Name</label>"#)
        .assert_see(r#"aria-describedby="rx-name-hint rx-name-error""#)
        .assert_see(r#"<span class="rx-required">(optional)</span>"#)
        .assert_see(r#"/_renox/ui-"#);
    // A failed submit: the kit's fields show the errors and old input.
    app.request()
        .header("referer", "/form")
        .post(
            "/signup",
            &[("name", "Much too long a name"), ("email", "nope")],
        )
        .await
        .assert_redirect("/form");
    let page = app.get("/form").await;
    page.assert_see(r#"value="Much too long a name""#)
        .assert_see(r#"aria-invalid="true""#)
        .assert_see("data-rx-error-summary")
        .assert_see(r##"href="#rx-email""##);
    // The kit's CSS and JS are served, cached for good.
    let css = page.text();
    let url = css
        .split("href=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let res = app.get(&url).await;
    res.assert_ok()
        .assert_header("cache-control", "public, max-age=31536000, immutable");
    assert!(res.text().contains("--rx-accent"));

    let res = app.get("/components").await;
    let html = res.text();
    let greeting = html.split("<p class=\"greet\">").nth(1).unwrap();
    assert!(
        greeting.starts_with("Cancel|out|/components|<input type=\"hidden\" name=\"_token\""),
        "{greeting}"
    );
    assert_eq!(html.matches("first").count(), 1, "once() is per page");
    assert!(!html.contains("again"));
    res.assert_see(r#"role="alertdialog""#)
        .assert_see(r#"<input type="hidden" name="_method" value="DELETE">"#)
        .assert_see(r#"aria-controls="m1""#)
        .assert_see(r#"id="t-tab-b" aria-controls="t-panel-b" aria-selected="true""#)
        .assert_see(r#"role="switch""#)
        .assert_see(r#"<option value="m" selected>Medium</option>"#)
        .assert_see(">about</textarea>");
}

#[renox::test]
async fn toasts_follow_the_next_page_or_ride_htmx() {
    let (app, _dir) = app().await;
    app.post("/signup", &[("name", "Ana"), ("email", "ana@test.id")])
        .await
        .assert_redirect("/form");
    let page = app.get("/form").await;
    page.assert_see("Welcome, Ana!")
        .assert_see(r#"role="status""#);
    app.get("/form").await.assert_dont_see("Welcome, Ana!");

    let res = app.htmx().post("/save", &[]).await;
    res.assert_ok()
        .assert_header("hx-retarget", "#summary")
        .assert_header("hx-reswap", "outerHTML")
        .assert_header("hx-push-url", "/saved");
    let trigger: renox::serde_json::Value =
        renox::serde_json::from_str(res.header("hx-trigger").unwrap()).unwrap();
    assert_eq!(
        trigger["renox:toast"]["toasts"][0]["message"],
        "Saved <draft>"
    );
    assert_eq!(trigger["renox:toast"]["toasts"][1]["kind"], "error");
    // Not htmx: saved for the next page, escaped there.
    app.post("/save", &[]).await.assert_ok();
    app.get("/form")
        .await
        .assert_see("Saved &lt;draft&gt;")
        .assert_see(r#"role="alert""#)
        .assert_see("data-sticky");
}

#[renox::test]
async fn fragments_carry_out_of_band_blocks() {
    let (app, _dir) = app().await;
    let res = app.htmx().get("/list").await;
    let body = res.text();
    assert!(body.starts_with(r#"<tr id="r1">"#), "{body}");
    assert!(body.contains(r#"<span id="count" hx-swap-oob="true">3</span>"#));
    assert!(!body.contains("<h1>"));
    app.get("/list").await.assert_see("<h1>List</h1>");
}

#[renox::test]
async fn live_validation_checks_one_field_without_the_handler() {
    let (app, _dir) = app().await;
    let res = app
        .request()
        .header("x-renox-validate", "email")
        .post("/signup", &[("name", ""), ("email", "nope")])
        .await;
    res.assert_ok();
    let body: renox::serde_json::Value = res.json();
    assert_eq!(body["field"], "email");
    assert!(body["errors"][0].as_str().unwrap().contains("email"));
    // A valid field: no errors, and still the handler didn't run.
    let res = app
        .request()
        .header("x-renox-validate", "name")
        .post("/signup", &[("name", "Ana"), ("email", "ana@test.id")])
        .await;
    let body: renox::serde_json::Value = res.json();
    assert_eq!(body["errors"], renox::serde_json::json!([]));
    app.get("/form").await.assert_dont_see("Welcome");
}

#[renox::test]
async fn kit_texts_follow_the_locale() {
    let dir = views();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(Auth::new()).module(Pages), move |c| {
        c.views_path = path;
        c.locale = "id".into();
    })
    .await;
    app.get("/form").await.assert_see("(opsional)");
    app.get("/components").await.assert_see("Batal|");
}

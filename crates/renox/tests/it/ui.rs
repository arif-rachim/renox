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

#[derive(serde::Deserialize)]
struct Profile {
    name: String,
    plan: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    news: Option<String>,
}

impl Validate for Profile {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required();
        v.field("plan", &self.plan).required();
        let _ = (&self.tags, &self.news);
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
            .post("/clear", || async {
                (Toast::success("Cleared"), HxRefresh)
            })
            .get("/list", || async {
                view("list.html", context! { count => 3 })
                    .fragment("rows")
                    .also("count")
            })
            .get("/fields", || async { view("fields.html", context! {}) })
            .post("/profile", |Valid(_): Valid<Profile>| async {
                Redirect::to("/fields")
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
        "fields.html",
        r#"{% from "renox/ui.html" import input, textarea, select, checkbox, radio, checkbox_list, form_grid, fieldset, form_errors %}
<form method="post" action="/profile">{{ csrf_field() }}{{ form_errors() }}
{% call form_grid(2) %}
{{ input("name", "Name", id="profile-name", required=true) }}
{{ input("price", "Price", type="number", prefix="Rp", suffix=".00", span=2) }}
{{ input("code", "Code", value="X1", readonly=true, datalist=["X1", ["X2", "Second"]]) }}
{{ textarea("bio", "Bio", id="profile-bio", disabled=true, span="full") }}
{{ select("size", "Size", ["s", "m"], id="profile-size", span=2) }}
{% endcall %}
{% call fieldset("Preferences", columns=2) %}
{{ radio("plan", "Plan", [["free", "Free"], ["pro", "Pro", "For teams"]], selected="free", required=true, inline=true) }}
{{ checkbox_list("tags", "Tags", [["a", "Alpha"], ["b", "Beta"], ["c", "Gamma"]], selected=["b"], columns=2) }}
{{ checkbox("news", "Email me news", checked=true) }}
{% endcall %}
</form>"#,
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
    // An htmx refresh reloads the page, which would lose a toast sent in
    // HX-Trigger: it waits in the session for that page instead.
    let res = app.htmx().post("/clear", &[]).await;
    res.assert_header("hx-refresh", "true");
    assert!(res.header("hx-trigger").is_none());
    app.get("/form").await.assert_see("Cleared");
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

#[renox::test]
async fn form_fields_choices_affixes_and_layout() {
    let (app, _dir) = app().await;
    let page = app.get("/fields").await;
    page.assert_ok()
        // Layout: a two-column grid, a field spanning both, a titled group.
        .assert_see(r#"<div class="rx-form-grid rx-cols-2">"#)
        .assert_see(r#"<div class="rx-field rx-span-2">"#)
        .assert_see(r#"<div class="rx-field rx-span-full">"#)
        .assert_see(r#"<legend class="rx-fieldset__legend">Preferences</legend>"#)
        // Every field takes its own id.
        .assert_see(r#"<label class="rx-label" for="profile-name">"#)
        .assert_see(r#"id="profile-bio""#)
        .assert_see(r#"id="profile-size""#)
        // Prefix and suffix are joined to the input and described by it.
        .assert_see(r#"<span class="rx-affix__text" id="rx-price-prefix">Rp</span>"#)
        .assert_see(r#"aria-describedby="rx-price-prefix rx-price-suffix rx-price-error""#)
        // Read-only, disabled, suggestions.
        .assert_see(r#"value="X1" list="rx-code-list" readonly"#)
        .assert_see(r#"<option value="X2">Second</option>"#)
        .assert_see(r#"id="profile-bio" name="bio" rows="4" disabled"#)
        // A radio group in a fieldset: the first option carries the group's id
        // (the error summary links to it), the others are numbered.
        .assert_see(r#"<legend class="rx-label">Plan</legend>"#)
        .assert_see(r#"<div class="rx-choices rx-choices--inline">"#)
        .assert_see(r#"id="rx-plan" name="plan" value="free" checked required"#)
        .assert_see(r#"id="rx-plan-2" name="plan" value="pro" required aria-describedby="rx-plan-2-detail""#)
        .assert_see(r#"<span class="rx-hint" id="rx-plan-2-detail">For teams</span>"#)
        // A checkbox list, ticked from `selected`.
        .assert_see(r#"<div class="rx-choices rx-cols-2">"#)
        .assert_see(r#"name="tags" value="a">"#)
        .assert_see(r#"name="tags" value="b" checked>"#)
        .assert_see(r#"id="rx-news" name="news" value="on" checked"#);

    // A failed submit refills what was sent: the other plan, two tags, and
    // the checkbox left unticked (which sends nothing) stays unticked.
    app.request()
        .header("referer", "/fields")
        .post(
            "/profile",
            &[("name", ""), ("plan", "pro"), ("tags", "a"), ("tags", "c")],
        )
        .await
        .assert_redirect("/fields");
    let page = app.get("/fields").await;
    page.assert_see(r#"name="plan" value="pro" checked"#)
        .assert_dont_see(r#"value="free" checked"#)
        .assert_see(r#"name="tags" value="a" checked>"#)
        .assert_dont_see(r#"name="tags" value="b" checked>"#)
        .assert_see(r#"name="tags" value="c" checked>"#)
        .assert_see(r#"id="rx-news" name="news" value="on" aria-describedby"#)
        // The summary links to the field by its name, whatever its id.
        .assert_see(r##"href="#rx-name" data-rx-field="name""##)
        .assert_see(r#"id="profile-name" name="name" type="text" value="" required aria-required="true" aria-invalid="true""#);

    // One tag only comes back as a string, not a list.
    app.request()
        .header("referer", "/fields")
        .post("/profile", &[("name", ""), ("tags", "b"), ("news", "on")])
        .await;
    let page = app.get("/fields").await;
    page.assert_see(r#"name="tags" value="b" checked>"#)
        .assert_dont_see(r#"name="tags" value="a" checked>"#)
        .assert_see(r#"id="rx-news" name="news" value="on" checked"#)
        .assert_see(r#"id="rx-plan" name="plan" value="free" required aria-invalid="true""#);
}

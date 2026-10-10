//! Template components at load time: unknown tags fail with `file:line`, plain pages are untouched.

use renox::prelude::*;
use renox::testing::TestApp;

struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/plain", || async { view("plain.html", context! {}) })
            .get("/stack", || async { view("stack.html", context! {}) })
            .get("/page", || async { view("page.html", context! {}) })
    }
}

async fn app() -> (TestApp, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| std::fs::write(dir.path().join(name), body).unwrap();
    write(
        "plain.html",
        "<h1 class=\"rx-title\">Plain {{ 1 + 1 }}</h1>",
    );
    write("stack.html", "<rx-stack>\n<p>in</p>\n</rx-stack>");
    write("page.html", "<h1>Page</h1>\n<rx-nope>x</rx-nope>\n");
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(Pages), move |c| c.views_path = path).await;
    (app, dir)
}

#[renox::test]
async fn an_unknown_component_names_file_and_line() {
    let (app, _dir) = app().await;
    let res = app.get("/page").await;
    res.assert_status(500);
    assert!(
        res.text()
            .contains("page.html:2: unknown component &lt;rx-nope&gt;"),
        "{}",
        res.text()
    );
}

#[renox::test]
async fn pages_without_components_render_as_before() {
    let (app, _dir) = app().await;
    app.get("/plain")
        .await
        .assert_ok()
        .assert_see("<h1 class=\"rx-title\">Plain 2</h1>");
}

#[renox::test]
async fn rx_stack_renders_a_div() {
    let (app, _dir) = app().await;
    app.get("/stack")
        .await
        .assert_ok()
        .assert_see("<div class=\"rx-stack\">\n<p>in</p>\n</div>");
}

/// Routes `/m` and `/r` render the two templates with the same data.
struct Pair(serde_json::Value);

impl Module for Pair {
    fn name(&self) -> &'static str {
        "pair"
    }

    fn routes(&self) -> Routes {
        let (a, b) = (self.0.clone(), self.0.clone());
        Routes::new()
            .get("/m", move || {
                let ctx = a.clone();
                async move { view("m.html", ctx) }
            })
            .get("/r", move || {
                let ctx = b.clone();
                async move { view("r.html", ctx) }
            })
    }
}

/// Collapses whitespace runs and drops whitespace between tags.
fn normalize(html: &str) -> String {
    let collapsed = html.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.replace("> <", "><")
}

/// A component page renders the same HTML as the macro calls it stands for.
async fn same(macro_src: &str, rx_src: &str, ctx: serde_json::Value) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("m.html"), macro_src).unwrap();
    std::fs::write(dir.path().join("r.html"), rx_src).unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(Pair(ctx)), move |c| c.views_path = path).await;
    let (m, r) = (app.get("/m").await, app.get("/r").await);
    m.assert_ok();
    r.assert_ok();
    let (m, r) = (normalize(&m.text()), normalize(&r.text()));
    assert!(!m.is_empty());
    assert_eq!(m, r);
}

const UI: &str = "{% import \"renox/ui.html\" as ui %}";

#[renox::test]
async fn rx_card_matches_the_macro() {
    same(
        &format!("{UI}{{% call ui.card(title=\"Contact\", subtitle=(sub)) %}}<p>body</p>{{% endcall %}}{{% call ui.card() %}}bare{{% endcall %}}"),
        "<rx-card title=\"Contact\" subtitle=\"{{ sub }}\"><p>body</p></rx-card><rx-card>bare</rx-card>",
        serde_json::json!({"sub": "Reach us"}),
    )
    .await;
}

#[renox::test]
async fn rx_toolbar_matches_the_macro() {
    same(
        &format!("{UI}{{% call ui.toolbar() %}}<a href=\"/x\">x</a>{{% endcall %}}"),
        "<rx-toolbar><a href=\"/x\">x</a></rx-toolbar>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_badge_matches_the_macro() {
    same(
        &format!(
            "{UI}{{{{ ui.badge(text=\"New\") }}}}{{{{ ui.badge(text=(label), kind=\"success\") }}}}"
        ),
        "<rx-badge text=\"New\"/><rx-badge text=\"{{ label }}\" kind=\"success\"/>",
        serde_json::json!({"label": "Paid"}),
    )
    .await;
}

#[renox::test]
async fn rx_badge_content_matches_the_prop() {
    same(
        &format!("{UI}{{{{ ui.badge(text=\"New\") }}}}"),
        "<rx-badge>New</rx-badge>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_page_header_matches_the_macro_with_and_without_actions() {
    same(
        &format!(
            "{UI}{{{{ ui.page_header(title=\"Orders\", subtitle=\"All\", back=\"/\", badge=\"3\", badge_kind=\"info\") }}}}\
             {{% call ui.page_header(title=(name)) %}}<a href=\"/new\">New</a>{{% endcall %}}"
        ),
        "<rx-page-header title=\"Orders\" subtitle=\"All\" back=\"/\" badge=\"3\" badge-kind=\"info\"> </rx-page-header>\
         <rx-page-header title=\"{{ name }}\"><a href=\"/new\">New</a></rx-page-header>",
        serde_json::json!({"name": "Items"}),
    )
    .await;
}

#[renox::test]
async fn control_flow_renders_lists_and_empty_lists() {
    let src =
        "<ul><li rx-for=\"x in xs\">{{ x }}</li></ul><p rx-if=\"xs\">some</p><p rx-else>none</p>";
    let mac = "<ul>{% for x in xs %}<li>{{ x }}</li>{% endfor %}</ul>{% if xs %}<p>some</p>{% else %}<p>none</p>{% endif %}";
    same(mac, src, serde_json::json!({"xs": ["a", "b"]})).await;
    same(mac, src, serde_json::json!({"xs": []})).await;
    let badge = "{% for x in xs %}{{ ui.badge(text=(x)) }}{% endfor %}";
    same(
        &format!("{UI}{badge}"),
        "<rx-badge rx-for=\"x in xs\" :text=\"x\"/>",
        serde_json::json!({"xs": ["a", "b"]}),
    )
    .await;
}

struct One;

impl Module for One {
    fn name(&self) -> &'static str {
        "one"
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/p", || async { view("p.html", context! {}) })
    }
}

/// Writes the files (paths relative to the views directory) and gets `/p`.
async fn render_files(files: &[(&str, &str)]) -> renox::testing::TestResponse {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in files {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(One), move |c| c.views_path = path).await;
    app.get("/p").await
}

const PRICE_TAG: (&str, &str) = (
    "components/price_tag.html",
    "<rx-props amount currency=\"USD\">\n<b>{{ amount }} {{ currency }}</b><rx-slot />",
);

#[renox::test]
async fn an_app_component_renders_with_defaults_and_content() {
    let res = render_files(&[
        PRICE_TAG,
        ("p.html", "<app-price-tag :amount=\"3\">!</app-price-tag>"),
    ])
    .await;
    res.assert_ok();
    assert_eq!(normalize(&res.text()), "<b>3 USD</b>!");
    let res = render_files(&[
        PRICE_TAG,
        ("p.html", "<app-price-tag :amount=\"3\" currency=\"EUR\"/>"),
    ])
    .await;
    assert_eq!(normalize(&res.text()), "<b>3 EUR</b>");
}

#[renox::test]
async fn an_app_component_with_a_missing_required_prop_fails() {
    let res = render_files(&[PRICE_TAG, ("p.html", "\n<app-price-tag/>")]).await;
    res.assert_status(500);
    assert!(
        res.text()
            .contains("p.html:2: &lt;app-price-tag&gt; needs the attribute &quot;amount&quot;"),
        "{}",
        res.text()
    );
}

#[renox::test]
async fn an_unknown_prop_of_an_app_component_gets_a_suggestion() {
    let res = render_files(&[
        PRICE_TAG,
        ("p.html", "<app-price-tag :amount=\"1\" curency=\"EUR\"/>"),
    ])
    .await;
    res.assert_status(500);
    assert!(
        res.text()
            .contains("has no attribute &quot;curency&quot;; did you mean &quot;currency&quot;?"),
        "{}",
        res.text()
    );
}

#[renox::test]
async fn an_app_component_takes_named_slots() {
    let res = render_files(&[
        (
            "components/panel.html",
            "<rx-props title=\"T\">\n<h2>{{ title }}</h2><rx-slot /><footer><rx-slot name=\"foot\" /></footer>",
        ),
        (
            "p.html",
            "<app-panel title=\"Hi\">body<rx-slot name=\"foot\">bye</rx-slot></app-panel>",
        ),
    ])
    .await;
    res.assert_ok();
    assert_eq!(
        normalize(&res.text()),
        "<h2>Hi</h2>body<footer>bye</footer>"
    );
}

#[renox::test]
async fn a_macro_file_used_as_a_component_says_what_to_do() {
    let res = render_files(&[
        ("components/legacy.html", "{% macro a() %}x{% endmacro %}"),
        ("p.html", "<app-legacy/>"),
    ])
    .await;
    res.assert_status(500);
    assert!(
        res.text()
            .contains("components/legacy.html has no &lt;rx-props&gt;: it holds macros"),
        "{}",
        res.text()
    );
    let res = render_files(&[("p.html", "<app-absent/>")]).await;
    assert!(
        res.text()
            .contains("&lt;app-absent&gt; needs resources/views/components/absent.html"),
        "{}",
        res.text()
    );
}

#[renox::test]
async fn app_components_can_use_kit_components() {
    let res = render_files(&[
        (
            "components/note.html",
            "<rx-props text>\n<rx-badge :text=\"text\"/>",
        ),
        ("p.html", "<app-note text=\"Hello\"/>"),
    ])
    .await;
    res.assert_ok();
    assert!(res.text().contains("Hello"), "{}", res.text());
}

#[renox::test]
async fn rx_page_and_rx_push_match_extends_and_push() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| std::fs::write(dir.path().join(name), body).unwrap();
    write(
        "layout.html",
        "<head>{% block seo %}<title>none</title>{% endblock %}</head><main>{% block content %}{% endblock %}</main>[{% block scripts %}{% endblock %}]{{ stack('js') }}",
    );
    write(
        "m.html",
        "{% extends \"layout.html\" %}{% block seo %}{{ seo(title=(name)) }}{% endblock %}{% block content %}<p>{{ name }}</p>\
         {% call push(\"js\", once=\"a\") %}<i>a</i>{% endcall %}{% call push(\"js\", once=\"a\") %}<i>a</i>{% endcall %}\
         {% endblock %}{% block scripts %}S{% endblock %}",
    );
    write(
        "r.html",
        "<rx-page layout=\"layout.html\" :title=\"name\">\n<p>{{ name }}</p>\n\
         <rx-push stack=\"js\" once=\"a\"><i>a</i></rx-push><rx-push stack=\"js\" once=\"a\"><i>a</i></rx-push>\n\
         <rx-slot name=\"scripts\">S</rx-slot></rx-page>",
    );
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(
        App::new().module(Pair(serde_json::json!({"name": "Items"}))),
        move |c| c.views_path = path,
    )
    .await;
    let (m, r) = (app.get("/m").await, app.get("/r").await);
    m.assert_ok();
    r.assert_ok();
    let (m, r) = (normalize(&m.text()), normalize(&r.text()));
    assert_eq!(m.replace("/m\"", "/r\""), r);
    assert!(r.contains("<title>Items"), "{r}");
    assert_eq!(r.matches("<i>a</i>").count(), 1, "{r}");
    assert!(r.contains("[S]"), "{r}");
}

#[renox::test]
async fn a_page_block_renders_as_a_fragment() {
    struct Frag;
    impl Module for Frag {
        fn name(&self) -> &'static str {
            "frag"
        }
        fn routes(&self) -> Routes {
            Routes::new().get("/f", || async {
                view("r.html", context! {}).fragment("content")
            })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("layout.html"),
        "<html>{% block content %}{% endblock %}</html>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-page layout=\"layout.html\"><p>inner</p></rx-page>",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(Frag), move |c| c.views_path = path).await;
    let res = app.htmx().get("/f").await;
    res.assert_ok().assert_see("<p>inner</p>");
    assert!(!res.text().contains("<html>"), "{}", res.text());
}

#[renox::test]
async fn rx_wizard_matches_the_macro() {
    same(
        &format!("{UI}{{% call ui.wizard(id=\"w\", steps=[[\"a\", \"First\"], [\"b\", (second)]], submit_label=\"Save\", cancel=true, back_label=\"Prev\") %}}{{% call ui.wizard_step(id=\"w\", key=\"a\", title=\"First\") %}}<p>one</p>{{% endcall %}}{{% call ui.wizard_step(id=\"w\", key=\"b\", title=(second)) %}}<p>two</p>{{% endcall %}}{{% endcall %}}"),
        "<rx-wizard id=\"w\" submit-label=\"Save\" cancel back-label=\"Prev\">\n<rx-wizard-step key=\"a\" title=\"First\"><p>one</p></rx-wizard-step>\n<rx-wizard-step key=\"b\" title=\"{{ second }}\"><p>two</p></rx-wizard-step>\n</rx-wizard>",
        serde_json::json!({"second": "Second"}),
    )
    .await;
}

#[renox::test]
async fn rx_wizard_holds_only_steps() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("m.html"),
        "<rx-wizard id=\"w\" submit-label=\"Save\"><p>x</p></rx-wizard>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-wizard-step key=\"a\">x</rx-wizard-step>",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(Pair(serde_json::json!({}))), move |c| {
        c.views_path = path
    })
    .await;
    let m = app.get("/m").await.text();
    assert!(m.contains("holds only &lt;rx-wizard-step&gt;"), "{m}");
    let r = app.get("/r").await.text();
    assert!(r.contains("belongs inside &lt;rx-wizard&gt;"), "{r}");
}

#[renox::test]
async fn rx_button_matches_the_macro_and_passes_hx_attributes() {
    same(
        &format!(
            "{UI}{{{{ ui.button(label=\"Save\", variant=\"danger\", size=\"small\", icon=\"plus\", attrs={{\"hx-post\": \"/x\"}}) }}}}"
        ),
        "<rx-button variant=\"danger\" size=\"small\" icon=\"plus\" hx-post=\"/x\">Save</rx-button>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_link_button_and_icon_button_match_the_macros() {
    same(
        &format!(
            "{UI}{{{{ ui.link_button(href=\"/a\", label=\"Go\", variant=\"primary\", new_tab=true) }}}}\
             {{{{ ui.icon_button(icon=\"plus\", label=\"Add\", href=\"/b\", variant=\"primary\") }}}}\
             {{{{ ui.icon_button(icon=\"plus\", label=\"Add\", attrs={{\"hx-get\": \"/c\"}}) }}}}"
        ),
        "<rx-link-button href=\"/a\" variant=\"primary\" new-tab>Go</rx-link-button>\
         <rx-icon-button icon=\"plus\" label=\"Add\" href=\"/b\" variant=\"primary\"/>\
         <rx-icon-button icon=\"plus\" label=\"Add\" hx-get=\"/c\"/>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_confirm_matches_the_macro() {
    same(
        &format!(
            "{UI}{{{{ ui.confirm(id=\"del\", label=\"Delete\", action=\"/x/1\", title=\"Sure?\", message=\"It goes.\", fields={{\"a\": \"b\"}}) }}}}"
        ),
        "<rx-confirm id=\"del\" label=\"Delete\" action=\"/x/1\" title=\"Sure?\" :fields=\"{'a': 'b'}\">It goes.</rx-confirm>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_alert_and_rx_empty_match_the_macros() {
    same(
        &format!(
            "{UI}{{{{ ui.alert(message=\"Heads up\", kind=\"warning\", title=\"Note\") }}}}\
             {{{{ ui.empty(title=\"Nothing\", message=\"Add one\", action_href=\"/new\", action_label=\"New\", icon=\"plus\") }}}}"
        ),
        "<rx-alert kind=\"warning\" title=\"Note\">Heads up</rx-alert>\
         <rx-empty title=\"Nothing\" action-href=\"/new\" action-label=\"New\" icon=\"plus\">Add one</rx-empty>",
        serde_json::json!({}),
    )
    .await;
}

#[derive(serde::Deserialize)]
struct Fields {
    name: String,
    bio: Option<String>,
    size: Option<String>,
    agree: Option<String>,
}

impl Validate for Fields {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(10);
        v.field("bio", &self.bio).max(5);
        v.field("size", &self.size).required();
        let _ = &self.agree;
    }
}

struct FormPages;

impl Module for FormPages {
    fn name(&self) -> &'static str {
        "form-pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/m", || async { view("m.html", context! {}) })
            .get("/r", || async { view("r.html", context! {}) })
            .post("/save", |Valid(_): Valid<Fields>| async { "ok" })
    }
}

#[renox::test]
async fn form_fields_match_the_macros_after_a_failed_submit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("m.html"),
        format!(
            "{UI}{{{{ ui.form_errors(title=\"Fix these\") }}}}\
             {{% call ui.form_grid(columns=3) %}}\
             {{{{ ui.input(name=\"name\", label=\"Name\", type=\"text\", hint=\"Short\", required=true, span=\"full\", attrs={{\"maxlength\": \"20\", \"data-x\": \"1\"}}) }}}}\
             {{{{ ui.textarea(name=\"bio\", label=\"Bio\", rows=2, value=\"hi\") }}}}\
             {{% endcall %}}\
             {{{{ ui.select(name=\"size\", label=\"Size\", options=[[\"s\", \"Small\"], [\"m\", \"Medium\"]], required=true) }}}}\
             {{{{ ui.checkbox(name=\"agree\", label=\"Agree\", switch=true) }}}}"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-form-errors title=\"Fix these\"/>\
         <rx-form-grid columns=\"3\">\
         <rx-input name=\"name\" label=\"Name\" type=\"text\" hint=\"Short\" required span=\"full\" maxlength=\"20\" data-x=\"1\"/>\
         <rx-textarea name=\"bio\" label=\"Bio\" rows=\"2\" value=\"hi\"/>\
         </rx-form-grid>\
         <rx-select name=\"size\" label=\"Size\" :options=\"[['s', 'Small'], ['m', 'Medium']]\" required/>\
         <rx-checkbox name=\"agree\" label=\"Agree\" switch/>",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(FormPages), move |c| c.views_path = path).await;
    let bad = [
        ("name", "Much too long a name"),
        ("bio", "far too long"),
        ("agree", "on"),
    ];
    let mut pages = Vec::new();
    for page in ["/m", "/r"] {
        app.request()
            .header("referer", page)
            .post("/save", &bad)
            .await
            .assert_redirect(page);
        let r = app.get(page).await;
        r.assert_ok();
        pages.push(normalize(&r.text()));
    }
    assert!(pages[0].contains("aria-invalid=\"true\""), "{}", pages[0]);
    assert!(pages[0].contains("value=\"Much too long a name\""));
    assert!(pages[0].contains("data-rx-error-summary"));
    assert_eq!(pages[0], pages[1]);
}

#[renox::test]
async fn rx_form_writes_the_csrf_and_method_fields() {
    let (put, get, post) = (
        "<rx-form action=\"/items/1\" method=\"PUT\" live class=\"box\" id=\"f\" :data-n=\"1 + 1\" hx-boost=\"true\">x</rx-form>",
        "<rx-form action=\"/search\" method=\"GET\"><input name=\"q\"></rx-form>",
        "<rx-form route=\"home\"></rx-form><rx-form></rx-form>",
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("r.html"), format!("{put}{get}{post}")).unwrap();
    std::fs::write(dir.path().join("m.html"), "").unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with_config(App::new().module(FormPages).module(HomeName), move |c| {
        c.views_path = path
    })
    .await;
    let html = app.get("/r").await.assert_ok().text();
    let forms: Vec<&str> = html.split("</form>").collect();
    let first = forms[0];
    assert!(first.starts_with(
        "<form method=\"post\" action=\"/items/1\" data-live-validate novalidate class=\"box\" id=\"f\" data-n=\"2\" hx-boost=\"true\">"
    ), "{first}");
    assert!(first.contains("name=\"_token\""), "{first}");
    assert!(first.contains("name=\"_method\" value=\"PUT\""), "{first}");
    let second = forms[1];
    assert!(
        second.starts_with("<form method=\"get\" action=\"/search\">"),
        "{second}"
    );
    assert!(
        !second.contains("_token") && !second.contains("_method"),
        "{second}"
    );
    assert!(
        forms[2].starts_with("<form method=\"post\" action=\"/home\">"),
        "{}",
        forms[2]
    );
    assert!(forms[2].contains("_token") && !forms[2].contains("_method"));
    assert!(
        forms[3].starts_with("<form method=\"post\">"),
        "{}",
        forms[3]
    );
}

struct HomeName;

impl Module for HomeName {
    fn name(&self) -> &'static str {
        "home-name"
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/home", || async { "home" }).name("home")
    }
}

#[renox::test]
async fn rx_form_rejects_a_bad_method() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-form method=\"PUTT\"></rx-form>",
    )
    .unwrap();
    std::fs::write(dir.path().join("m.html"), "").unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(FormPages), move |c| c.views_path = path).await;
    let r = app.get("/r").await.text();
    assert!(r.contains("must be one of"), "{r}");
}

#[derive(serde::Deserialize)]
struct Choices {
    plan: String,
    #[serde(default)]
    extras: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

impl Validate for Choices {
    fn rules(&self, v: &mut Validator) {
        v.field("plan", &self.plan)
            .one_of(&["free".to_string(), "pro".to_string()]);
        let _ = (&self.extras, &self.tags);
    }
}

struct ChoicePages;

impl Module for ChoicePages {
    fn name(&self) -> &'static str {
        "choice-pages"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/m", || async { view("m.html", context! {}) })
            .get("/r", || async { view("r.html", context! {}) })
            .post("/save", |Valid(_): Valid<Choices>| async { "ok" })
    }
}

#[renox::test]
async fn choice_fields_match_the_macros_and_refill_after_a_failed_submit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("m.html"),
        format!(
            "{UI}{{{{ ui.radio(name=\"plan\", label=\"Plan\", options=[[\"free\", \"Free\"], [\"pro\", \"Pro\", \"Teams\"]], hint=\"Pick\", required=true, inline=true, attrs={{\"data-x\": \"1\"}}) }}}}\
             {{{{ ui.checkbox_list(name=\"extras\", label=\"Extras\", options=[\"gift\", [\"note\", \"Card\"]], selected=[\"gift\"], columns=2) }}}}\
             {{{{ ui.tags_input(name=\"tags\", label=\"Tags\", value=[\"a\"], suggestions=[\"b\", \"c\"], placeholder=\"Add\") }}}}"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-radio name=\"plan\" label=\"Plan\" :options=\"[['free', 'Free'], ['pro', 'Pro', 'Teams']]\" hint=\"Pick\" required inline data-x=\"1\"/>\
         <rx-checkbox-list name=\"extras\" label=\"Extras\" :options=\"['gift', ['note', 'Card']]\" :selected=\"['gift']\" columns=\"2\"/>\
         <rx-tags-input name=\"tags\" label=\"Tags\" :value=\"['a']\" :suggestions=\"['b', 'c']\" placeholder=\"Add\"/>",
    )
    .unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(ChoicePages), move |c| c.views_path = path).await;
    let bad = [
        ("plan", "gold"),
        ("extras", "note"),
        ("tags", "x"),
        ("tags", "y"),
    ];
    let mut pages = Vec::new();
    for page in ["/m", "/r"] {
        app.request()
            .header("referer", page)
            .post("/save", &bad)
            .await
            .assert_redirect(page);
        let r = app.get(page).await;
        r.assert_ok();
        pages.push(normalize(&r.text()));
    }
    assert!(pages[0].contains("aria-invalid=\"true\""), "{}", pages[0]);
    assert!(pages[0].contains("value=\"x\""), "{}", pages[0]);
    assert_eq!(pages[0], pages[1]);
}

#[renox::test]
async fn rx_toggle_buttons_and_rx_file_match_the_macros() {
    same(
        &format!(
            "{UI}{{{{ ui.toggle_buttons(name=\"mode\", label=\"Mode\", options=[[\"a\", \"A\"], [\"b\", \"B\"]], selected=\"b\", hint=\"One\") }}}}\
             {{{{ ui.toggle_buttons(name=\"days\", label=\"Days\", options=[\"mon\", \"tue\"], selected=[\"tue\"], multiple=true) }}}}\
             {{{{ ui.file(name=\"doc\", label=\"Document\", accept=\"image/*\", multiple=true, current=\"/f/a.png\", current_name=\"a.png\", preview=true, required=true) }}}}"
        ),
        "<rx-toggle-buttons name=\"mode\" label=\"Mode\" :options=\"[['a', 'A'], ['b', 'B']]\" :selected=\"'b'\" hint=\"One\"/>\
         <rx-toggle-buttons name=\"days\" label=\"Days\" :options=\"['mon', 'tue']\" :selected=\"['tue']\" multiple/>\
         <rx-file name=\"doc\" label=\"Document\" accept=\"image/*\" multiple current=\"/f/a.png\" current-name=\"a.png\" preview required/>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_date_picker_matches_the_macro() {
    same(
        &format!(
            "{UI}{{{{ ui.date_picker(name=\"day\", label=\"Day\", value=\"2026-01-05\", min=\"2026-01-01\", max=\"2026-12-31\", hint=\"Pick\", required=true, readonly=true, disabled_dates=[\"2026-01-06\"], closed_weekdays=[0, 6]) }}}}"
        ),
        "<rx-date-picker name=\"day\" label=\"Day\" :value=\"'2026-01-05'\" min=\"2026-01-01\" max=\"2026-12-31\" hint=\"Pick\" required readonly :disabled-dates=\"['2026-01-06']\" :closed-weekdays=\"[0, 6]\"/>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_layout_fields_match_the_macros() {
    same(
        &format!(
            "{UI}{{% call ui.fieldset(legend=\"Shipping\", hint=\"Where to\", columns=2) %}}<p>a</p>{{% endcall %}}\
             {{% call ui.show_when(field=\"delivery\", values=\"courier\") %}}<p>b</p>{{% endcall %}}\
             {{% call ui.show_when(field=\"delivery\", values=[\"courier\", \"post\"]) %}}<p>c</p>{{% endcall %}}\
             {{% call ui.hide_when(field=\"pickup\", values=\"on\") %}}<p>d</p>{{% endcall %}}\
             {{{{ ui.key_value(name=\"headers\", label=\"Headers\", value=[[\"a\", \"1\"]], key_label=\"Name\", value_label=\"Val\", add_label=\"More\", hint=\"Sent\", span=\"full\") }}}}\
             {{{{ ui.key_value(name=\"plain\", label=\"Plain\") }}}}"
        ),
        "<rx-fieldset legend=\"Shipping\" hint=\"Where to\" columns=\"2\"><p>a</p></rx-fieldset>\
         <rx-show-when field=\"delivery\" values=\"courier\"><p>b</p></rx-show-when>\
         <rx-show-when field=\"delivery\" :values=\"['courier', 'post']\"><p>c</p></rx-show-when>\
         <rx-hide-when field=\"pickup\" values=\"on\"><p>d</p></rx-hide-when>\
         <rx-key-value name=\"headers\" label=\"Headers\" :value=\"[['a', '1']]\" key-label=\"Name\" value-label=\"Val\" add-label=\"More\" hint=\"Sent\" span=\"full\"/>\
         <rx-key-value name=\"plain\" label=\"Plain\"/>",
        serde_json::json!({}),
    )
    .await;
}

#[renox::test]
async fn rx_select_options_url_and_editable_match_the_macro() {
    same(
        &format!(
            "{UI}{{{{ ui.select(name=\"cat\", label=\"Category\", options=[], options_url=\"/options/cats\", editable=true, searchable=true) }}}}\
             {{{{ ui.select(name=\"tags\", label=\"Tags\", options=[[\"a\", \"A\"]], selected=[\"a\"], multiple=true, hide_label=true) }}}}"
        ),
        "<rx-select name=\"cat\" label=\"Category\" :options=\"[]\" options-url=\"/options/cats\" editable searchable/>\
         <rx-select name=\"tags\" label=\"Tags\" :options=\"[['a', 'A']]\" :selected=\"['a']\" multiple hide-label/>",
        serde_json::json!({}),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("r.html"),
        "<rx-select name=\"cat\" label=\"Category\" :options=\"[]\" options-url=\"/options/cats\" editable/>",
    )
    .unwrap();
    std::fs::write(dir.path().join("m.html"), "").unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(FormPages), move |c| c.views_path = path).await;
    let html = app.get("/r").await.assert_ok().text();
    assert!(
        html.contains("data-rx-options-url=\"/options/cats\""),
        "{html}"
    );
    assert!(html.contains("data-rx-editable"), "{html}");
}

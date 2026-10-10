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

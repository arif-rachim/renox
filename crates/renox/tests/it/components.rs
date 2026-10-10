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

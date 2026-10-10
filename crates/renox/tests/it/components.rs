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

/// Serves `m.html` / `r.html` with `ctx`, plus a named route with a parameter.
struct Tables(serde_json::Value);

impl Module for Tables {
    fn name(&self) -> &'static str {
        "tables"
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
            .get("/items/{id}", || async { "item" })
            .name("items.show")
    }
}

async fn tables(rx_src: &str, ctx: serde_json::Value) -> (TestApp, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("m.html"), "").unwrap();
    std::fs::write(dir.path().join("r.html"), rx_src).unwrap();
    let path = dir.path().to_path_buf();
    let app =
        TestApp::with_config(App::new().module(Tables(ctx)), move |c| c.views_path = path).await;
    (app, dir)
}

#[renox::test]
async fn rx_table_over_a_list_matches_the_macro() {
    let ctx = serde_json::json!({"items": [
        {"id": 1, "name": "Ann", "qty": 3},
        {"id": 2, "name": "Bob", "qty": 4},
    ]});
    same(
        &format!(
            "{UI}<div class=\"rx-stack\" id=\"rx-table-1\">{{% if items %}}{{% call ui.table(head=[\"Name\", [\"Qty\", \"num\"], [\"Note\", \"hide-narrow\"], [\"Both\", \"num rx-hide-narrow\"], [\"\", \"num\"]], caption=\"People\") %}}\
             {{% for p in items %}}<tr><td>{{{{ p.name }}}}</td><td class=\"rx-num\">{{{{ p.qty }}}}</td>\
             <td class=\"rx-hide-narrow\">n</td><td class=\"rx-num rx-hide-narrow\">b</td>\
             <td class=\"rx-num\">{{% call ui.row_actions() %}}<a href=\"/x\">x</a>{{% endcall %}}</td></tr>{{% endfor %}}{{% endcall %}}{{% endif %}}</div>"
        ),
        "<rx-table :rows=\"items\" as=\"p\" caption=\"People\">\n\
         <rx-column label=\"Name\">{{ p.name }}</rx-column>\n\
         <rx-column label=\"Qty\" align=\"num\">{{ p.qty }}</rx-column>\n\
         <rx-column label=\"Note\" hide-narrow>n</rx-column>\n\
         <rx-column label=\"Both\" align=\"num\" hide-narrow>b</rx-column>\n\
         <rx-row-actions><a href=\"/x\">x</a></rx-row-actions>\n\
         </rx-table>",
        ctx,
    )
    .await;
}

#[renox::test]
async fn rx_table_pages_get_links_and_the_empty_slot_shows() {
    let items: Vec<serde_json::Value> = (11..=20).map(|i| serde_json::json!({"id": i})).collect();
    let page = Paginated::new(items, 2, 10, 25);
    let src = "<rx-table :rows=\"rows\" id=\"things\" card>\
               <rx-column label=\"Id\">{{ row.id }}</rx-column>\
               <rx-slot name=\"empty\"><p>Nothing yet</p></rx-slot></rx-table>";
    let (app, _dir) = tables(
        src,
        serde_json::json!({"rows": serde_json::to_value(&page).unwrap()}),
    )
    .await;
    let r = app.get("/r").await;
    r.assert_ok();
    let t = r.text();
    assert!(t.contains("id=\"things\""), "{t}");
    assert!(
        t.contains("<div class=\"rx-card\"><div class=\"rx-table-wrap\">"),
        "{t}"
    );
    assert!(t.contains("hx-target=\"#things\""), "{t}");
    assert!(t.contains("hx-select=\"#things\""), "{t}");
    assert!(t.contains("class=\"pagination\""), "{t}");
    assert!(t.contains("<td>20</td>"), "{t}");
    assert!(!t.contains("Nothing yet"), "{t}");

    let (app, _dir) = tables(src, serde_json::json!({"rows": []})).await;
    let t = app.get("/r").await.text();
    assert!(t.contains("<p>Nothing yet</p>"), "{t}");
    assert!(!t.contains("<table"), "{t}");
}

#[renox::test]
async fn rx_table_row_routes_and_abilities() {
    let ctx = serde_json::json!({"rows": [
        {"id": 7, "_can": {"edit": true}},
        {"id": 8, "_can": {"edit": false}},
    ]});
    let (app, _dir) = tables(
        "<rx-table :rows=\"rows\"><rx-column label=\"Id\">{{ row.id }}</rx-column>\
         <rx-row-actions><rx-link-button route=\"items.show\" can=\"edit\">Edit</rx-link-button></rx-row-actions></rx-table>",
        ctx,
    )
    .await;
    let t = app.get("/r").await.text();
    assert!(t.contains("href=\"/items/7\""), "{t}");
    assert!(!t.contains("/items/8"), "{t}");
}

#[renox::test]
async fn rx_table_holds_columns_only() {
    let (app, _dir) = tables(
        "<rx-table :rows=\"[]\"><p>no</p></rx-table>",
        serde_json::json!({}),
    )
    .await;
    let t = app.get("/r").await.text();
    assert!(
        t.contains("&lt;rx-table&gt; holds &lt;rx-column&gt;, &lt;rx-row-actions&gt; and &lt;rx-slot name=&quot;empty&quot;&gt;"),
        "{t}"
    );
    let (app, _dir) = tables(
        "<rx-column label=\"x\">y</rx-column>",
        serde_json::json!({}),
    )
    .await;
    let t = app.get("/r").await.text();
    assert!(t.contains("belongs inside &lt;rx-table&gt;"), "{t}");
}

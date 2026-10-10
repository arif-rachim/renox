//! Template components: `<rx-…>` and `<app-…>` tags compiled to plain MiniJinja when a template
//! loads. This step scans, builds the tree and reports unknown components; code generation comes
//! in later steps, so a template without components is returned as it is.

use std::sync::LazyLock;

mod app;
#[allow(dead_code)]
mod attrs;
mod contracts;
mod custom_data;
mod emit;
mod plugin;
mod scan;
mod special;
pub(crate) mod suggest;
mod tree;

pub(crate) use contracts::{BUILTIN, Contract};
pub use plugin::{Component, Prop};

/// The editor data file for `contracts` and the app's component files (`(file name, source)`
/// pairs). A file whose `<rx-props>` can't be read is left out.
pub(crate) fn custom_data_for(
    contracts: &[Contract],
    files: &[(String, String)],
) -> serde_json::Value {
    let app: Vec<(String, app::AppContract)> = files
        .iter()
        .filter_map(|(file, src)| {
            let stem = file.strip_prefix("components/")?.strip_suffix(".html")?;
            let tag = format!("app-{}", stem.replace('_', "-"));
            Some((tag, app::contract_of(file, src).ok()?))
        })
        .collect();
    custom_data::custom_data(contracts, &app)
}

/// A mistake found while compiling a template, with the line it is on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompileError {
    /// The line of the template source.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

/// The components the compiler knows, and a way to read other templates.
pub(crate) struct Catalog<'a> {
    /// The known components.
    pub contracts: &'a [Contract],
    /// Reads a template's source by name.
    #[allow(dead_code)]
    pub lookup: &'a dyn Fn(&str) -> Option<String>,
}

/// Templates that need no compiling: no component tag and no `rx-if` / `rx-else` / `rx-for`.
static NEEDS_COMPILE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"</?(?:rx|app)-|\srx-(?:if|else|for)\b").expect("a valid pattern")
});

/// Compiles the template `file` (its source is `src`) to plain MiniJinja.
pub(crate) fn compile(_file: &str, src: &str, catalog: &Catalog) -> Result<String, CompileError> {
    if !NEEDS_COMPILE.is_match(src) {
        return Ok(src.to_owned());
    }
    if let Some((header, body)) = app::component_file(src)? {
        let inner = compile(_file, &body, catalog)?;
        return Ok(format!("{header}{inner}{{% endmacro %}}"));
    }
    let nodes = tree::build(scan::scan(src)?)?;
    emit::emit(src, &nodes, catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog<'static> {
        Catalog {
            contracts: &[],
            lookup: &|_| None,
        }
    }

    #[test]
    fn builtins_compile_to_themselves() {
        for (name, source) in crate::view::BUILTIN {
            assert_eq!(
                compile(name, source, &catalog()).unwrap(),
                *source,
                "{name}"
            );
        }
    }

    #[test]
    fn plain_templates_pass_through() {
        let src = "<div class=\"rx-card\">{{ x }}</div> <a rx-ifx=\"1\">";
        assert_eq!(compile("a.html", src, &catalog()).unwrap(), src);
    }

    #[test]
    fn unknown_components_name_the_line() {
        let e = compile(
            "bad.html",
            "a\nb\n<rx-tabel :rows=\"x\">\n</rx-tabel>",
            &catalog(),
        )
        .unwrap_err();
        assert_eq!(e.line, 3);
        assert_eq!(e.message, "unknown component <rx-tabel>");
    }

    #[test]
    fn tree_errors_come_through() {
        let e = compile("bad.html", "<app-x>", &catalog()).unwrap_err();
        assert_eq!(e.message, "<app-x> opened here is never closed");
    }

    fn builtin() -> Catalog<'static> {
        Catalog {
            contracts: BUILTIN,
            lookup: &|_| None,
        }
    }

    #[test]
    fn element_components_keep_attributes_and_merge_classes() {
        let out = compile(
            "a.html",
            "<rx-row end data-bs-reveal class=\"x\" :title=\"t\">\n<p>a</p></rx-row>",
            &builtin(),
        )
        .unwrap();
        assert_eq!(
            out,
            "<div class=\"rx-row rx-row--end x\" data-bs-reveal title=\"{{ t }}\">\n<p>a</p></div>"
        );
        let out = compile(
            "a.html",
            "<rx-row end data-bs-reveal class=\"x\">\n<p>a</p></rx-row>",
            &builtin(),
        )
        .unwrap();
        assert_eq!(
            out,
            "<div class=\"rx-row rx-row--end x\" data-bs-reveal>\n<p>a</p></div>"
        );
        assert_eq!(
            compile("a.html", "<rx-stack />", &builtin()).unwrap(),
            "<div class=\"rx-stack\"></div>"
        );
    }

    #[test]
    fn macro_components_call_the_kit() {
        let out = compile(
            "a.html",
            "<rx-card title=\"Contact\" subtitle=\"{{ t('a') }}\">body</rx-card>",
            &builtin(),
        )
        .unwrap();
        assert_eq!(
            out,
            "{% import \"renox/ui.html\" as __rx_ui %}{% call __rx_ui.card(title=\"Contact\", subtitle=(t('a'))) %}body{% endcall %}"
        );
        let out = compile(
            "a.html",
            "<rx-page-header title=\"T\" badge-kind=\"info\"> </rx-page-header><rx-badge text=\"x\"/><rx-badge text=\"y\"/>",
            &builtin(),
        )
        .unwrap();
        assert_eq!(
            out,
            "{% import \"renox/ui.html\" as __rx_ui %}{{ __rx_ui.page_header(title=\"T\", badge_kind=\"info\") }}{{ __rx_ui.badge(text=\"x\") }}{{ __rx_ui.badge(text=\"y\") }}"
        );
        let msg = |s: &str| compile("a.html", s, &builtin()).unwrap_err().message;
        assert_eq!(
            msg("<rx-badge lable=\"x\"/>"),
            "<rx-badge> has no attribute \"lable\". It takes: text, kind"
        );
        assert_eq!(
            msg("<rx-badge/>"),
            "<rx-badge> needs the attribute \"text\""
        );
    }

    #[test]
    fn slots_fill_props_and_pass_arguments() {
        use contracts::{Module, Render, Slot};
        static T: &[Contract] = &[Contract {
            tag: "rx-t",
            doc: "t",
            render: Render::Macro {
                module: Module::Ui,
                name: "t",
            },
            props: &[],
            slots: &[
                Slot {
                    name: "",
                    into: None,
                    args: &["row", "prefix"],
                    optional: false,
                    doc: "x",
                },
                Slot {
                    name: "footer",
                    into: None,
                    args: &[],
                    optional: true,
                    doc: "x",
                },
            ],
            events: &[],
            route_prop: None,
            attrs: false,
            parent: None,
        }];
        let all: Vec<Contract> = BUILTIN.iter().chain(T).copied().collect();
        let c = Catalog {
            contracts: &all,
            lookup: &|_| None,
        };
        let imp = "{% import \"renox/ui.html\" as __rx_ui %}";
        let ok = |s: &str| compile("a.html", s, &c).unwrap();
        let msg = |s: &str| compile("a.html", s, &c).unwrap_err().message;
        assert_eq!(
            ok("<rx-badge kind=\"info\">New {{ n }}</rx-badge>"),
            format!(
                "{imp}{{% set __rx_slot_1 %}}New {{{{ n }}}}{{% endset %}}{{{{ __rx_ui.badge(kind=\"info\", text=__rx_slot_1) }}}}"
            )
        );
        assert_eq!(
            ok("<rx-t>{{ row }}<rx-slot name=\"footer\">F</rx-slot></rx-t>"),
            format!(
                "{imp}{{% set __rx_slot_1 %}}F{{% endset %}}{{% call(row, prefix) __rx_ui.t(footer=__rx_slot_1) %}}{{{{ row }}}}{{% endcall %}}"
            )
        );
        assert_eq!(
            msg("<rx-t><rx-slot name=\"fotter\">F</rx-slot></rx-t>"),
            "<rx-t> has no slot \"fotter\"; did you mean \"footer\"?"
        );
        assert_eq!(
            msg("<rx-badge text=\"a\">b</rx-badge>"),
            "<rx-badge> give \"text\" or content, not both"
        );
        assert_eq!(
            msg("<rx-badge><rx-slot>b</rx-slot></rx-badge>"),
            "<rx-slot> needs a name here"
        );
        assert_eq!(
            msg("<rx-badge/>"),
            "<rx-badge> needs the attribute \"text\""
        );
    }

    #[test]
    fn typos_get_a_suggestion() {
        let e = compile("a.html", "<rx-stak></rx-stak>", &builtin()).unwrap_err();
        assert_eq!(
            e.message,
            "unknown component <rx-stak>; did you mean <rx-stack>?"
        );
    }

    #[test]
    fn padding_keeps_the_line_count() {
        let src = "<rx-row\n  end\n  class=\"x\">\na</rx-row>";
        let out = compile("a.html", src, &builtin()).unwrap();
        assert_eq!(src.lines().count(), out.lines().count());
        assert_eq!(emit::pad("a\nb\n"), "{#\n\n#}");
        assert_eq!(emit::pad("ab"), "");
    }

    #[test]
    fn control_flow_wraps_elements_and_components() {
        let c = |s: &str| compile("a.html", s, &builtin()).unwrap();
        assert_eq!(
            c("<tr class=\"a\" rx-for=\"s in xs\">x</tr>"),
            "{% for s in xs %}<tr class=\"a\">x</tr>{% endfor %}"
        );
        assert_eq!(
            c("<p rx-if=\"a\">A</p>\n<p rx-else>B</p>"),
            "{% if a %}<p>A</p>\n{% else %}<p>B</p>{% endif %}"
        );
        assert_eq!(
            c("<rx-badge can=\"update\" text=\"x\"/>"),
            "{% import \"renox/ui.html\" as __rx_ui %}{% if can(\"update\") %}{{ __rx_ui.badge(text=\"x\") }}{% endif %}"
        );
        assert_eq!(
            c("<rx-badge rx-for=\"s in xs\" rx-if=\"s.ok\" can=\"a\" text=\"x\"/>"),
            "{% import \"renox/ui.html\" as __rx_ui %}{% for s in xs %}{% if s.ok and can(\"a\") %}{{ __rx_ui.badge(text=\"x\") }}{% endif %}{% endfor %}"
        );
    }

    #[test]
    fn control_flow_keeps_the_line_count() {
        let src = "<p\n  class=\"a\"\n  rx-if=\"a\"\n>A</p>\n<!-- c -->\n<p rx-else>B</p>\n<tr\n rx-for=\"s in xs\">x</tr>";
        let out = compile("a.html", src, &builtin()).unwrap();
        assert_eq!(src.lines().count(), out.lines().count(), "{out}");
    }

    #[test]
    fn control_flow_errors() {
        let msg = |s: &str| compile("a.html", s, &builtin()).unwrap_err().message;
        assert_eq!(
            msg("<p rx-else>B</p>"),
            "rx-else must follow an element with rx-if"
        );
        assert_eq!(
            msg("<p rx-for=\"a in b\" rx-if=\"a\">A</p><p rx-else>B</p>"),
            "rx-else can't follow an element with rx-for"
        );
        assert_eq!(msg("<p rx-if>A</p>"), "rx-if needs a condition");
        assert_eq!(
            msg("<p rx-for=\"xs\">A</p>"),
            "rx-for needs \"item in list\""
        );
    }

    #[test]
    fn contract_errors_use_the_fixed_texts() {
        use contracts::{Prop, Render};
        const PROPS: &[Prop] = &[
            Prop {
                name: "label",
                kind: attrs::Kind::Text,
                required: true,
                values: &[],
                doc: "x",
            },
            Prop {
                name: "size",
                kind: attrs::Kind::Text,
                required: false,
                values: &[],
                doc: "x",
            },
        ];
        static T: &[Contract] = &[Contract {
            tag: "rx-t",
            doc: "t",
            render: Render::Element {
                tag: "p",
                class: "t",
            },
            props: PROPS,
            slots: &[],
            events: &[],
            route_prop: None,
            attrs: false,
            parent: Some("rx-stack"),
        }];
        let all: Vec<Contract> = BUILTIN.iter().chain(T).copied().collect();
        let c = Catalog {
            contracts: &all,
            lookup: &|_| None,
        };
        let msg = |s: &str| compile("a.html", s, &c).unwrap_err().message;
        assert_eq!(
            msg("<rx-stack><rx-t label=\"a\" sise=\"b\"/></rx-stack>"),
            "<rx-t> has no attribute \"sise\"; did you mean \"size\"? It takes: label, size"
        );
        assert_eq!(
            msg("<rx-stack><rx-t/></rx-stack>"),
            "<rx-t> needs the attribute \"label\""
        );
        assert_eq!(
            msg("<rx-stack><rx-t label=\"a\">x</rx-t></rx-stack>"),
            "<rx-t> takes no content"
        );
        assert_eq!(
            msg("<rx-t label=\"a\"/>"),
            "<rx-t> belongs inside <rx-stack>"
        );
    }

    #[test]
    fn pages_extend_a_layout_and_move_slots() {
        let c = |s: &str| compile("a.html", s, &builtin()).unwrap();
        let msg = |s: &str| compile("a.html", s, &builtin()).unwrap_err().message;
        assert_eq!(
            c(
                "<rx-page layout=\"layouts/staff.html\" title=\"{{ t('x') }}\">\n<p>a</p>\n</rx-page>"
            ),
            "{% extends \"layouts/staff.html\" %}{% block seo %}{{ seo(title=(t('x'))) }}{% endblock %}{% block content %}\n<p>a</p>\n{% endblock %}"
        );
        assert_eq!(
            c(
                "{# c #}\n<rx-page layout=\"l.html\" title=\"T\" description=\"D\">x<rx-slot name=\"scripts\">s</rx-slot></rx-page>\n"
            ),
            "{# c #}\n{% extends \"l.html\" %}{% block seo %}{{ seo(title=\"T\", description=\"D\") }}{% endblock %}{% block content %}x{% endblock %}{% block scripts %}s{% endblock %}\n"
        );
        assert_eq!(
            c("<rx-page layout=\"l.html\"><rx-slot name=\"seo\">S</rx-slot>x</rx-page>"),
            "{% extends \"l.html\" %}{% block content %}x{% endblock %}{% block seo %}S{% endblock %}"
        );
        assert_eq!(
            msg("<rx-page layout=\"{{ l }}\">x</rx-page>"),
            "<rx-page> \"layout\" must be a file name, not {{ }}"
        );
        assert_eq!(
            msg("<p>a</p><rx-page layout=\"l.html\">x</rx-page>"),
            "<rx-page> must hold the whole template"
        );
        assert_eq!(
            msg("<rx-stack><rx-page layout=\"l.html\">x</rx-page></rx-stack>"),
            "<rx-page> must hold the whole template"
        );
        assert_eq!(
            msg(
                "<rx-page layout=\"l.html\" title=\"T\"><rx-slot name=\"seo\">S</rx-slot></rx-page>"
            ),
            "give title or a seo slot, not both"
        );
        assert_eq!(
            msg("<rx-page title=\"T\">x</rx-page>"),
            "<rx-page> needs the attribute \"layout\""
        );
    }

    #[test]
    fn push_calls_the_stack_function() {
        let c = |s: &str| compile("a.html", s, &builtin()).unwrap();
        assert_eq!(
            c("<rx-push stack=\"scripts\">\n<script></script>\n</rx-push>"),
            "{% call push(\"scripts\") %}\n<script></script>\n{% endcall %}"
        );
        assert_eq!(
            c("<rx-push stack=\"scripts\" once=\"chart\">x</rx-push>"),
            "{% call push(\"scripts\", once=\"chart\") %}x{% endcall %}"
        );
    }

    #[test]
    fn actions_use_route_and_pass_attributes_through() {
        let out = compile(
            "a.html",
            "<rx-link-button route=\"stock.suppliers.create\" variant=\"primary\" icon=\"plus\">{{ t('n') }}</rx-link-button>",
            &builtin(),
        )
        .unwrap();
        assert_eq!(
            out,
            "{% import \"renox/ui.html\" as __rx_ui %}{% set __rx_slot_1 %}{{ t('n') }}{% endset %}{{ __rx_ui.link_button(href=route(\"stock.suppliers.create\"), variant=\"primary\", icon=\"plus\", label=__rx_slot_1) }}"
        );
        let out = compile(
            "a.html",
            "<rx-button hx-post=\"/x\" hx-vals=\"{{ v }}\" @click=\"go\" autofocus>Save</rx-button>",
            &builtin(),
        )
        .unwrap();
        assert!(
            out.ends_with(
                "label=__rx_slot_1, attrs={\"hx-post\": \"/x\", \"hx-vals\": (v), \"@click\": \"go\", \"autofocus\": true}) }}"
            ),
            "{out}"
        );
    }

    #[test]
    fn other_attributes_go_to_attrs() {
        let out = compile(
            "a.html",
            "<rx-button hx-target=\"#x\" class=\"a\" @click=\"go\" id=\"i\" data-n=\"{{ n }}\">x</rx-button>",
            &builtin(),
        )
        .unwrap();
        assert!(
            out.contains(
                "attrs={\"hx-target\": \"#x\", \"class\": \"a\", \"@click\": \"go\", \"id\": \"i\", \"data-n\": "
            ),
            "{out}"
        );
    }

    #[test]
    fn route_and_class_errors() {
        let msg = |s: &str| compile("a.html", s, &builtin()).unwrap_err().message;
        assert_eq!(
            msg("<rx-card route=\"a\">x</rx-card>"),
            "<rx-card> has no route shortcut"
        );
        assert_eq!(
            msg("<rx-link-button route=\"a\" href=\"/b\">x</rx-link-button>"),
            "<rx-link-button> takes route or \"href\", not both"
        );
        assert!(msg("<rx-button :foo=\"x\">x</rx-button>").contains("has no attribute \"foo\""),);
        assert_eq!(
            msg("<rx-link-button>x</rx-link-button>"),
            "<rx-link-button> needs the attribute \"href\""
        );
    }
}

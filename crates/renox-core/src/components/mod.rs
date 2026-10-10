//! Template components: `<rx-…>` and `<app-…>` tags compiled to plain MiniJinja when a template
//! loads. This step scans, builds the tree and reports unknown components; code generation comes
//! in later steps, so a template without components is returned as it is.

use std::sync::LazyLock;

#[allow(dead_code)]
mod attrs;
mod contracts;
mod emit;
mod scan;
mod suggest;
mod tree;

pub(crate) use contracts::{BUILTIN, Contract};

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
}

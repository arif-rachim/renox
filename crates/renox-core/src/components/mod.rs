//! Template components: `<rx-…>` and `<app-…>` tags compiled to plain MiniJinja when a template
//! loads. This step scans, builds the tree and reports unknown components; code generation comes
//! in later steps, so a template without components is returned as it is.

use std::sync::LazyLock;

mod scan;
mod suggest;
mod tree;

use tree::Node;

/// A mistake found while compiling a template, with the line it is on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompileError {
    /// The line of the template source.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

/// What a component promises. Empty until the contract table arrives.
#[derive(Debug)]
pub(crate) struct Contract {}

/// The components the compiler knows, and a way to read other templates.
pub(crate) struct Catalog<'a> {
    /// The known components.
    pub contracts: &'a [Contract],
    /// Reads a template's source by name.
    pub lookup: &'a dyn Fn(&str) -> Option<String>,
}

/// Templates that need no compiling: no component tag and no `rx-if` / `rx-else` / `rx-for`.
static NEEDS_COMPILE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"</?(?:rx|app)-|\srx-(?:if|else|for)\b").expect("a valid pattern")
});

/// Compiles the template `file` (its source is `src`) to plain MiniJinja.
pub(crate) fn compile(file: &str, src: &str, catalog: &Catalog) -> Result<String, CompileError> {
    let _ = file;
    if !NEEDS_COMPILE.is_match(src) {
        return Ok(src.to_owned());
    }
    let nodes = tree::build(scan::scan(src)?)?;
    check(&nodes, catalog)?;
    Ok(src.to_owned())
}

/// Reports the first component nothing defines.
fn check(nodes: &[Node<'_>], catalog: &Catalog) -> Result<(), CompileError> {
    let _ = catalog.lookup;
    for node in nodes {
        if let Node::Element {
            name,
            children,
            line,
            ..
        } = node
        {
            if tree::is_component(name) {
                // Contracts carry no tag name yet, so there is nothing to suggest.
                let known: Vec<&str> = Vec::new();
                let hint = suggest::did_you_mean(name, known)
                    .map(|x| format!("; did you mean <{x}>?"))
                    .unwrap_or_default();
                return Err(CompileError {
                    line: *line,
                    message: format!("unknown component <{name}>{hint}"),
                });
            }
            check(children, catalog)?;
        }
    }
    let _ = catalog.contracts;
    Ok(())
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
}

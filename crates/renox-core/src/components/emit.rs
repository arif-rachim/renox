//! Code generation: the tree back to template source, components replaced by plain MiniJinja or
//! HTML. Generated code never holds a line break; a replaced span that had some keeps them inside
//! a `{# … #}` comment, so the lines of the source stay where they were (Decision 8 of #372).

use super::attrs::{self, Kind};
use super::contracts::{Contract, Render};
use super::scan::Attr;
use super::tree::{self, Node};
use super::{Catalog, CompileError, suggest};

/// A comment holding as many line breaks as `span_text` has, or nothing.
pub(super) fn pad(span_text: &str) -> String {
    let k = span_text.matches('\n').count();
    if k == 0 {
        String::new()
    } else {
        format!("{{#{}#}}", "\n".repeat(k))
    }
}

fn bare(name: &str) -> &str {
    name.strip_prefix(':').unwrap_or(name)
}

/// Checks the attributes against the contract.
pub(super) fn check_attrs(contract: &Contract, attrs: &[Attr<'_>]) -> Result<(), String> {
    let tag = contract.tag;
    for a in attrs {
        let n = bare(a.name);
        if n == "class" && contract.attrs {
            continue;
        }
        if contract.props.iter().any(|p| p.name == n) || contract.attrs {
            continue;
        }
        let names: Vec<&str> = contract.props.iter().map(|p| p.name).collect();
        let hint = suggest::did_you_mean(n, names.iter().copied())
            .map(|x| format!("; did you mean \"{x}\"?"))
            .unwrap_or_else(|| ".".to_owned());
        let takes = if names.is_empty() {
            "It takes no attributes".to_owned()
        } else {
            format!("It takes: {}", names.join(", "))
        };
        return Err(format!("<{tag}> has no attribute \"{n}\"{hint} {takes}"));
    }
    for p in contract.props.iter().filter(|p| p.required) {
        if !attrs.iter().any(|a| bare(a.name) == p.name) {
            return Err(format!("<{tag}> needs the attribute \"{}\"", p.name));
        }
    }
    Ok(())
}

fn err(line: usize, message: String) -> CompileError {
    CompileError { line, message }
}

/// The tree as template source.
pub(crate) fn emit(
    src: &str,
    nodes: &[Node<'_>],
    catalog: &Catalog,
) -> Result<String, CompileError> {
    let mut out = String::with_capacity(src.len());
    emit_into(src, nodes, catalog, None, &mut out)?;
    Ok(out)
}

fn emit_into(
    src: &str,
    nodes: &[Node<'_>],
    catalog: &Catalog,
    parent: Option<&str>,
    out: &mut String,
) -> Result<(), CompileError> {
    for node in nodes {
        match node {
            Node::Text(span) => out.push_str(&src[span.clone()]),
            Node::Element {
                name,
                attrs,
                children,
                open,
                close,
                line,
            } => {
                if !tree::is_component(name) {
                    // Control-flow elements come in a later step: copied as they are.
                    out.push_str(&src[open.clone()]);
                    emit_into(src, children, catalog, parent, out)?;
                    if let Some(c) = close {
                        out.push_str(&src[c.clone()]);
                    }
                    continue;
                }
                let Some(contract) = catalog.contracts.iter().find(|c| c.tag == name) else {
                    let hint = suggest::did_you_mean(name, catalog.contracts.iter().map(|c| c.tag))
                        .map(|x| format!("; did you mean <{x}>?"))
                        .unwrap_or_default();
                    return Err(err(*line, format!("unknown component <{name}>{hint}")));
                };
                if let Some(p) = contract.parent
                    && parent != Some(p)
                {
                    return Err(err(*line, format!("<{name}> belongs inside <{p}>")));
                }
                check_attrs(contract, attrs).map_err(|m| err(*line, m))?;
                if contract.slots.is_empty() && has_content(src, children) {
                    return Err(err(*line, format!("<{name}> takes no content")));
                }
                match contract.render {
                    Render::Element { tag, class } => {
                        element(src, contract, tag, class, attrs, open, *line, out)?;
                        emit_into(src, children, catalog, Some(name), out)?;
                        out.push_str(&format!("</{tag}>"));
                        if let Some(c) = close {
                            out.push_str(&pad(&src[c.clone()]));
                        }
                    }
                    _ => {
                        return Err(err(*line, format!("<{name}> is not supported yet")));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Whether the children hold anything but whitespace.
fn has_content(src: &str, children: &[Node<'_>]) -> bool {
    children.iter().any(|c| match c {
        Node::Text(s) => !src[s.clone()].trim().is_empty(),
        Node::Element { .. } => true,
    })
}

/// Writes the start tag of an `Element` component.
#[allow(clippy::too_many_arguments)]
fn element(
    src: &str,
    contract: &Contract,
    tag: &str,
    class: &str,
    attrs: &[Attr<'_>],
    open: &std::ops::Range<usize>,
    line: usize,
    out: &mut String,
) -> Result<(), CompileError> {
    let mut classes = class.to_owned();
    let mut extra: Vec<String> = Vec::new();
    let mut passthrough = String::new();
    for a in attrs {
        let n = bare(a.name);
        if let Some(p) = contract.props.iter().find(|p| p.name == n) {
            let e = attrs::prop_expr(a, p.kind, p.values)
                .map_err(|m| err(a.line, format!("<{}> {m}", contract.tag)))?;
            let modifier = format!("{class}--{n}");
            match (p.kind, e.as_str()) {
                (Kind::Bool, "true") => classes.push_str(&format!(" {modifier}")),
                (Kind::Bool, "false") => {}
                (Kind::Bool, _) => extra.push(format!("{{% if {e} %}} {modifier}{{% endif %}}")),
                _ => {}
            }
        } else if n == "class" {
            let value = a.value.unwrap_or("");
            if a.name.starts_with(':') {
                extra.push(format!(" {{{{ {} }}}}", value.trim()));
            } else if !value.is_empty() {
                extra.push(format!(" {value}"));
            }
        } else if let Some(name) = a.name.strip_prefix(':') {
            let value = a.value.unwrap_or("").trim();
            if value.is_empty() {
                return Err(err(
                    a.line,
                    format!("<{}> \":{name}\" needs an expression", contract.tag),
                ));
            }
            passthrough.push_str(&format!(" {name}=\"{{{{ {value} }}}}\""));
        } else {
            passthrough.push(' ');
            passthrough.push_str(&src[a.span.clone()]);
        }
    }
    let _ = line;
    // Plain extra classes join the attribute value; conditional ones follow.
    for e in &extra {
        if e.starts_with(' ') {
            classes.push_str(e);
        }
    }
    out.push_str(&format!("<{tag} class=\"{classes}"));
    for e in extra.iter().filter(|e| !e.starts_with(' ')) {
        out.push_str(e);
    }
    out.push('"');
    out.push_str(&passthrough);
    out.push('>');
    out.push_str(&pad(&src[open.clone()]));
    Ok(())
}

//! Components with code generation of their own: `rx-page` and `rx-push` (Decisions 16 and 17 of
//! #372).

use super::Catalog;
use super::CompileError;
use super::attrs;
use super::contracts::Contract;
use super::contracts::Module;
use super::emit::{check_attrs, emit_into, err, pad};
use super::scan::Attr;
use super::tree::Node;
use std::collections::BTreeSet;
use std::ops::Range;

/// What nested code generation needs.
pub(super) struct Ctx<'a, 'b> {
    pub src: &'a str,
    pub catalog: &'a Catalog<'a>,
    pub used: &'b mut BTreeSet<Module>,
    pub counter: &'b mut usize,
}

impl Ctx<'_, '_> {
    fn nodes(
        &mut self,
        nodes: &[Node<'_>],
        parent: &str,
        out: &mut String,
    ) -> Result<(), CompileError> {
        emit_into(
            self.src,
            nodes,
            self.catalog,
            Some(parent),
            self.used,
            self.counter,
            out,
        )
    }
}

fn bare(name: &str) -> &str {
    name.strip_prefix(':').unwrap_or(name)
}

fn attr<'a, 'b>(attrs: &'b [Attr<'a>], name: &str) -> Option<&'b Attr<'a>> {
    attrs.iter().find(|a| bare(a.name) == name)
}

fn text_prop(a: &Attr<'_>, tag: &str) -> Result<String, CompileError> {
    attrs::prop_expr(a, attrs::Kind::Text, &[]).map_err(|m| err(a.line, format!("<{tag}> {m}")))
}

/// `<rx-page>`: `{% extends %}`, the `seo` block, the content block, then one block per slot.
#[allow(clippy::too_many_arguments)]
pub(super) fn page(
    cx: &mut Ctx<'_, '_>,
    contract: &Contract,
    attrs: &[Attr<'_>],
    children: &[Node<'_>],
    open: &Range<usize>,
    close: Option<&Range<usize>>,
    line: usize,
    out: &mut String,
) -> Result<(), CompileError> {
    check_attrs(contract, attrs, &[]).map_err(|m| err(line, m))?;
    let layout = attr(attrs, "layout").expect("checked: required");
    let value = layout.value.unwrap_or("");
    if layout.name.starts_with(':') || value.contains("{{") || value.contains("{%") {
        return Err(err(
            layout.line,
            "<rx-page> \"layout\" must be a file name, not {{ }}".to_owned(),
        ));
    }
    // Split the children: `<rx-slot name="X">` become blocks, the rest is the content.
    let mut content: Vec<Node<'_>> = Vec::new();
    let mut slots: Vec<(&str, &[Node<'_>])> = Vec::new();
    for child in children {
        if let Node::Element {
            name,
            attrs: sa,
            children: inner,
            line: at,
            ..
        } = child
            && name == "rx-slot"
        {
            let want = sa
                .iter()
                .find(|a| a.name == "name")
                .and_then(|a| a.value)
                .unwrap_or("");
            if want.is_empty() {
                return Err(err(*at, "<rx-slot> needs a name here".to_owned()));
            }
            if want == "content" {
                return Err(err(
                    *at,
                    "<rx-page> \"content\" is the page itself, not a slot".to_owned(),
                ));
            }
            if slots.iter().any(|(n, _)| *n == want) {
                return Err(err(*at, format!("<rx-page> has the slot \"{want}\" twice")));
            }
            slots.push((want, inner));
        } else {
            content.push(child.clone());
        }
    }
    let title = attr(attrs, "title");
    let description = attr(attrs, "description");
    let seo_slot = slots.iter().position(|(n, _)| *n == "seo");
    if title.is_some() && seo_slot.is_some() {
        return Err(err(line, "give title or a seo slot, not both".to_owned()));
    }
    out.push_str(&format!("{{% extends {} %}}", attrs::literal(value.trim())));
    out.push_str(&pad(&cx.src[open.clone()]));
    if let Some(t) = title {
        let mut args = format!("title={}", text_prop(t, "rx-page")?);
        if let Some(d) = description {
            args.push_str(&format!(", description={}", text_prop(d, "rx-page")?));
        }
        out.push_str(&format!(
            "{{% block seo %}}{{{{ seo({args}) }}}}{{% endblock %}}"
        ));
    }
    out.push_str("{% block content %}");
    cx.nodes(&content, "rx-page", out)?;
    out.push_str("{% endblock %}");
    // Error lines inside a moved slot can be off by the lines of the content above: accepted
    // (Decision 16).
    for (name, body) in slots {
        out.push_str(&format!("{{% block {name} %}}"));
        cx.nodes(body, "rx-page", out)?;
        out.push_str("{% endblock %}");
    }
    if let Some(c) = close {
        out.push_str(&pad(&cx.src[c.clone()]));
    }
    Ok(())
}

/// `<rx-push stack="…" once="…">`: `{% call push("S", once="O") %}…{% endcall %}`.
#[allow(clippy::too_many_arguments)]
pub(super) fn push(
    cx: &mut Ctx<'_, '_>,
    contract: &Contract,
    attrs: &[Attr<'_>],
    children: &[Node<'_>],
    open: &Range<usize>,
    close: Option<&Range<usize>>,
    line: usize,
    out: &mut String,
) -> Result<(), CompileError> {
    check_attrs(contract, attrs, &[]).map_err(|m| err(line, m))?;
    if children
        .iter()
        .any(|c| matches!(c, Node::Element { name, .. } if name == "rx-slot"))
    {
        return Err(err(line, "<rx-push> has no named slots".to_owned()));
    }
    let stack = text_prop(attr(attrs, "stack").expect("checked: required"), "rx-push")?;
    let mut args = stack;
    if let Some(o) = attr(attrs, "once") {
        args.push_str(&format!(", once={}", text_prop(o, "rx-push")?));
    }
    out.push_str(&format!("{{% call push({args}) %}}"));
    out.push_str(&pad(&cx.src[open.clone()]));
    cx.nodes(children, "rx-push", out)?;
    out.push_str("{% endcall %}");
    if let Some(c) = close {
        out.push_str(&pad(&cx.src[c.clone()]));
    }
    Ok(())
}

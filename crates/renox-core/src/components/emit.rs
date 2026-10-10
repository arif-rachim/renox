//! Code generation: the tree back to template source, components replaced by plain MiniJinja or
//! HTML. Generated code never holds a line break; a replaced span that had some keeps them inside
//! a `{# … #}` comment, so the lines of the source stay where they were (Decision 8 of #372).

use super::attrs::{self, Kind};
use super::contracts::{Contract, Render, Slot, Special};
use super::scan::Attr;
use super::special::{self, Ctx};
use super::tree::{self, Node};
use super::{Catalog, CompileError, suggest};
use std::collections::BTreeSet;

/// A template module imported at the top of the output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Import {
    pub alias: String,
    pub path: String,
}

pub(super) type Used = BTreeSet<Import>;

/// A comment holding as many line breaks as `span_text` has, or nothing.
pub(super) fn pad(span_text: &str) -> String {
    let k = span_text.matches('\n').count();
    if k == 0 {
        String::new()
    } else {
        format!("{{#{}#}}", "\n".repeat(k))
    }
}

/// Whether a macro component hands the attribute on in `attrs` (Decision 12 of #372).
fn passes_through(name: &str) -> bool {
    name.contains('-')
        || name.starts_with('@')
        || matches!(
            name,
            "min"
                | "max"
                | "step"
                | "pattern"
                | "minlength"
                | "maxlength"
                | "inputmode"
                | "autofocus"
                | "tabindex"
                | "title"
                | "form"
                | "accept"
        )
}

fn bare(name: &str) -> &str {
    name.strip_prefix(':').unwrap_or(name)
}

/// Checks the attributes against a list of prop names; `open` lets other attributes through.
pub(super) fn check_names(
    tag: &str,
    names: &[&str],
    required: &[&str],
    open: bool,
    attrs: &[Attr<'_>],
    filled: &[&str],
) -> Result<(), String> {
    for a in attrs {
        let n = bare(a.name);
        if names.contains(&n) || open {
            continue;
        }
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
    for p in required {
        if !attrs.iter().any(|a| bare(a.name) == *p) && !filled.contains(p) {
            return Err(format!("<{tag}> needs the attribute \"{p}\""));
        }
    }
    Ok(())
}

/// Checks the attributes against the contract.
pub(super) fn check_attrs(
    contract: &Contract,
    attrs: &[Attr<'_>],
    filled: &[&str],
) -> Result<(), String> {
    let tag = contract.tag;
    let element = matches!(
        contract.render,
        Render::Element { .. } | Render::Special(Special::Form)
    );
    let route = attrs.iter().find(|a| bare(a.name) == "route");
    if route.is_some() {
        let Some(prop) = contract.route_prop else {
            return Err(format!("<{tag}> has no route shortcut"));
        };
        if attrs.iter().any(|a| bare(a.name) == prop) {
            return Err(format!("<{tag}> takes route or \"{prop}\", not both"));
        }
    }
    for a in attrs {
        let n = bare(a.name);
        if n == "route" {
            continue;
        }
        if n == "class" && contract.attrs {
            if element {
                continue;
            }
            return Err(format!(
                "<{tag}> doesn't take class yet; merging class comes with #373"
            ));
        }
        if contract.props.iter().any(|p| p.name == n)
            || (contract.attrs && (element || passes_through(n)))
        {
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
        let by_route = route.is_some() && contract.route_prop == Some(p.name);
        if !attrs.iter().any(|a| bare(a.name) == p.name) && !filled.contains(&p.name) && !by_route {
            return Err(format!("<{tag}> needs the attribute \"{}\"", p.name));
        }
    }
    Ok(())
}

pub(super) fn err(line: usize, message: String) -> CompileError {
    CompileError { line, message }
}

/// The tree as template source.
pub(crate) fn emit(
    src: &str,
    nodes: &[Node<'_>],
    catalog: &Catalog,
) -> Result<String, CompileError> {
    let mut out = String::with_capacity(src.len());
    let mut used = Used::new();
    let mut counter = 0;
    let is_page = |n: &Node<'_>| matches!(n, Node::Element { name, .. } if name == "rx-page");
    if nodes.iter().any(is_page) {
        let stray = nodes.iter().find_map(|n| match n {
            Node::Text(s) if !blank(&src[s.clone()]) => {
                Some(1 + src[..s.start].matches('\n').count())
            }
            Node::Element { line, .. } if !is_page(n) => Some(*line),
            _ => None,
        });
        let second = nodes
            .iter()
            .filter(|n| is_page(n))
            .nth(1)
            .and_then(|n| match n {
                Node::Element { line, .. } => Some(*line),
                _ => None,
            });
        if let Some(line) = stray.or(second) {
            return Err(err(
                line,
                "<rx-page> must hold the whole template".to_owned(),
            ));
        }
    }
    emit_into(src, nodes, catalog, None, &mut used, &mut counter, &mut out)?;
    if used.is_empty() {
        return Ok(out);
    }
    let mut head = String::new();
    for m in used {
        head.push_str(&format!("{{% import \"{}\" as {} %}}", m.path, m.alias));
    }
    head.push_str(&out);
    Ok(head)
}

/// The control-flow attributes of one element, taken out of its attribute list.
struct Flow<'a> {
    each: Option<String>,
    cond: Option<String>,
    otherwise: bool,
    can: Option<String>,
    /// The attributes that stay.
    attrs: Vec<Attr<'a>>,
    /// Where the removed attributes were, with the whitespace before each.
    removed: Vec<std::ops::Range<usize>>,
}

impl Flow<'_> {
    /// The condition of the `{% if %}`, from `rx-if` and `can`.
    fn condition(&self) -> Option<String> {
        let can = self.can.as_ref().map(|a| format!("can({a:?})"));
        match (&self.cond, can) {
            (Some(c), Some(can)) if c.contains(" or ") => Some(format!("({c}) and {can}")),
            (Some(c), Some(can)) => Some(format!("{c} and {can}")),
            (Some(c), None) => Some(c.clone()),
            (None, can) => can,
        }
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flow_of<'a>(
    src: &str,
    component: bool,
    attrs: &[Attr<'a>],
    open: &std::ops::Range<usize>,
) -> Result<Flow<'a>, CompileError> {
    let mut f = Flow {
        each: None,
        cond: None,
        otherwise: false,
        can: None,
        attrs: Vec::new(),
        removed: Vec::new(),
    };
    for a in attrs {
        match a.name {
            "rx-for" => {
                let v = one_line(a.value.unwrap_or(""));
                let words: Vec<&str> = v.split(' ').collect();
                let ok = words
                    .iter()
                    .position(|w| *w == "in")
                    .is_some_and(|p| p > 0 && p + 1 < words.len());
                if !ok {
                    return Err(err(a.line, "rx-for needs \"item in list\"".to_owned()));
                }
                f.each = Some(v);
            }
            "rx-if" => {
                let v = one_line(a.value.unwrap_or(""));
                if v.is_empty() {
                    return Err(err(a.line, "rx-if needs a condition".to_owned()));
                }
                f.cond = Some(v);
            }
            "rx-else" => f.otherwise = true,
            "can" if component => {
                let v = a.value.unwrap_or("").trim();
                if v.is_empty() {
                    return Err(err(a.line, "can needs an ability".to_owned()));
                }
                f.can = Some(v.to_owned());
            }
            _ => {
                f.attrs.push(a.clone());
                continue;
            }
        }
        let mut start = a.span.start;
        while start > open.start + 1 && src.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        f.removed.push(start..a.span.end);
    }
    if f.otherwise && (f.cond.is_some() || f.each.is_some()) {
        let line = attrs.first().map_or(0, |a| a.line);
        return Err(err(
            line,
            "rx-else can't be combined with rx-if or rx-for".to_owned(),
        ));
    }
    Ok(f)
}

/// Whether the text holds only whitespace and comments.
fn blank(text: &str) -> bool {
    let mut t = text.trim_start();
    loop {
        if let Some(r) = t.strip_prefix("<!--") {
            let Some(p) = r.find("-->") else { return false };
            t = r[p + 3..].trim_start();
        } else if let Some(r) = t.strip_prefix("{#") {
            let Some(p) = r.find("#}") else { return false };
            t = r[p + 2..].trim_start();
        } else {
            return t.is_empty();
        }
    }
}

fn element_attrs<'n, 'a>(node: &'n Node<'a>) -> Option<(&'n [Attr<'a>], usize)> {
    match node {
        Node::Element { attrs, line, .. } => Some((attrs, *line)),
        Node::Text(_) => None,
    }
}

fn has_attr(node: &Node<'_>, name: &str) -> bool {
    element_attrs(node).is_some_and(|(a, _)| a.iter().any(|x| x.name == name))
}

pub(super) fn emit_into(
    src: &str,
    nodes: &[Node<'_>],
    catalog: &Catalog,
    parent: Option<&str>,
    used: &mut Used,
    counter: &mut usize,
    out: &mut String,
) -> Result<(), CompileError> {
    let mut i = 0;
    while i < nodes.len() {
        let node = &nodes[i];
        let Node::Element {
            name, attrs, open, ..
        } = node
        else {
            if let Node::Text(span) = node {
                out.push_str(&src[span.clone()]);
            }
            i += 1;
            continue;
        };
        let flow = flow_of(src, tree::is_component(name), attrs, open)?;
        if flow.otherwise {
            let line = attrs.first().map_or(0, |a| a.line);
            let prev = nodes[..i]
                .iter()
                .rev()
                .find(|n| !matches!(n, Node::Text(s) if blank(&src[s.clone()])));
            let msg = if prev.is_some_and(|p| has_attr(p, "rx-for")) {
                "rx-else can't follow an element with rx-for"
            } else {
                "rx-else must follow an element with rx-if"
            };
            return Err(err(line, msg.to_owned()));
        }
        // An `rx-if` element directly followed by an `rx-else` one.
        let mut partner = None;
        if flow.cond.is_some() && flow.each.is_none() {
            let mut j = i + 1;
            if let Some(Node::Text(s)) = nodes.get(j)
                && blank(&src[s.clone()])
            {
                j += 1;
            }
            if nodes.get(j).is_some_and(|n| has_attr(n, "rx-else")) {
                partner = Some(j);
            }
        }
        if let Some(each) = &flow.each {
            out.push_str(&format!("{{% for {each} %}}"));
        }
        let cond = flow.condition();
        if let Some(c) = &cond {
            out.push_str(&format!("{{% if {c} %}}"));
        }
        emit_node(src, node, &flow, catalog, parent, used, counter, out)?;
        if let Some(j) = partner {
            for n in &nodes[i + 1..j] {
                if let Node::Text(s) = n {
                    out.push_str(&src[s.clone()]);
                }
            }
            out.push_str("{% else %}");
            let b = &nodes[j];
            let Node::Element {
                name, attrs, open, ..
            } = b
            else {
                unreachable!("has_attr is true only for elements")
            };
            let bf = flow_of(src, tree::is_component(name), attrs, open)?;
            if let Some(each) = &bf.each {
                out.push_str(&format!("{{% for {each} %}}"));
            }
            let bc = bf.condition();
            if let Some(c) = &bc {
                out.push_str(&format!("{{% if {c} %}}"));
            }
            emit_node(src, b, &bf, catalog, parent, used, counter, out)?;
            if bc.is_some() {
                out.push_str("{% endif %}");
            }
            if bf.each.is_some() {
                out.push_str("{% endfor %}");
            }
            out.push_str("{% endif %}");
            i = j + 1;
        } else {
            if cond.is_some() {
                out.push_str("{% endif %}");
            }
            if flow.each.is_some() {
                out.push_str("{% endfor %}");
            }
            i += 1;
        }
    }
    Ok(())
}

/// One element: a component replaced, or a plain tag without its control-flow attributes.
#[allow(clippy::too_many_arguments)]
fn emit_node(
    src: &str,
    node: &Node<'_>,
    flow: &Flow<'_>,
    catalog: &Catalog,
    parent: Option<&str>,
    used: &mut Used,
    counter: &mut usize,
    out: &mut String,
) -> Result<(), CompileError> {
    let Node::Element {
        name,
        children,
        open,
        close,
        line,
        ..
    } = node
    else {
        return Ok(());
    };
    let attrs = &flow.attrs;
    if !tree::is_component(name) {
        let mut pos = open.start;
        let mut gone = String::new();
        for r in &flow.removed {
            out.push_str(&src[pos..r.start]);
            gone.push_str(&src[r.clone()]);
            pos = r.end;
        }
        out.push_str(&src[pos..open.end]);
        out.push_str(&pad(&gone));
        emit_into(src, children, catalog, parent, used, counter, out)?;
        if let Some(c) = close {
            out.push_str(&src[c.clone()]);
        }
        return Ok(());
    }
    if name.starts_with("app-") {
        return super::app::usage(src, node, attrs, catalog, used, counter, out);
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
    if let Render::Special(sp) = contract.render {
        let mut cx = Ctx {
            src,
            catalog,
            used,
            counter,
        };
        let close = close.as_ref();
        return match sp {
            Special::Page => {
                if parent.is_some() {
                    return Err(err(
                        *line,
                        "<rx-page> must hold the whole template".to_owned(),
                    ));
                }
                special::page(&mut cx, contract, attrs, children, open, close, *line, out)
            }
            Special::Push => {
                special::push(&mut cx, contract, attrs, children, open, close, *line, out)
            }
            Special::Wizard => {
                special::wizard(&mut cx, contract, attrs, children, open, close, *line, out)
            }
            Special::Form => {
                special::form(&mut cx, contract, attrs, children, open, close, *line, out)
            }
            _ => Err(err(*line, format!("<{name}> is not supported yet"))),
        };
    }
    let parts = split_slots(src, contract, children, *line)?;
    let filled: Vec<&str> = parts.iter().filter_map(|(slot, _)| slot.into).collect();
    for (slot, _) in &parts {
        if let Some(p) = slot.into
            && attrs.iter().any(|a| bare(a.name) == p)
        {
            return Err(err(
                *line,
                format!("<{name}> give \"{p}\" or content, not both"),
            ));
        }
    }
    check_attrs(contract, attrs, &filled).map_err(|m| err(*line, m))?;
    if parts.is_empty() && has_content(src, children) {
        return Err(err(*line, format!("<{name}> takes no content")));
    }
    match contract.render {
        Render::Element { tag, class } => {
            element(src, contract, tag, class, attrs, open, *line, out)?;
            emit_into(src, children, catalog, Some(name), used, counter, out)?;
            out.push_str(&format!("</{tag}>"));
            if let Some(c) = close {
                out.push_str(&pad(&src[c.clone()]));
            }
        }
        Render::Macro { module, name: mac } => {
            used.insert(Import {
                alias: module.alias().to_owned(),
                path: module.path().to_owned(),
            });
            // Each argument with its place: props as written, then slots, then `attrs`.
            let mut args: Vec<(usize, String)> = Vec::new();
            let mut extra: Vec<String> = Vec::new();
            for (order, a) in attrs.iter().enumerate() {
                let n = bare(a.name);
                if n == "route" {
                    let e = attrs::prop_expr(a, Kind::Text, &[])
                        .map_err(|m| err(a.line, format!("<{}> {m}", contract.tag)))?;
                    let prop = contract.route_prop.expect("checked against the contract");
                    let at = contract
                        .props
                        .iter()
                        .position(|p| p.name == prop)
                        .unwrap_or(0);
                    args.push((at, format!("{}=route({e})", attrs::snake(prop))));
                    continue;
                }
                let Some(p) = contract.props.iter().find(|p| p.name == n) else {
                    let e = attrs::prop_expr(a, Kind::Text, &[])
                        .map_err(|m| err(a.line, format!("<{}> {m}", contract.tag)))?;
                    extra.push(format!("{}: {e}", attrs::literal(n)));
                    continue;
                };
                let e = attrs::prop_expr(a, p.kind, p.values)
                    .map_err(|m| err(a.line, format!("<{}> {m}", contract.tag)))?;
                args.push((order, format!("{}={e}", attrs::snake(n))));
            }
            let mut pre = String::new();
            let mut caller: Option<(&Slot, &[Node<'_>])> = None;
            let mut consumed_default = false;
            for (slot, nodes) in &parts {
                if slot.name.is_empty() && slot.into.is_none() {
                    caller = Some((slot, nodes));
                    continue;
                }
                *counter += 1;
                let n = *counter;
                let mut body = String::new();
                if slot.name.is_empty() {
                    consumed_default = true;
                }
                emit_into(src, nodes, catalog, Some(name), used, counter, &mut body)?;
                pre.push_str(&format!("{{% set __rx_slot_{n} %}}{body}{{% endset %}}"));
                let target = slot.into.unwrap_or(slot.name);
                args.push((
                    attrs.len() + n,
                    format!("{}=__rx_slot_{n}", attrs::snake(target)),
                ));
            }
            out.push_str(&pre);
            args.sort_by_key(|(at, _)| *at);
            let mut args: Vec<String> = args.into_iter().map(|(_, a)| a).collect();
            if !extra.is_empty() {
                args.push(format!("attrs={{{}}}", extra.join(", ")));
            }
            let call = format!("{}.{mac}({})", module.alias(), args.join(", "));
            let caller = caller.filter(|(slot, nodes)| !slot.optional || has_content(src, nodes));
            if let Some((slot, nodes)) = caller {
                let params = slot.args.join(", ");
                if slot.args.is_empty() {
                    out.push_str(&format!("{{% call {call} %}}"));
                } else {
                    out.push_str(&format!("{{% call({params}) {call} %}}"));
                }
                out.push_str(&pad(&src[open.clone()]));
                emit_into(src, nodes, catalog, Some(name), used, counter, out)?;
                out.push_str("{% endcall %}");
            } else {
                out.push_str(&format!("{{{{ {call} }}}}"));
                out.push_str(&pad(&src[open.clone()]));
                if !consumed_default {
                    for c in children {
                        if let Node::Text(span) = c {
                            out.push_str(&pad(&src[span.clone()]));
                        }
                    }
                }
            }
            if let Some(c) = close {
                out.push_str(&pad(&src[c.clone()]));
            }
        }
        _ => {
            return Err(err(*line, format!("<{name}> is not supported yet")));
        }
    }
    Ok(())
}

/// Whether the children hold anything but whitespace.
pub(super) fn has_content(src: &str, children: &[Node<'_>]) -> bool {
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

/// The content for each slot: `<rx-slot name="x">` children for named slots, the rest for the
/// default slot. Slots without content are left out.
fn split_slots<'c, 'a>(
    src: &str,
    contract: &'c Contract,
    children: &[Node<'a>],
    line: usize,
) -> Result<Vec<(&'c Slot, Vec<Node<'a>>)>, CompileError> {
    let mut parts: Vec<(&Slot, Vec<Node<'a>>)> = Vec::new();
    let mut rest: Vec<Node<'a>> = Vec::new();
    for child in children {
        let Node::Element {
            name,
            attrs,
            children: inner,
            open,
            close,
            line: at,
        } = child
        else {
            rest.push(child.clone());
            continue;
        };
        if name != "rx-slot" {
            rest.push(child.clone());
            continue;
        }
        let want = attrs
            .iter()
            .find(|a| a.name == "name")
            .and_then(|a| a.value)
            .unwrap_or("");
        let tag = contract.tag;
        let named: Vec<&str> = contract
            .slots
            .iter()
            .map(|s| s.name)
            .filter(|n| !n.is_empty())
            .collect();
        if want.is_empty() {
            return Err(err(*at, "<rx-slot> needs a name here".to_owned()));
        }
        let Some(slot) = contract.slots.iter().find(|s| s.name == want) else {
            let hint = suggest::did_you_mean(want, named.iter().copied())
                .map(|y| format!("; did you mean \"{y}\"?"))
                .unwrap_or_else(|| ".".to_owned());
            return Err(err(*at, format!("<{tag}> has no slot \"{want}\"{hint}")));
        };
        let mut body: Vec<Node<'a>> = Vec::new();
        let _ = (open, close);
        body.extend(inner.iter().cloned());
        parts.push((slot, body));
    }
    if let Some(slot) = contract.slots.iter().find(|s| s.name.is_empty()) {
        if has_content(src, &rest) || (slot.into.is_none() && !slot.optional) {
            parts.push((slot, rest));
        }
    } else if has_content(src, &rest) {
        return Err(err(line, format!("<{}> takes no content", contract.tag)));
    }
    Ok(parts)
}

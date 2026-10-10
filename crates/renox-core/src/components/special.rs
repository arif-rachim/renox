//! Components with code generation of their own: `rx-page` and `rx-push` (Decisions 16 and 17 of
//! #372).

use super::Catalog;
use super::CompileError;
use super::attrs;
use super::contracts::Contract;
use super::contracts::Module;
use super::emit::{blank, check_attrs, emit_into, err, pad, with_row};
use super::scan::Attr;
use super::tree::Node;
use std::ops::Range;

/// What nested code generation needs.
pub(super) struct Ctx<'a, 'b> {
    pub src: &'a str,
    pub catalog: &'a Catalog<'a>,
    pub used: &'b mut super::emit::Used,
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

/// A wizard step read from its tag: key, title, content, start tag, end tag.
type Step<'a, 'b> = (
    String,
    Option<String>,
    &'b [Node<'a>],
    &'b Range<usize>,
    Option<&'b Range<usize>>,
);

/// `<rx-wizard>`: the steps come from the `<rx-wizard-step>` children, and each gets the
/// wizard's id (Decision 3 of #374).
#[allow(clippy::too_many_arguments)]
pub(super) fn wizard(
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
    let step_contract = cx
        .catalog
        .contracts
        .iter()
        .find(|c| c.tag == "rx-wizard-step")
        .expect("rx-wizard-step is built in");
    let mut steps: Vec<Step<'_, '_>> = Vec::new();
    let mut gaps = String::new();
    for child in children {
        match child {
            Node::Text(span) => {
                let text = &cx.src[span.clone()];
                if !text.trim().is_empty() {
                    return Err(err(
                        line,
                        "<rx-wizard> holds only <rx-wizard-step>".to_owned(),
                    ));
                }
                gaps.push_str(&pad(text));
            }
            Node::Element {
                name,
                attrs: sa,
                children: inner,
                open: sopen,
                close: sclose,
                line: at,
            } => {
                if name != "rx-wizard-step" {
                    return Err(err(
                        *at,
                        "<rx-wizard> holds only <rx-wizard-step>".to_owned(),
                    ));
                }
                check_attrs(step_contract, sa, &[]).map_err(|m| err(*at, m))?;
                let key = text_prop(attr(sa, "key").expect("checked: required"), name)?;
                let title = attr(sa, "title").map(|t| text_prop(t, name)).transpose()?;
                steps.push((key, title, inner, sopen, sclose.as_ref()));
            }
        }
    }
    cx.used.insert(super::emit::Import {
        alias: Module::Ui.alias().to_owned(),
        path: Module::Ui.path().to_owned(),
    });
    let id = text_prop(attr(attrs, "id").expect("checked: required"), "rx-wizard")?;
    let list: Vec<String> = steps
        .iter()
        .map(|(k, t, ..)| format!("[{k}, {}]", t.as_deref().unwrap_or(k)))
        .collect();
    let mut args = format!(
        "id={id}, steps=[{}], submit_label={}",
        list.join(", "),
        text_prop(
            attr(attrs, "submit-label").expect("checked: required"),
            "rx-wizard"
        )?
    );
    for (name, arg) in [
        ("back-label", "back_label"),
        ("next-label", "next_label"),
        ("cancel-label", "cancel_label"),
    ] {
        if let Some(a) = attr(attrs, name) {
            args.push_str(&format!(", {arg}={}", text_prop(a, "rx-wizard")?));
        }
    }
    if let Some(a) = attr(attrs, "cancel") {
        let e = attrs::prop_expr(a, attrs::Kind::Bool, &[])
            .map_err(|m| err(a.line, format!("<rx-wizard> {m}")))?;
        args.push_str(&format!(", cancel={e}"));
    }
    out.push_str(&format!("{{% call __rx_ui.wizard({args}) %}}"));
    out.push_str(&pad(&cx.src[open.clone()]));
    out.push_str(&gaps);
    for (key, title, inner, sopen, sclose) in steps {
        let mut call = format!("id={id}, key={key}");
        if let Some(t) = title {
            call.push_str(&format!(", title={t}"));
        }
        out.push_str(&format!("{{% call __rx_ui.wizard_step({call}) %}}"));
        out.push_str(&pad(&cx.src[sopen.clone()]));
        cx.nodes(inner, "rx-wizard-step", out)?;
        out.push_str("{% endcall %}");
        if let Some(c) = sclose {
            out.push_str(&pad(&cx.src[c.clone()]));
        }
    }
    out.push_str("{% endcall %}");
    if let Some(c) = close {
        out.push_str(&pad(&cx.src[c.clone()]));
    }
    Ok(())
}

const TABLE_HOLDS: &str =
    "<rx-table> holds <rx-column>, <rx-row-actions> and <rx-slot name=\"empty\">";

fn plain_name(v: &str) -> bool {
    let mut chars = v.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `<rx-table>`: the columns become a `table(...)` call, a loop over the rows and, for a page of
/// rows, its pagination (Decision 18 of #372).
#[allow(clippy::too_many_arguments)]
pub(super) fn table(
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
    let find = |tag: &str| {
        cx.catalog
            .contracts
            .iter()
            .find(|c| c.tag == tag)
            .copied()
            .unwrap_or_else(|| panic!("{tag} is built in"))
    };
    let (column, actions) = (find("rx-column"), find("rx-row-actions"));
    let data = |name: &str| -> Result<Option<String>, CompileError> {
        attr(attrs, name)
            .map(|a| {
                attrs::prop_expr(a, attrs::Kind::Data, &[])
                    .map_err(|m| err(a.line, format!("<rx-table> {m}")))
            })
            .transpose()
    };
    let rows = data("rows")?.expect("checked: required");
    let var = match attr(attrs, "as") {
        Some(a) => {
            let v = a.value.unwrap_or("").trim();
            if a.name.starts_with(':') || !plain_name(v) {
                return Err(err(
                    a.line,
                    "<rx-table> \"as\" must be a plain name, such as product".to_owned(),
                ));
            }
            v.to_owned()
        }
        None => "row".to_owned(),
    };
    let key = data("key")?.unwrap_or_else(|| format!("{var}.id"));
    let caption = attr(attrs, "caption")
        .map(|a| text_prop(a, "rx-table"))
        .transpose()?;
    for m in [Module::Ui, Module::Pagination] {
        cx.used.insert(super::emit::Import {
            alias: m.alias().to_owned(),
            path: m.path().to_owned(),
        });
    }
    *cx.counter += 1;
    let n = *cx.counter;
    let id = match attr(attrs, "id") {
        Some(a)
            if !a.name.starts_with(':')
                && a.value.is_some_and(|v| {
                    !v.is_empty()
                        && v.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "_-.:".contains(c))
                }) =>
        {
            a.value.unwrap_or("").to_owned()
        }
        Some(a) => format!("{{{{ {} }}}}", text_prop(a, "rx-table")?),
        None => format!("rx-table-{n}"),
    };
    let card = match attr(attrs, "card") {
        Some(a) => Some(
            attrs::prop_expr(a, attrs::Kind::Bool, &[])
                .map_err(|m| err(a.line, format!("<rx-table> {m}")))?,
        ),
        None => None,
    };

    let mut head: Vec<String> = Vec::new();
    let mut cells = String::new();
    let mut empty = String::new();
    let mut seen_empty = false;
    for child in children {
        match child {
            Node::Text(span) => {
                let text = &cx.src[span.clone()];
                if !blank(text) {
                    return Err(err(line, TABLE_HOLDS.to_owned()));
                }
                cells.push_str(&pad(text));
            }
            Node::Element {
                name,
                attrs: ca,
                children: inner,
                open: copen,
                close: cclose,
                line: at,
            } => match name.as_str() {
                "rx-column" => {
                    check_attrs(&column, ca, &[]).map_err(|m| err(*at, m))?;
                    let label = text_prop(attr(ca, "label").expect("checked: required"), name)?;
                    let (mut num, mut narrow) = (false, false);
                    if let Some(a) = attr(ca, "align") {
                        match (a.name.starts_with(':'), a.value) {
                            (false, Some("num")) => num = true,
                            (false, Some("start")) => {}
                            _ => {
                                return Err(err(
                                    a.line,
                                    "<rx-column> \"align\" is start or num".to_owned(),
                                ));
                            }
                        }
                    }
                    if let Some(a) = attr(ca, "hide-narrow") {
                        match (a.name.starts_with(':'), a.value) {
                            (false, None | Some("true")) => narrow = true,
                            (false, Some("false")) => {}
                            _ => {
                                return Err(err(
                                    a.line,
                                    "<rx-column> \"hide-narrow\" is true or false: write hide-narrow"
                                        .to_owned(),
                                ));
                            }
                        }
                    }
                    let td = match (num, narrow) {
                        (false, false) => {
                            head.push(label);
                            "<td>"
                        }
                        (true, false) => {
                            head.push(format!("[{label}, \"num\"]"));
                            "<td class=\"rx-num\">"
                        }
                        (false, true) => {
                            head.push(format!("[{label}, \"hide-narrow\"]"));
                            "<td class=\"rx-hide-narrow\">"
                        }
                        (true, true) => {
                            head.push(format!("[{label}, \"num rx-hide-narrow\"]"));
                            "<td class=\"rx-num rx-hide-narrow\">"
                        }
                    };
                    cells.push_str(td);
                    cells.push_str(&pad(&cx.src[copen.clone()]));
                    with_row(&var, &key, || cx.nodes(inner, "rx-column", &mut cells))?;
                    cells.push_str("</td>");
                    if let Some(c) = cclose {
                        cells.push_str(&pad(&cx.src[c.clone()]));
                    }
                }
                "rx-row-actions" => {
                    check_attrs(&actions, ca, &[]).map_err(|m| err(*at, m))?;
                    head.push("[\"\", \"num\"]".to_owned());
                    cells.push_str("<td class=\"rx-num\">{% call __rx_ui.row_actions() %}");
                    cells.push_str(&pad(&cx.src[copen.clone()]));
                    with_row(&var, &key, || cx.nodes(inner, "rx-row-actions", &mut cells))?;
                    cells.push_str("{% endcall %}</td>");
                    if let Some(c) = cclose {
                        cells.push_str(&pad(&cx.src[c.clone()]));
                    }
                }
                "rx-slot" => {
                    let want = ca
                        .iter()
                        .find(|a| a.name == "name")
                        .and_then(|a| a.value)
                        .unwrap_or("");
                    if want != "empty" {
                        return Err(err(*at, TABLE_HOLDS.to_owned()));
                    }
                    if seen_empty {
                        return Err(err(
                            *at,
                            "<rx-table> has the slot \"empty\" twice".to_owned(),
                        ));
                    }
                    seen_empty = true;
                    empty.push_str(&pad(&cx.src[copen.clone()]));
                    cx.nodes(inner, "rx-table", &mut empty)?;
                    if let Some(c) = cclose {
                        empty.push_str(&pad(&cx.src[c.clone()]));
                    }
                }
                _ => return Err(err(*at, TABLE_HOLDS.to_owned())),
            },
        }
    }
    if head.is_empty() {
        return Err(err(
            line,
            "<rx-table> needs at least one <rx-column>".to_owned(),
        ));
    }

    let r = &rows;
    let hx = format!(
        "<div hx-boost=\"true\" hx-target=\"#{id}\" hx-select=\"#{id}\" hx-swap=\"outerHTML\">"
    );
    out.push_str(&format!(
        "{{% set __rx_rows_{n} = ({r}.items if {r}.items is defined else {r}) %}}<div class=\"rx-stack\" id=\"{id}\">{{% if __rx_rows_{n} %}}"
    ));
    let card_open = match &card {
        None => String::new(),
        Some(c) if c == "true" => "<div class=\"rx-card\">".to_owned(),
        Some(c) if c == "false" => String::new(),
        Some(c) => format!("{{% if {c} %}}<div class=\"rx-card\">{{% endif %}}"),
    };
    let card_close = match &card {
        None => String::new(),
        Some(c) if c == "true" => "</div>".to_owned(),
        Some(c) if c == "false" => String::new(),
        Some(c) => format!("{{% if {c} %}}</div>{{% endif %}}"),
    };
    out.push_str(&card_open);
    let mut args = format!("head=[{}]", head.join(", "));
    if let Some(c) = caption {
        args.push_str(&format!(", caption={c}"));
    }
    out.push_str(&format!("{{% call __rx_ui.table({args}) %}}"));
    out.push_str(&pad(&cx.src[open.clone()]));
    out.push_str(&format!("{{% for {var} in __rx_rows_{n} %}}<tr>"));
    out.push_str(&cells);
    out.push_str("</tr>{% endfor %}{% endcall %}");
    out.push_str(&card_close);
    out.push_str(&format!(
        "{{% if {r}.last_page is defined %}}{hx}{{{{ __rx_pagination.pagination({r}) }}}}</div>{{% elif {r}.has_next is defined %}}{hx}{{{{ __rx_pagination.simple_pagination({r}) }}}}</div>{{% endif %}}{{% else %}}"
    ));
    out.push_str(&empty);
    out.push_str("{% endif %}</div>");
    if let Some(c) = close {
        out.push_str(&pad(&cx.src[c.clone()]));
    }
    Ok(())
}

/// `<rx-form>`: a `<form>` with the CSRF field (not for GET) and the method field (PUT, PATCH,
/// DELETE). Every other attribute passes through as written (Decision 17 of #372).
#[allow(clippy::too_many_arguments)]
pub(super) fn form(
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
    let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
    let mut method = "POST".to_owned();
    if let Some(m) = attr(attrs, "method") {
        let v = m.value.unwrap_or("");
        if m.name.starts_with(':') || v.contains("{{") || v.contains("{%") {
            return Err(err(
                m.line,
                "<rx-form> \"method\" must be plain text".to_owned(),
            ));
        }
        attrs::prop_expr(m, attrs::Kind::Enum, &methods)
            .map_err(|e| err(m.line, format!("<rx-form> {e}")))?;
        method = v.to_owned();
    }
    let action = if let Some(r) = attr(attrs, "route") {
        Some(format!("route({})", text_prop(r, "rx-form")?))
    } else {
        attr(attrs, "action")
            .map(|a| text_prop(a, "rx-form"))
            .transpose()?
    };
    let live = match attr(attrs, "live") {
        Some(a) => Some(
            attrs::prop_expr(a, attrs::Kind::Bool, &[])
                .map_err(|m| err(a.line, format!("<rx-form> {m}")))?,
        ),
        None => None,
    };
    let get = method == "GET";
    out.push_str(&format!(
        "<form method=\"{}\"",
        if get { "get" } else { "post" }
    ));
    if let Some(a) = action {
        out.push_str(&format!(" action=\"{{{{ {a} }}}}\""));
    }
    match live.as_deref() {
        None | Some("false") => {}
        Some("true") => out.push_str(" data-live-validate novalidate"),
        Some(e) => out.push_str(&format!(
            "{{% if {e} %}} data-live-validate novalidate{{% endif %}}"
        )),
    }
    for a in attrs {
        let n = bare(a.name);
        if matches!(n, "action" | "route" | "method" | "live") {
            continue;
        }
        if let Some(name) = a.name.strip_prefix(':') {
            let value = a.value.unwrap_or("").trim();
            if value.is_empty() {
                return Err(err(
                    a.line,
                    format!("<rx-form> \":{name}\" needs an expression"),
                ));
            }
            out.push_str(&format!(
                " {name}=\"{{{{ {} }}}}\"",
                value.replace(['\n', '\r'], " ")
            ));
        } else {
            out.push(' ');
            out.push_str(&cx.src[a.span.clone()]);
        }
    }
    out.push('>');
    out.push_str(&pad(&cx.src[open.clone()]));
    if !get {
        out.push_str("{{ csrf_field() }}");
    }
    if matches!(method.as_str(), "PUT" | "PATCH" | "DELETE") {
        out.push_str(&format!("{{{{ method_field(\"{method}\") }}}}"));
    }
    cx.nodes(children, "rx-form", out)?;
    out.push_str("</form>");
    if let Some(c) = close {
        out.push_str(&pad(&cx.src[c.clone()]));
    }
    Ok(())
}

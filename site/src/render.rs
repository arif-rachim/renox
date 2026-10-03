//! Markdown to HTML for the site: anchors on headings and a table of
//! contents, doctest setup lines (`# …`) left out of Rust examples, and
//! links between the repository's files turned into the site's addresses.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use crate::content::{PAGES, Page, page_for_path};

/// The repository, for files that aren't pages.
pub const REPOSITORY: &str = "https://github.com/arif-rachim/renox";
const RAW: &str = "https://raw.githubusercontent.com/arif-rachim/renox/main";

/// A heading in a page's table of contents.
#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// A page, rendered once.
#[derive(Debug, Clone, Serialize)]
pub struct Rendered {
    /// The first `# heading`, else the sidebar name.
    pub title: String,
    pub html: String,
    pub toc: Vec<Heading>,
    /// The words of the page (no code), for search.
    #[serde(skip)]
    pub text: String,
}

/// Every page, rendered at first use.
pub static RENDERED: LazyLock<HashMap<&'static str, Rendered>> =
    LazyLock::new(|| PAGES.iter().map(|page| (page.slug, render(page))).collect());

/// `page` as HTML.
pub fn render(page: &Page) -> Rendered {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let events: Vec<Event> = Parser::new_ext(page.markdown, options).collect();
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut title = None;
    let mut toc = Vec::new();
    let mut ids = HashSet::new();
    let mut text = String::new();
    let mut rust_block = false;
    let mut in_code = false;
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level;
                let mut end = i + 1;
                while !matches!(events[end], Event::End(TagEnd::Heading(_))) {
                    end += 1;
                }
                let inner: Vec<Event> = events[i + 1..end].to_vec();
                let plain = plain_text(&inner);
                text.push_str(&plain);
                text.push('\n');
                if level == HeadingLevel::H1 && title.is_none() {
                    // Shown in the page header instead.
                    title = Some(plain);
                    i = end + 1;
                    continue;
                }
                let id = unique_id(&slugify(&plain), &mut ids);
                let number = heading_number(level);
                if number <= 3 {
                    toc.push(Heading {
                        level: number,
                        id: id.clone(),
                        text: plain,
                    });
                }
                out.push(Event::Html(CowStr::from(format!(
                    "<h{number} id=\"{id}\"><a class=\"site-anchor\" href=\"#{id}\" aria-label=\"Link to this section\">#</a>"
                ))));
                out.extend(inner.into_iter().map(|event| rewrite(event, page.path)));
                out.push(Event::Html(CowStr::from(format!("</h{number}>"))));
                i = end + 1;
                continue;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split([',', ' ']).next().unwrap_or_default().to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                rust_block = language == "rust" || language.is_empty() && is_doctest(kind);
                let language = if language.is_empty() {
                    "text".into()
                } else {
                    language
                };
                out.push(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                    CowStr::from(language),
                ))));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                rust_block = false;
                out.push(events[i].clone());
            }
            Event::Text(body) if in_code && rust_block => {
                out.push(Event::Text(CowStr::from(without_hidden_lines(body))));
            }
            Event::Text(words) | Event::Code(words) if !in_code => {
                text.push_str(words);
                text.push(' ');
                out.push(events[i].clone());
            }
            event => out.push(rewrite(event.clone(), page.path)),
        }
        i += 1;
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, out.into_iter());
    Rendered {
        title: title.unwrap_or_else(|| page.nav.to_owned()),
        html,
        toc,
        text,
    }
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn is_doctest(kind: &CodeBlockKind) -> bool {
    matches!(kind, CodeBlockKind::Fenced(info) if info.is_empty())
}

/// The text of a heading's events.
fn plain_text(events: &[Event]) -> String {
    let mut text = String::new();
    for event in events {
        if let Event::Text(t) | Event::Code(t) = event {
            text.push_str(t);
        }
    }
    text.trim().to_owned()
}

/// `Forms and validation` → `forms-and-validation`, as GitHub makes anchors.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.trim().to_lowercase().chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            slug.push(c);
        } else if c.is_whitespace() {
            slug.push('-');
        }
    }
    if slug.is_empty() {
        "section".into()
    } else {
        slug
    }
}

fn unique_id(slug: &str, ids: &mut HashSet<String>) -> String {
    if ids.insert(slug.to_owned()) {
        return slug.to_owned();
    }
    (1..)
        .map(|n| format!("{slug}-{n}"))
        .find(|candidate| ids.insert(candidate.clone()))
        .expect("an unused id")
}

/// A doctest's setup lines (`# use …`, `#` alone) aren't shown, as rustdoc
/// hides them; `##` stands for a line starting with `#`.
fn without_hidden_lines(code: &str) -> String {
    let mut shown = String::with_capacity(code.len());
    for line in code.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("##") {
            let indent = &line[..line.len() - trimmed.len()];
            shown.push_str(indent);
            shown.push_str(&trimmed[1..]);
        } else if trimmed == "#" || trimmed == "#\n" || trimmed.starts_with("# ") {
            continue;
        } else {
            shown.push_str(line);
        }
    }
    shown
}

fn rewrite(event: Event<'static>, from: &str) -> Event<'static> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: CowStr::from(link_target(&dest_url, from)),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: CowStr::from(image_source(&dest_url, from)),
            title,
            id,
        }),
        other => other,
    }
}

/// Where a link in the file `from` goes on the site: another page, a
/// section of this one, or the repository on GitHub.
pub fn link_target(url: &str, from: &str) -> String {
    if url.starts_with('#') || url.contains("://") || url.starts_with("mailto:") {
        return url.to_owned();
    }
    let (path, anchor) = match url.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (url, String::new()),
    };
    let resolved = resolve(from, path);
    match page_for_path(&resolved) {
        Some(page) => format!("/docs/{}{anchor}", page.slug),
        None if resolved.is_empty() => format!("{REPOSITORY}{anchor}"),
        None if is_directory(&resolved) => format!("{REPOSITORY}/tree/main/{resolved}{anchor}"),
        None => format!("{REPOSITORY}/blob/main/{resolved}{anchor}"),
    }
}

fn image_source(url: &str, from: &str) -> String {
    if url.contains("://") {
        url.to_owned()
    } else {
        format!("{RAW}/{}", resolve(from, url))
    }
}

/// `path` relative to the file `from`, as a path from the repository's root.
fn resolve(from: &str, path: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop(); // the file's own name
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// Folders (`examples/crud`) have no extension.
fn is_directory(path: &str) -> bool {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .is_some_and(|name| !name.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_become_site_addresses() {
        assert_eq!(link_target("routing.md", "docs/ui.md"), "/docs/routing");
        assert_eq!(
            link_target("docs/routing.md#guards", "README.md"),
            "/docs/routing#guards"
        );
        assert_eq!(
            link_target("../CHEATSHEET.md", "docs/ui.md"),
            "/docs/cheatsheet"
        );
        assert_eq!(link_target("#models", "README.md"), "#models");
        assert_eq!(
            link_target("examples/crud", "README.md"),
            format!("{REPOSITORY}/tree/main/examples/crud")
        );
        assert_eq!(
            link_target("examples/crud/src/lib.rs", "README.md"),
            format!("{REPOSITORY}/blob/main/examples/crud/src/lib.rs")
        );
        assert_eq!(
            link_target("https://htmx.org", "README.md"),
            "https://htmx.org"
        );
    }

    #[test]
    fn doctest_setup_lines_are_hidden() {
        let code = "# use renox::prelude::*;\nfn main() {}\n    # let x = 1;\n#\n## not hidden\n";
        assert_eq!(without_hidden_lines(code), "fn main() {}\n# not hidden\n");
    }

    #[test]
    fn headings_get_github_style_ids() {
        assert_eq!(slugify("Forms and validation"), "forms-and-validation");
        assert_eq!(slugify("`Valid<T>` reads"), "validt-reads");
        let mut ids = HashSet::new();
        assert_eq!(unique_id("a", &mut ids), "a");
        assert_eq!(unique_id("a", &mut ids), "a-1");
    }
}

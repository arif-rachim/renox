//! A chapter's Markdown as HTML.
//!
//! The docs site's renderer, trimmed for a course: headings get ids and an
//! anchor, code goes through the docs' code panel, `> [!TIP]` callouts have
//! the docs' markup, raw HTML is shown as text (except `<kbd>`), links to
//! other sites get `rel="noopener"`, and the five `learn:` directives become
//! their markup (`*_html` below; later issues replace one function each).

use std::collections::HashSet;
use std::ops::Range;

use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};
use renox_site::icons;
use renox_site::render::{Heading, callout, code_panel, plain_text, slugify, unique_id};

use crate::challenge;
use crate::code;
use crate::content::Problem;
use crate::directive::{self, Challenge, Directive, Include, Quiz, Screenshot, Video, Viewport};

/// `markdown` (a chapter's body) as HTML, with its table of contents, its
/// directives and its problems. `file` is the chapter's path relative to
/// `learn/` (`content/stages/02-models.md`) and `front_lines` the number of
/// lines its front matter takes, so problems carry lines of the whole file.
pub fn render(
    markdown: &str,
    file: &str,
    front_lines: usize,
) -> (String, Vec<Heading>, Vec<Directive>, Vec<Problem>) {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_GFM;
    let events: Vec<(Event, Range<usize>)> = Parser::new_ext(markdown, options)
        .into_offset_iter()
        .collect();
    // The file line of a byte offset in `markdown`.
    let line_of = |offset: usize| {
        front_lines + 1 + markdown[..offset.min(markdown.len())].matches('\n').count()
    };
    let stage = stage_of(file);

    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut toc = Vec::new();
    let mut directives = Vec::new();
    let mut problems = Vec::new();
    let mut ids = HashSet::new();
    let mut i = 0;
    while i < events.len() {
        let (event, range) = &events[i];
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let mut end = i + 1;
                while !matches!(events[end].0, Event::End(TagEnd::Heading(_))) {
                    end += 1;
                }
                let inner: Vec<Event> = events[i + 1..end]
                    .iter()
                    .map(|(e, _)| transform(e.clone()))
                    .collect();
                i = end + 1;
                if *level == HeadingLevel::H1 {
                    problems.push(Problem {
                        file: file.to_owned(),
                        line: line_of(range.start),
                        message: "a chapter's headings start at ##: its title is the page's <h1>"
                            .into(),
                    });
                    continue;
                }
                let plain = plain_text(&inner);
                let id = unique_id(&slugify(&plain), &mut ids);
                let number = match level {
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    _ => 6,
                };
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
                out.extend(inner);
                out.push(Event::Html(CowStr::from(format!("</h{number}>"))));
                continue;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.trim().to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                let mut code = String::new();
                let mut end = i + 1;
                while !matches!(events[end].0, Event::End(TagEnd::CodeBlock)) {
                    if let Event::Text(t) = &events[end].0 {
                        code.push_str(t);
                    }
                    end += 1;
                }
                i = end + 1;
                if let Some(name) = info.strip_prefix("learn:") {
                    // The fence is on this line, the body starts on the next.
                    let line = line_of(range.start) + 1;
                    let html = match directive::parse(name, &code, line) {
                        Ok(d) => match directive_html(&d, stage) {
                            Ok(html) => {
                                directives.push(d);
                                html
                            }
                            Err(message) => {
                                let p = Problem {
                                    file: file.to_owned(),
                                    line: line - 1,
                                    message,
                                };
                                let html = problem_html(&p);
                                problems.push(p);
                                html
                            }
                        },
                        Err(mut p) => {
                            p.file = file.to_owned();
                            let html = problem_html(&p);
                            problems.push(p);
                            html
                        }
                    };
                    out.push(Event::Html(CowStr::from(html)));
                } else {
                    let language = info.split([',', ' ']).next().unwrap_or_default();
                    let language = if language.is_empty() {
                        "text"
                    } else {
                        language
                    };
                    out.push(Event::Html(CowStr::from(code_panel(language, &code))));
                }
                continue;
            }
            Event::Start(Tag::BlockQuote(kind)) => {
                let html = match callout(*kind) {
                    Some((class, icon, title)) => format!(
                        "<div class=\"site-callout site-callout--{class}\">\
                         <p class=\"site-callout__title\">{}{title}</p>",
                        icons::svg(icon)
                    ),
                    None => "<div class=\"site-callout site-callout--quote\">".to_owned(),
                };
                out.push(Event::Html(CowStr::from(html)));
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                out.push(Event::Html(CowStr::from("</div>")));
            }
            other => out.push(transform(other.clone())),
        }
        i += 1;
    }
    let mut rendered = String::new();
    html::push_html(&mut rendered, out.into_iter());
    (rendered, toc, directives, problems)
}

/// One event as the chapter shows it: raw HTML becomes text (`<kbd>` stays),
/// and links to other sites get `rel="noopener"`.
fn transform(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Html(raw) | Event::InlineHtml(raw) => {
            if matches!(raw.trim(), "<kbd>" | "</kbd>") {
                Event::InlineHtml(raw)
            } else {
                Event::Text(raw)
            }
        }
        Event::Start(Tag::Link {
            dest_url, title, ..
        }) if dest_url.starts_with("http") => {
            let title = if title.is_empty() {
                String::new()
            } else {
                format!(" title=\"{}\"", escape(&title))
            };
            Event::Html(CowStr::from(format!(
                "<a href=\"{}\"{title} rel=\"noopener\">",
                escape(&dest_url)
            )))
        }
        other => other,
    }
}

/// Inline Markdown (a quiz's question, a choice, an explanation) as HTML,
/// without the paragraph around it.
fn inline(markdown: &str) -> String {
    let events = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH).map(transform);
    let mut out = String::new();
    html::push_html(&mut out, events);
    let out = out.trim();
    out.strip_prefix("<p>")
        .and_then(|s| s.strip_suffix("</p>"))
        .unwrap_or(out)
        .to_owned()
}

/// The stage number in a file name like `content/stages/02-models.md`.
fn stage_of(file: &str) -> u8 {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.get(..2).and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// HTML-escaped text.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A mistake in the Markdown, shown where it is.
fn problem_html(p: &Problem) -> String {
    format!(
        "<div class=\"learn-problem\" role=\"alert\">{}</div>",
        escape(&p.to_string())
    )
}

/// A directive's markup, or what is wrong with it (code that isn't there).
fn directive_html(d: &Directive, stage: u8) -> Result<String, String> {
    Ok(match d {
        Directive::Include(d) => include_html(d, stage)?,
        Directive::Changes => changes_html(stage)?,
        Directive::Screenshot(d) => screenshot_html(d, stage),
        Directive::Video(d) => video_html(d)?,
        Directive::Challenge(d) => challenge_html(d, stage)?,
        Directive::Quiz(d) => quiz_html(d),
    })
}

/// Code from the course app: a region or a whole file of a stage (the
/// chapter's own unless `stage:` says another), named above its code.
fn include_html(d: &Include, stage: u8) -> Result<String, String> {
    let stage = d.stage.unwrap_or(stage);
    let (path, language, text) = match (&d.region, &d.file) {
        (Some(name), _) => {
            let r = code::region(stage, name)
                .ok_or_else(|| format!("learn:include: stage {stage} has no region `{name}`"))?;
            (r.path, r.language, r.text)
        }
        (None, Some(path)) => {
            let f = code::file(stage, path)
                .ok_or_else(|| format!("learn:include: stage {stage} has no file `{path}`"))?;
            (
                f.path,
                code::language_of(f.path),
                f.text.trim_end_matches('\n'),
            )
        }
        (None, None) => return Err("learn:include: give exactly one of `region` or `file`".into()),
    };
    Ok(format!(
        "<figure class=\"learn-code\"><figcaption><code>{}</code></figcaption>{}</figure>",
        escape(path),
        code_panel(language, text)
    ))
}

/// What the chapter's stage changed against the stage before: one fold-out
/// diff per added, changed or removed file.
fn changes_html(stage: u8) -> Result<String, String> {
    if stage == 0 {
        return Err("learn:changes: stage 0 has no stage before it".into());
    }
    let mut out = String::from("<div class=\"learn-changes\">");
    for change in code::changes(stage) {
        let badge = match change.kind {
            "added" => "success",
            "removed" => "error",
            _ => "info",
        };
        out.push_str(&format!(
            "<details class=\"learn-change\"><summary><code>{}</code> <span class=\"learn-change__kind rx-badge rx-badge--{badge}\">{}</span></summary><pre class=\"learn-diff\">",
            escape(change.path),
            change.kind
        ));
        for line in change.diff.split('\n') {
            let class = match line.as_bytes().first() {
                Some(b'+') => "add",
                Some(b'-') => "del",
                _ => "ctx",
            };
            out.push_str(&format!("<span class=\"{class}\">{}</span>", escape(line)));
        }
        out.push_str("</pre></details>");
    }
    out.push_str("</div>");
    Ok(out)
}

/// A screenshot of the stage's app, made by `learn/media/screens.mjs`.
fn screenshot_html(d: &Screenshot, stage: u8) -> String {
    let (width, height) = d.viewport.size();
    let phone = if d.viewport == Viewport::Phone {
        " learn-shot--phone"
    } else {
        ""
    };
    let caption = d
        .caption
        .as_ref()
        .map(|c| format!("<figcaption>{}</figcaption>", inline(c)))
        .unwrap_or_default();
    format!(
        "<figure class=\"learn-shot{phone}\"><img src=\"/img/s{stage:02}/{}\" alt=\"{}\" width=\"{width}\" height=\"{height}\" loading=\"lazy\" decoding=\"async\">{caption}</figure>",
        d.file,
        escape(&d.alt)
    )
}

/// A video: the player of `media::figure`, with the media URL left as a
/// placeholder the stage page fills in. An id missing from the manifest is
/// reported as a problem.
fn video_html(d: &Video) -> Result<String, String> {
    let video = crate::media::manifest().videos.get(&d.id).ok_or_else(|| {
        format!(
            "learn:video: `{}` is not in content/media.json (record it: learn/README.md, \"Videos\")",
            d.id
        )
    })?;
    Ok(crate::media::figure(
        video,
        &d.title,
        d.caption.as_deref(),
        crate::media::PLACEHOLDER,
    ))
}

/// A code panel under the file's name.
fn file_figure(path: &str, text: &str) -> String {
    format!(
        "<figure class=\"learn-code\"><figcaption><code>{}</code></figcaption>{}</figure>",
        escape(path),
        code_panel(code::language_of(path), text.trim_end_matches('\n'))
    )
}

/// A challenge: its instructions, the files to change as they start, the
/// test to add, the command with the recorded outputs, the solution, and the
/// button `challenge.js` reveals (hidden without JavaScript).
fn challenge_html(d: &Challenge, stage: u8) -> Result<String, String> {
    let c = challenge::find(stage, &d.id)
        .ok_or_else(|| format!("learn:challenge: stage {stage} has no challenge `{}`", d.id))?;
    let stem = c
        .test_path
        .trim_start_matches("tests/")
        .trim_end_matches(".rs");
    let command = format!("cargo test --test {stem}");
    let mut out = format!(
        "<section class=\"learn-challenge\" id=\"challenge-{id}\" data-challenge=\"s{stage:02}/{id}\">\n\
         <p class=\"eyebrow\"><b>{stage:02}</b> Challenge</p>\n<h2 id=\"challenge-{id}-title\">{}</h2>\n\
         <p class=\"learn-challenge__meta\">{} About {} minutes</p>\n{}\n\
         <h3 id=\"challenge-{id}-start\">1. Start from your stage {stage} app</h3>\n\
         <p>Continue in the app you built in the chapters. If you skipped ahead, take the finished stage {stage} app from a clone of the repository.</p>\n{}\n\
         <h3 id=\"challenge-{id}-change\">2. Change {}</h3>\n",
        escape(&c.title),
        icons::svg("clock"),
        c.minutes,
        c.instructions_html,
        code_panel(
            "bash",
            &format!(
                "git clone --depth 1 https://github.com/arif-rachim/renox\ncd renox/learn/course/stage-{stage:02}"
            )
        ),
        if c.files.len() == 1 {
            "this file"
        } else {
            "these files"
        },
        id = c.id,
    );
    for f in &c.files {
        out.push_str(&file_figure(&f.path, f.start));
        out.push('\n');
    }
    out.push_str(&format!(
        "<h3 id=\"challenge-{id}-test\">3. Add the test</h3>\n<p>Save this as <code>{}</code>. It fails until you solve the challenge.</p>\n{}\n\
         <h3 id=\"challenge-{id}-run\">4. Run it</h3>\n{}\n",
        escape(&c.test_path),
        file_figure(&c.test_path, c.test),
        code_panel("bash", &command),
        id = c.id,
    ));
    for (summary, output) in [
        ("What failing looks like", c.failing),
        ("What passing looks like", c.passing),
    ] {
        if let Some(output) = output {
            out.push_str(&format!(
                "<details class=\"learn-challenge__output\"><summary>{summary}</summary>{}</details>\n",
                code_panel("text", output)
            ));
        }
    }
    out.push_str(
        "<details class=\"learn-challenge__solution\"><summary>Show the solution</summary>\n",
    );
    for f in &c.files {
        out.push_str(&file_figure(&f.path, f.solution));
        out.push('\n');
    }
    for f in &c.new_files {
        out.push_str(&file_figure(&f.path, f.solution));
        out.push('\n');
    }
    out.push_str(
        "</details>\n<button type=\"button\" class=\"rx-button rx-button--secondary learn-challenge__done\" data-challenge-done aria-pressed=\"false\" hidden>I passed it</button>\n</section>",
    );
    Ok(out)
}

/// A quiz: the markup #535's script reads. Options are numbered from 0 in
/// the order written; the button stays disabled until the script enables it.
fn quiz_html(d: &Quiz) -> String {
    let mut out = format!(
        "<form class=\"learn-quiz\" id=\"quiz-{id}\" data-quiz=\"{id}\">\n\
         <p class=\"eyebrow\"><b>?</b> Quick check</p>\n<fieldset>\n\
         <legend class=\"learn-quiz__question\">{}</legend>\n",
        inline(&d.question),
        id = d.id
    );
    for (i, choice) in d.choices.iter().enumerate() {
        let correct = if choice.correct { " data-correct" } else { "" };
        out.push_str(&format!(
            "<label class=\"learn-quiz__choice\"><input type=\"radio\" name=\"quiz-{id}\" value=\"{i}\"{correct}> <span>{}</span></label>\n",
            inline(&choice.text),
            id = d.id
        ));
    }
    out.push_str(
        "</fieldset>\n<p class=\"learn-quiz__result\" role=\"status\" aria-live=\"polite\"></p>\n",
    );
    if let Some(explain) = &d.explain {
        out.push_str(&format!(
            "<div class=\"learn-quiz__explain\" hidden><p>{}</p></div>\n",
            inline(explain)
        ));
    }
    out.push_str(
        "<button type=\"submit\" class=\"rx-button rx-button--primary learn-quiz__check\" disabled>Check</button>\n</form>",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kbd_passes_through_and_other_html_is_text() {
        let (html, ..) = render(
            "Press <kbd>Ctrl</kbd> and <b>x</b>.\n",
            "content/stages/00-x.md",
            0,
        );
        assert!(html.contains("<kbd>Ctrl</kbd>"), "{html}");
        assert!(html.contains("&lt;b&gt;x&lt;/b&gt;"), "{html}");
    }
}

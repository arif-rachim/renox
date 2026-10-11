//! The directives of a chapter: fenced blocks whose info string is
//! `learn:<name>` and whose body is one `key: value` per line.
//!
//! ````markdown
//! ```learn:include
//! region: menu.item-model
//! ```
//! ````
//!
//! | Directive | Keys (required in **bold**) |
//! |---|---|
//! | `learn:include` | exactly one of **region**, **file**; stage |
//! | `learn:changes` | none |
//! | `learn:screenshot` | **file**, **page**, **alt**, caption, viewport, wait, click, login |
//! | `learn:video` | **id**, **title**, caption |
//! | `learn:challenge` | **id** |
//! | `learn:quiz` | **id**, **question**, choice…, correct, explain |
//!
//! Empty lines and lines starting with `//` are ignored. Values are trimmed;
//! a value may be quoted (`"…"`) to keep spaces at its ends. Each key may
//! appear once, except `choice` and `correct` in a quiz. A mistake becomes a
//! [`Problem`] with its line in the file.

use serde::Serialize;

use crate::content::Problem;

/// The names `learn:<name>` can take.
pub const NAMES: &[&str] = &[
    "include",
    "changes",
    "screenshot",
    "video",
    "challenge",
    "quiz",
];

/// A parsed directive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Directive {
    /// Code from the course app: a named region or a whole file.
    Include(Include),
    /// What the chapter's stage changed against the stage before.
    Changes,
    /// A screenshot of the course app.
    Screenshot(Screenshot),
    /// A recorded video.
    Video(Video),
    /// A challenge solved on the reader's machine.
    Challenge(Challenge),
    /// A multiple-choice question.
    Quiz(Quiz),
}

/// `learn:include`: exactly one of `region` and `file` is set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Include {
    /// A named region (`[learn:name]` markers in the stage's files).
    pub region: Option<String>,
    /// A whole file, relative to the stage's folder (`src/lib.rs`).
    pub file: Option<String>,
    /// The stage to take it from; the chapter's own when `None`.
    pub stage: Option<u8>,
}

/// `learn:screenshot`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Screenshot {
    /// `[a-z0-9-]+.webp`.
    pub file: String,
    /// A path on the stage's app, starting with `/`.
    pub page: String,
    /// `desktop` (1280 x 800) or `phone` (390 x 844).
    pub viewport: Viewport,
    /// A CSS selector awaited before capturing; `main` when `None`.
    pub wait: Option<String>,
    /// A selector clicked before capturing (a form's submit button).
    pub click: Option<String>,
    /// The seeded user (an email) signed in before the page is opened.
    pub login: Option<String>,
    /// What the picture shows, for readers who can't see it.
    pub alt: String,
    /// A line under the picture.
    pub caption: Option<String>,
}

/// The size of a screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Viewport {
    /// 1280 x 800.
    Desktop,
    /// 390 x 844.
    Phone,
}

impl Viewport {
    /// The image's width and height in pixels.
    pub fn size(self) -> (u32, u32) {
        match self {
            Self::Desktop => (1280, 800),
            Self::Phone => (390, 844),
        }
    }
}

/// `learn:video`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Video {
    /// A key of `content/media.json`.
    pub id: String,
    /// What the video shows.
    pub title: String,
    /// A line under the video.
    pub caption: Option<String>,
}

/// `learn:challenge`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Challenge {
    /// The directory `learn/challenges/sNN/<id>/`.
    pub id: String,
}

/// `learn:quiz`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quiz {
    /// Unique across the course.
    pub id: String,
    /// Inline Markdown.
    pub question: String,
    /// In the order written; exactly one is correct.
    pub choices: Vec<Choice>,
    /// Inline Markdown, shown after the reader answers.
    pub explain: Option<String>,
}

/// One answer of a quiz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    /// Inline Markdown.
    pub text: String,
    /// Whether it is the right answer.
    pub correct: bool,
}

/// The keys each directive takes, in the order the messages list them.
fn keys_of(name: &str) -> &'static [&'static str] {
    match name {
        "include" => &["region", "file", "stage"],
        "changes" => &[],
        "screenshot" => &[
            "file", "page", "alt", "caption", "viewport", "wait", "click", "login",
        ],
        "video" => &["id", "title", "caption"],
        "challenge" => &["id"],
        _ => &["id", "question", "choice", "correct", "explain"],
    }
}

/// Keys that may repeat.
fn repeats(name: &str, key: &str) -> bool {
    name == "quiz" && (key == "choice" || key == "correct")
}

/// A problem at `line`; the caller fills in the file.
fn problem(line: usize, message: impl Into<String>) -> Problem {
    Problem {
        file: String::new(),
        line,
        message: message.into(),
    }
}

/// The `key: value` pairs of a body, in order, with their lines.
struct Pairs {
    name: String,
    fence: usize,
    pairs: Vec<(String, String, usize)>,
}

impl Pairs {
    fn read(name: &str, body: &str, line: usize) -> Result<Self, Problem> {
        let keys = keys_of(name);
        let mut pairs: Vec<(String, String, usize)> = Vec::new();
        for (i, raw) in body.lines().enumerate() {
            let at = line + i;
            let text = raw.trim();
            if text.is_empty() || text.starts_with("//") {
                continue;
            }
            let Some((key, value)) = text.split_once(':') else {
                return Err(problem(at, format!("`{text}` is not `key: value`")));
            };
            let key = key.trim();
            if key.is_empty() {
                return Err(problem(at, format!("`{text}` is not `key: value`")));
            }
            if keys.is_empty() {
                return Err(problem(
                    at,
                    format!("learn:{name} takes no keys (found `{key}`)"),
                ));
            }
            if !keys.contains(&key) {
                return Err(problem(
                    at,
                    format!(
                        "learn:{name} has no key `{key}` (keys: {})",
                        keys.join(", ")
                    ),
                ));
            }
            if !repeats(name, key) && pairs.iter().any(|(k, _, _)| k == key) {
                return Err(problem(at, format!("learn:{name} has `{key}` twice")));
            }
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            pairs.push((key.to_owned(), value.to_owned(), at));
        }
        Ok(Self {
            name: name.to_owned(),
            fence: line.saturating_sub(1),
            pairs,
        })
    }

    /// The first non-empty value of `key`, with its line.
    fn get(&self, key: &str) -> Option<(&str, usize)> {
        self.pairs
            .iter()
            .find(|(k, v, _)| k == key && !v.is_empty())
            .map(|(_, v, at)| (v.as_str(), *at))
    }

    fn need(&self, key: &str) -> Result<(String, usize), Problem> {
        self.get(key)
            .map(|(v, at)| (v.to_owned(), at))
            .ok_or_else(|| problem(self.fence, format!("learn:{} needs `{key}`", self.name)))
    }

    fn rule(&self, key: &str, value: &str, at: usize, rule: &str) -> Problem {
        problem(at, format!("learn:{}: {key} `{value}` {rule}", self.name))
    }

    /// A required value matching `[a-z0-9-]+`.
    fn word(&self, key: &str) -> Result<String, Problem> {
        let (value, at) = self.need(key)?;
        self.check_word(key, &value, at)?;
        Ok(value)
    }

    fn check_word(&self, key: &str, value: &str, at: usize) -> Result<(), Problem> {
        if is_word(value) {
            Ok(())
        } else {
            Err(self.rule(key, value, at, "must match [a-z0-9-]+"))
        }
    }

    fn optional(&self, key: &str) -> Option<String> {
        self.get(key).map(|(v, _)| v.to_owned())
    }
}

/// `[a-z0-9-]+`.
fn is_word(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `[a-z0-9._-]+`: a region's name.
fn is_region_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// A path with no `..`, no empty part, no `\` and no leading `/`.
fn is_relative_path(s: &str) -> bool {
    !s.is_empty() && !s.contains('\\') && !s.split('/').any(|part| part == ".." || part.is_empty())
}

/// Parses the body of a ```` ```learn:<name> ```` block. `body` is the
/// block's text; `line` is the file line of its first body line (the fence
/// is the line before). Mistakes about the block as a whole are reported at
/// the fence, mistakes about one line at that line. The problem's `file` is
/// empty: the caller knows it.
pub fn parse(name: &str, body: &str, line: usize) -> Result<Directive, Problem> {
    if !NAMES.contains(&name) {
        return Err(problem(
            line.saturating_sub(1),
            format!(
                "unknown directive `learn:{name}` (known: {})",
                NAMES.join(", ")
            ),
        ));
    }
    let p = Pairs::read(name, body, line)?;
    Ok(match name {
        "include" => {
            let region = p.get("region");
            let file = p.get("file");
            if region.is_some() == file.is_some() {
                return Err(problem(
                    p.fence,
                    "learn:include: give exactly one of `region` or `file`",
                ));
            }
            if let Some((r, at)) = region
                && !is_region_name(r)
            {
                return Err(p.rule("region", r, at, "must match [a-z0-9._-]+"));
            }
            if let Some((f, at)) = file
                && !is_relative_path(f)
            {
                return Err(p.rule(
                    "file",
                    f,
                    at,
                    "must be a path inside the stage, with no `..` and no leading `/`",
                ));
            }
            let stage = match p.get("stage") {
                Some((value, at)) => Some(
                    value
                        .parse::<u8>()
                        .ok()
                        .filter(|n| *n <= 99)
                        .ok_or_else(|| {
                            p.rule("stage", value, at, "must be a number from 0 to 99")
                        })?,
                ),
                None => None,
            };
            Directive::Include(Include {
                region: p.optional("region"),
                file: p.optional("file"),
                stage,
            })
        }
        "changes" => Directive::Changes,
        "screenshot" => {
            let (file, at) = p.need("file")?;
            let stem = file.strip_suffix(".webp");
            if !stem.is_some_and(is_word) {
                return Err(p.rule("file", &file, at, "must match [a-z0-9-]+\\.webp"));
            }
            let (page, at) = p.need("page")?;
            if !page.starts_with('/') || page.contains(char::is_whitespace) {
                return Err(p.rule(
                    "page",
                    &page,
                    at,
                    "must be a path starting with `/`, with no spaces",
                ));
            }
            let (alt, _) = p.need("alt")?;
            let viewport = match p.get("viewport") {
                None | Some(("desktop", _)) => Viewport::Desktop,
                Some(("phone", _)) => Viewport::Phone,
                Some((other, at)) => {
                    return Err(p.rule("viewport", other, at, "must be desktop or phone"));
                }
            };
            Directive::Screenshot(Screenshot {
                file,
                page,
                viewport,
                wait: p.optional("wait"),
                click: p.optional("click"),
                login: p.optional("login"),
                alt,
                caption: p.optional("caption"),
            })
        }
        "video" => {
            let id = p.word("id")?;
            let (title, _) = p.need("title")?;
            Directive::Video(Video {
                id,
                title,
                caption: p.optional("caption"),
            })
        }
        "challenge" => Directive::Challenge(Challenge { id: p.word("id")? }),
        _ => {
            let id = p.word("id")?;
            let (question, _) = p.need("question")?;
            let mut choices = Vec::new();
            let mut correct = 0;
            for (key, value, at) in &p.pairs {
                if key == "choice" || key == "correct" {
                    if value.is_empty() {
                        return Err(p.rule(key, value, *at, "must not be empty"));
                    }
                    let is_correct = key == "correct";
                    correct += usize::from(is_correct);
                    choices.push(Choice {
                        text: value.clone(),
                        correct: is_correct,
                    });
                }
            }
            if correct != 1 {
                return Err(problem(
                    p.fence,
                    format!("learn:quiz needs exactly one `correct:` (found {correct})"),
                ));
            }
            if choices.len() < 2 {
                return Err(problem(p.fence, "learn:quiz needs at least 2 options"));
            }
            Directive::Quiz(Quiz {
                id,
                question,
                choices,
                explain: p.optional("explain"),
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(name: &str, body: &str, line: usize) -> (usize, String) {
        let p = parse(name, body, line).unwrap_err();
        (p.line, p.message)
    }

    #[test]
    fn every_directive_parses_into_its_type() {
        assert_eq!(
            parse("include", "region: menu.item-model", 5).unwrap(),
            Directive::Include(Include {
                region: Some("menu.item-model".into()),
                file: None,
                stage: None,
            })
        );
        let file = "file: migrations/20261012090000_create_categories_table.up.sql\nstage: 1";
        assert_eq!(
            parse("include", file, 5).unwrap(),
            Directive::Include(Include {
                region: None,
                file: Some("migrations/20261012090000_create_categories_table.up.sql".into()),
                stage: Some(1),
            })
        );
        assert_eq!(parse("changes", "", 5).unwrap(), Directive::Changes);
        let shot = "file: menu.webp\npage: /menu\nalt: The menu page: a table of drinks with their prices\ncaption: The menu after stage 3";
        assert_eq!(
            parse("screenshot", shot, 1).unwrap(),
            Directive::Screenshot(Screenshot {
                file: "menu.webp".into(),
                page: "/menu".into(),
                viewport: Viewport::Desktop,
                wait: None,
                click: None,
                login: None,
                alt: "The menu page: a table of drinks with their prices".into(),
                caption: Some("The menu after stage 3".into()),
            })
        );
        let video = "id: s06-drink-builder\ntitle: The drink builder answering a click\ncaption: Each click sends the signed snapshot and gets HTML back";
        assert_eq!(
            parse("video", video, 1).unwrap(),
            Directive::Video(Video {
                id: "s06-drink-builder".into(),
                title: "The drink builder answering a click".into(),
                caption: Some("Each click sends the signed snapshot and gets HTML back".into()),
            })
        );
        assert_eq!(
            parse("challenge", "id: cheap-items", 1).unwrap(),
            Directive::Challenge(Challenge {
                id: "cheap-items".into()
            })
        );
        let quiz = "id: where-it-runs\nquestion: Where does `rnx serve` answer by default?\nchoice: http://localhost:8000\ncorrect: http://127.0.0.1:3000\nchoice: http://0.0.0.0:80\nexplain: `APP_HOST` defaults to `127.0.0.1` and `APP_PORT` to `3000`; `.env` changes both.";
        assert_eq!(
            parse("quiz", quiz, 1).unwrap(),
            Directive::Quiz(Quiz {
                id: "where-it-runs".into(),
                question: "Where does `rnx serve` answer by default?".into(),
                choices: vec![
                    Choice {
                        text: "http://localhost:8000".into(),
                        correct: false
                    },
                    Choice {
                        text: "http://127.0.0.1:3000".into(),
                        correct: true
                    },
                    Choice {
                        text: "http://0.0.0.0:80".into(),
                        correct: false
                    },
                ],
                explain: Some(
                    "`APP_HOST` defaults to `127.0.0.1` and `APP_PORT` to `3000`; `.env` changes both."
                        .into()
                ),
            })
        );
    }

    #[test]
    fn comments_blank_lines_and_quotes_are_handled() {
        let body = "// a note\n\nid: x\n\ntitle: \" padded: yes \"";
        let Directive::Video(v) = parse("video", body, 1).unwrap() else {
            panic!("not a video");
        };
        assert_eq!(v.title, " padded: yes ");
    }

    #[test]
    fn an_unknown_directive_is_a_problem() {
        assert_eq!(
            msg("screenshoot", "file: a.webp", 8),
            (
                7,
                "unknown directive `learn:screenshoot` (known: include, changes, screenshot, video, challenge, quiz)"
                    .into()
            )
        );
    }

    #[test]
    fn a_line_without_a_colon_is_a_problem() {
        assert_eq!(
            msg("challenge", "id: a\nnonsense here", 10),
            (11, "`nonsense here` is not `key: value`".into())
        );
    }

    #[test]
    fn an_unknown_key_is_a_problem() {
        assert_eq!(
            msg("challenge", "id: a\ncolor: red", 10),
            (11, "learn:challenge has no key `color` (keys: id)".into())
        );
        assert_eq!(
            msg("video", "id: a\nfile: x", 3),
            (
                4,
                "learn:video has no key `file` (keys: id, title, caption)".into()
            )
        );
    }

    #[test]
    fn a_missing_key_is_a_problem() {
        assert_eq!(
            msg("screenshot", "file: menu.webp\npage: /menu", 6),
            (5, "learn:screenshot needs `alt`".into())
        );
        assert_eq!(
            msg("screenshot", "file: menu.webp\nalt: x", 6),
            (5, "learn:screenshot needs `page`".into())
        );
        assert_eq!(
            msg("challenge", "", 6),
            (5, "learn:challenge needs `id`".into())
        );
    }

    #[test]
    fn a_repeated_key_is_a_problem() {
        assert_eq!(
            msg("challenge", "id: a\nid: b", 10),
            (11, "learn:challenge has `id` twice".into())
        );
    }

    #[test]
    fn include_takes_exactly_one_of_region_and_file() {
        let rule = "learn:include: give exactly one of `region` or `file`";
        assert_eq!(
            msg("include", "file: src/lib.rs\nregion: model", 20),
            (19, rule.into())
        );
        assert_eq!(msg("include", "stage: 1", 20), (19, rule.into()));
    }

    #[test]
    fn include_refuses_paths_outside_the_stage() {
        let rule = "must be a path inside the stage, with no `..` and no leading `/`";
        for bad in [
            "../secret.txt",
            "/etc/passwd",
            "src/../../x",
            "a//b",
            "a\\b",
        ] {
            assert_eq!(
                msg("include", &format!("file: {bad}"), 4),
                (4, format!("learn:include: file `{bad}` {rule}")),
                "{bad}"
            );
        }
        assert_eq!(
            msg("include", "file: src/lib.rs\nstage: 100", 4),
            (
                5,
                "learn:include: stage `100` must be a number from 0 to 99".into()
            )
        );
    }

    #[test]
    fn changes_takes_no_keys() {
        assert_eq!(
            msg("changes", "stage: 1", 4),
            (4, "learn:changes takes no keys (found `stage`)".into())
        );
    }

    #[test]
    fn a_quiz_needs_one_correct_and_two_options() {
        assert_eq!(
            msg("quiz", "id: q\nquestion: Q?\nchoice: a\nchoice: b", 9),
            (
                8,
                "learn:quiz needs exactly one `correct:` (found 0)".into()
            )
        );
        assert_eq!(
            msg(
                "quiz",
                "id: q\nquestion: Q?\ncorrect: a\ncorrect: b\nchoice: c",
                9
            ),
            (
                8,
                "learn:quiz needs exactly one `correct:` (found 2)".into()
            )
        );
        assert_eq!(
            msg("quiz", "id: q\nquestion: Q?\ncorrect: a", 9),
            (8, "learn:quiz needs at least 2 options".into())
        );
        assert_eq!(
            msg("quiz", "id: q\ncorrect: a\nchoice: b", 9),
            (8, "learn:quiz needs `question`".into())
        );
    }

    #[test]
    fn ids_must_be_lowercase_words() {
        assert_eq!(
            msg("challenge", "id: Cheap Items", 4),
            (
                4,
                "learn:challenge: id `Cheap Items` must match [a-z0-9-]+".into()
            )
        );
        assert_eq!(
            msg("video", "id: a_b\ntitle: T", 4),
            (4, "learn:video: id `a_b` must match [a-z0-9-]+".into())
        );
        assert_eq!(
            msg("screenshot", "file: Menu.png\nalt: A", 4),
            (
                4,
                "learn:screenshot: file `Menu.png` must match [a-z0-9-]+\\.webp".into()
            )
        );
        assert_eq!(
            msg("include", "region: Model", 4),
            (
                4,
                "learn:include: region `Model` must match [a-z0-9._-]+".into()
            )
        );
    }
}

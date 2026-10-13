//! Signatures as people write them: `quote`'s token text (`pub fn new () ->
//! Self`) printed again with Rust's usual spacing (`pub fn new() -> Self`).

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use std::str::FromStr;

/// `text` (Rust tokens) with the spacing rustfmt would give a signature:
/// none before `,` `;` `:` `>` `)` `?` or after `(` `<` `&` `'`, none around
/// `::`, none between a name and its `(` or `<`, and no trailing comma
/// before `)` or `>`.
pub fn tidy(text: &str) -> String {
    match TokenStream::from_str(text) {
        Ok(tokens) => {
            let mut out = String::new();
            print(tokens, &mut out);
            out
        }
        Err(_) => text.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// What the previous token was, for the spacing decision.
#[derive(Clone, Copy, PartialEq)]
enum Prev {
    Start,
    /// A name or a literal.
    Word,
    /// An opening `(`, `[` or `{`, or a generic `<`.
    Open,
    /// A closing `)`, `]`, `}` or generic `>`.
    Close,
    /// `&`, `'`, `#`, `!`, `*`, `$` or `.`: the next token sticks to it.
    Sticky,
    /// `::`.
    PathSep,
    /// `->`, `=`, `=>`, `+`, `|`, `:` or `,`: the next token has a space.
    Spaced,
    /// A lifetime (`'a`): a type after it has a space (`&'a [u8]`).
    Lifetime,
}

fn push(out: &mut String, space: bool, text: &str) {
    if space && !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
}

fn print(tokens: TokenStream, out: &mut String) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut prev = Prev::Start;
    // Generic `<` still open (so `>` closes one instead of being a
    // comparison or the end of `->`).
    let mut angle = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let next_char = |j: usize| match tokens.get(j) {
            Some(TokenTree::Punct(p)) => Some(p.as_char()),
            _ => None,
        };
        match &tokens[i] {
            TokenTree::Ident(ident) => {
                let space = !matches!(
                    prev,
                    Prev::Start | Prev::Open | Prev::Sticky | Prev::PathSep
                );
                push(out, space, &ident.to_string());
                prev = if out.ends_with(&format!("'{ident}")) {
                    Prev::Lifetime
                } else {
                    Prev::Word
                };
            }
            TokenTree::Literal(lit) => {
                let space = !matches!(
                    prev,
                    Prev::Start | Prev::Open | Prev::Sticky | Prev::PathSep
                );
                push(out, space, &lit.to_string());
                prev = Prev::Word;
            }
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::None => ("", ""),
                };
                // `name(`, `name[`, `>(` (a turbofish call) stick; `-> (A, B)` doesn't.
                let space = match group.delimiter() {
                    Delimiter::Brace => prev != Prev::Start,
                    _ => !matches!(
                        prev,
                        Prev::Start
                            | Prev::Word
                            | Prev::Close
                            | Prev::Open
                            | Prev::Sticky
                            | Prev::PathSep
                    ),
                };
                push(out, space, open);
                let mut inner = String::new();
                print(group.stream(), &mut inner);
                // A trailing comma before the closing delimiter goes.
                let inner = inner.strip_suffix(',').unwrap_or(&inner);
                if group.delimiter() == Delimiter::Brace && !inner.is_empty() {
                    out.push(' ');
                    out.push_str(inner);
                    out.push(' ');
                } else {
                    out.push_str(inner);
                }
                out.push_str(close);
                prev = Prev::Close;
            }
            TokenTree::Punct(p) => {
                let c = p.as_char();
                let joint = p.spacing() == Spacing::Joint;
                // Two- and three-character operators.
                if c == ':' && joint && next_char(i + 1) == Some(':') {
                    out.push_str("::");
                    prev = Prev::PathSep;
                    i += 2;
                    continue;
                }
                if c == '-' && joint && next_char(i + 1) == Some('>') {
                    push(out, prev != Prev::Start, "->");
                    prev = Prev::Spaced;
                    i += 2;
                    continue;
                }
                if c == '=' && joint && next_char(i + 1) == Some('>') {
                    push(out, prev != Prev::Start, "=>");
                    prev = Prev::Spaced;
                    i += 2;
                    continue;
                }
                if c == '.' && joint && next_char(i + 1) == Some('.') {
                    // `..` and `..=`.
                    out.push_str("..");
                    prev = Prev::Sticky;
                    i += 2;
                    continue;
                }
                match c {
                    ',' | ';' => {
                        out.push(c);
                        prev = Prev::Spaced;
                    }
                    ':' => {
                        out.push(':');
                        prev = Prev::Spaced;
                    }
                    '?' => {
                        out.push('?');
                        prev = Prev::Word;
                    }
                    '<' if matches!(prev, Prev::Word | Prev::PathSep | Prev::Close)
                        || prev == Prev::Start =>
                    {
                        out.push('<');
                        angle += 1;
                        prev = Prev::Open;
                    }
                    '<' => {
                        // After `as`, `impl`, `=` and the like: `<T as Trait>`.
                        push(
                            out,
                            !matches!(prev, Prev::Open | Prev::Sticky | Prev::Start),
                            "<",
                        );
                        angle += 1;
                        prev = Prev::Open;
                    }
                    '>' if angle > 0 => {
                        out.push('>');
                        angle -= 1;
                        prev = Prev::Close;
                    }
                    '\'' => {
                        push(
                            out,
                            !matches!(
                                prev,
                                Prev::Start | Prev::Open | Prev::Sticky | Prev::PathSep
                            ),
                            "'",
                        );
                        prev = Prev::Sticky;
                    }
                    '&' | '*' | '#' | '$' => {
                        push(
                            out,
                            !matches!(
                                prev,
                                Prev::Start | Prev::Open | Prev::Sticky | Prev::PathSep
                            ),
                            &c.to_string(),
                        );
                        prev = Prev::Sticky;
                    }
                    '!' => {
                        // `name!(…)` (a macro) sticks; `!Send` has a space before.
                        push(
                            out,
                            !matches!(prev, Prev::Word | Prev::Start | Prev::Open | Prev::Sticky),
                            "!",
                        );
                        prev = Prev::Sticky;
                    }
                    '.' => {
                        out.push('.');
                        prev = Prev::Sticky;
                    }
                    _ => {
                        push(
                            out,
                            prev != Prev::Start && prev != Prev::Open,
                            &c.to_string(),
                        );
                        prev = if joint { Prev::Sticky } else { Prev::Spaced };
                    }
                }
            }
        }
        i += 1;
    }
}

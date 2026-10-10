//! Template components: `<rx-…>` and `<app-…>` tags compiled to plain MiniJinja when a template
//! loads. This step only holds the tag scanner and the "did you mean" helper; nothing calls them yet.
#![allow(dead_code)]

mod scan;
mod suggest;

/// A mistake found while compiling a template, with the line it is on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompileError {
    /// The line of the template source.
    pub line: usize,
    /// What is wrong.
    pub message: String,
}

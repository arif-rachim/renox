//! Each course stage's models and their typed columns (#529), read with
//! `syn` from `learn/course/stage-NN/src/**/*.rs`: a struct whose
//! `#[derive(…)]` has `Model`, unless `#[model(no_typed_columns)]`; a field
//! is a column unless `#[model(skip)]`, its constant named in SHOUTY_SNAKE
//! as `#[derive(Model)]` names it.

use super::api::{cfg_enabled, doc_text, first_sentence};
use super::tidy::tidy;
use heck::{ToShoutySnakeCase, ToSnakeCase};
use proc_macro2::TokenTree;
use quote::ToTokens;
use serde::Serialize;
use std::path::Path;
use syn::{Attribute, Fields, Item, Meta};

#[derive(Serialize, Debug)]
pub struct Model {
    pub name: String,
    pub table: String,
    pub key: String,
    pub doc: String,
    pub columns: Vec<Column>,
}

#[derive(Serialize, Debug)]
pub struct Column {
    #[serde(rename = "const")]
    pub konst: String,
    pub column: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub doc: String,
}

/// The idents inside every `#[name(…)]` of `attrs`.
fn list_idents(attrs: &[Attribute], name: &str) -> Vec<TokenTree> {
    attrs
        .iter()
        .filter_map(|a| match &a.meta {
            Meta::List(list) if list.path.is_ident(name) => Some(list.tokens.clone()),
            _ => None,
        })
        .flat_map(|t| t.into_iter())
        .collect()
}

fn has_ident(tokens: &[TokenTree], name: &str) -> bool {
    tokens
        .iter()
        .any(|t| matches!(t, TokenTree::Ident(i) if i == name))
}

/// `name = "value"` among `tokens`.
fn string_value(tokens: &[TokenTree], name: &str) -> Option<String> {
    tokens.windows(3).find_map(|w| match w {
        [
            TokenTree::Ident(i),
            TokenTree::Punct(p),
            TokenTree::Literal(l),
        ] if i == name && p.as_char() == '=' => {
            syn::parse2::<syn::LitStr>(TokenTree::Literal(l.clone()).into())
                .ok()
                .map(|s| s.value())
        }
        _ => None,
    })
}

fn collect(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|x| x == "rs") {
            files.push(path);
        }
    }
}

/// Every model of the stage app at `stage` (its directory), sorted by name.
pub fn models(stage: &Path) -> Vec<Model> {
    let mut files = Vec::new();
    collect(&stage.join("src"), &mut files);
    let mut out = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let parsed = syn::parse_file(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let mut items = parsed.items;
        // Inline modules too.
        while let Some(item) = items.pop() {
            match item {
                Item::Mod(m) => {
                    if let Some((_, inner)) = m.content {
                        items.extend(inner);
                    }
                }
                Item::Struct(s) if cfg_enabled(&s.attrs) => {
                    if !has_ident(&list_idents(&s.attrs, "derive"), "Model") {
                        continue;
                    }
                    let model_attr = list_idents(&s.attrs, "model");
                    if has_ident(&model_attr, "no_typed_columns") {
                        continue;
                    }
                    let name = s.ident.to_string();
                    let table =
                        string_value(&model_attr, "table").unwrap_or_else(|| name.to_snake_case());
                    let Fields::Named(fields) = &s.fields else {
                        continue;
                    };
                    let mut key = String::new();
                    let mut columns = Vec::new();
                    for f in &fields.named {
                        let Some(ident) = &f.ident else { continue };
                        if has_ident(&list_idents(&f.attrs, "model"), "skip") {
                            continue;
                        }
                        let field = ident.to_string();
                        let ty = tidy(&f.ty.to_token_stream().to_string());
                        if field == "id" {
                            key = ty.clone();
                        }
                        let doc = doc_text(&f.attrs);
                        columns.push(Column {
                            konst: field.to_shouty_snake_case(),
                            column: field,
                            ty,
                            doc: doc.lines().next().unwrap_or("").trim().to_owned(),
                        });
                    }
                    out.push(Model {
                        name,
                        table,
                        key,
                        doc: first_sentence(&doc_text(&s.attrs)),
                        columns,
                    });
                }
                _ => {}
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

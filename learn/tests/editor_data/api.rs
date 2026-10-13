//! Renox's public API, read from its source with `syn` (#529): every item
//! reachable under `renox::`, its members, signatures and first doc
//! sentence, for the editor's autocomplete (`editor/data/renox-api.json`).
//!
//! The walk: renox-core's module tree from `src/lib.rs` (following `mod x;`
//! to `x.rs` or `x/mod.rs`), every item indexed by its defining module, then
//! the public paths worked out from `pub mod`s and `pub use`s (resolved
//! through `crate::`, `self::`, `super::`, globs and chains), then the facade
//! (`crates/renox/src/lib.rs`: `pub use renox_core::*`, the macros, the
//! prelude). Members come from inherent impls, trait items, enum variants and
//! `macro_rules!` method templates (`method!(get, post, …)`).

use super::tidy::tidy;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use syn::{
    Attribute, Fields, ImplItem, Item, Meta, ReturnType, TraitItem, Type, UseTree, Visibility,
};

/// The JSON file.
#[derive(Serialize)]
pub struct Api {
    pub version: u32,
    pub renox: String,
    pub items: Vec<ApiItem>,
}

/// One item under `renox::`.
#[derive(Serialize, Clone, Debug)]
pub struct ApiItem {
    pub path: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub prelude: bool,
    pub doc: String,
    pub members: Vec<Member>,
}

/// A member: a method, an associated fn, const or type, or a variant.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub kind: String,
    pub sig: String,
    pub doc: String,
    pub returns: Option<String>,
}

/// An item defined in renox-core.
struct Def {
    name: String,
    kind: &'static str,
    public: bool,
    /// `pub`, `pub(crate)` or another restriction: what a glob import sees.
    visible: bool,
    hidden: bool,
    doc: String,
    members: Vec<Member>,
    prelude: bool,
    path: Option<String>,
}

/// A `use` declaration, flattened: `alias` is `*` for a glob.
struct Use {
    alias: String,
    path: Vec<String>,
    public: bool,
    hidden: bool,
}

/// A `macro_rules!` whose transcriber defines one `pub fn $name`.
struct Template {
    /// The `$name` variable, without `$`.
    var: String,
    /// The doc written in the transcriber, if any.
    doc: String,
    /// The tokens from `pub` to the body (or `where`).
    sig: Vec<TokenTree>,
}

#[derive(Default)]
struct Module {
    parent: Option<String>,
    public: bool,
    hidden: bool,
    doc: String,
    defs: Vec<usize>,
    uses: Vec<Use>,
    children: BTreeMap<String, String>,
    impls: Vec<syn::ItemImpl>,
    templates: BTreeMap<String, Template>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Def(usize),
    Module(String),
    External(String),
}

struct Crate {
    modules: BTreeMap<String, Module>,
    defs: Vec<Def>,
}

fn join(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}::{name}")
    }
}

/// Whether `attrs`' `#[cfg]`s hold for the default build: `fake`, `http` and
/// `server-events` on, every other feature off, `test`/`doctest` off, and
/// any other predicate on.
pub fn cfg_enabled(attrs: &[Attribute]) -> bool {
    attrs.iter().all(|attr| match &attr.meta {
        Meta::List(list) if list.path.is_ident("cfg") => {
            eval_cfg(&list.tokens.clone().into_iter().collect::<Vec<_>>())
        }
        _ => true,
    })
}

fn eval_cfg(tokens: &[TokenTree]) -> bool {
    match tokens {
        [
            TokenTree::Ident(i),
            TokenTree::Punct(eq),
            TokenTree::Literal(lit),
        ] if i == "feature" && eq.as_char() == '=' => {
            let value = lit.to_string();
            let value = value.trim_matches('"');
            matches!(value, "fake" | "http" | "server-events")
        }
        [TokenTree::Ident(i)] if i == "test" || i == "doctest" => false,
        [TokenTree::Ident(i), TokenTree::Group(g)] if g.delimiter() == Delimiter::Parenthesis => {
            let parts = split_commas(g.stream());
            match i.to_string().as_str() {
                "any" => parts.iter().any(|p| eval_cfg(p)),
                "all" => parts.iter().all(|p| eval_cfg(p)),
                "not" => !parts.first().is_some_and(|p| eval_cfg(p)),
                _ => true,
            }
        }
        _ => true,
    }
}

/// `tokens` split on top-level commas (empty parts dropped).
fn split_commas(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut parts = vec![Vec::new()];
    for t in tokens {
        match &t {
            TokenTree::Punct(p) if p.as_char() == ',' => parts.push(Vec::new()),
            _ => parts.last_mut().unwrap().push(t),
        }
    }
    parts.retain(|p| !p.is_empty());
    parts
}

/// `#[doc(hidden)]`.
pub fn is_hidden(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        Meta::List(list) if list.path.is_ident("doc") => list
            .tokens
            .clone()
            .into_iter()
            .any(|t| matches!(&t, TokenTree::Ident(i) if i == "hidden")),
        _ => false,
    })
}

/// The doc comment's text, one line per `///`.
pub fn doc_text(attrs: &[Attribute]) -> String {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(nv) if nv.path.is_ident("doc") => {
                syn::parse2::<syn::LitStr>(nv.value.to_token_stream())
                    .ok()
                    .map(|s| s.value())
            }
            _ => None,
        })
        .collect();
    lines
        .iter()
        .map(|l| l.strip_prefix(' ').unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first sentence of the doc's first paragraph, Markdown kept, at most
/// 160 characters.
pub fn first_sentence(doc: &str) -> String {
    let paragraph = doc
        .split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty())
        .unwrap_or("");
    let text = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    // The first ". " outside backticks ends the sentence.
    let mut in_code = false;
    let mut end = text.len();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        match c {
            '`' => in_code = !in_code,
            '.' if !in_code && chars.get(k + 1).is_some_and(|&(_, n)| n == ' ') => {
                // Not "e.g." or "i.e.".
                let before = &text[..i];
                if !(before.ends_with("e.g") || before.ends_with("i.e") || before.ends_with("etc"))
                {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let sentence = &text[..end];
    if sentence.chars().count() > 160 {
        let cut: String = sentence.chars().take(159).collect();
        format!("{}…", cut.trim_end())
    } else {
        sentence.to_owned()
    }
}

fn flatten_use(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    public: bool,
    hidden: bool,
    out: &mut Vec<Use>,
) {
    match tree {
        UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten_use(&p.tree, prefix, public, hidden, out);
            prefix.pop();
        }
        UseTree::Name(n) => {
            let name = n.ident.to_string();
            if name == "self" {
                if let Some(last) = prefix.last() {
                    out.push(Use {
                        alias: last.clone(),
                        path: prefix.clone(),
                        public,
                        hidden,
                    });
                }
            } else {
                let mut path = prefix.clone();
                path.push(name.clone());
                out.push(Use {
                    alias: name,
                    path,
                    public,
                    hidden,
                });
            }
        }
        UseTree::Rename(r) => {
            let alias = r.rename.to_string();
            if alias == "_" {
                return;
            }
            let mut path = prefix.clone();
            let name = r.ident.to_string();
            if name != "self" {
                path.push(name);
            }
            out.push(Use {
                alias,
                path,
                public,
                hidden,
            });
        }
        UseTree::Glob(_) => out.push(Use {
            alias: "*".into(),
            path: prefix.clone(),
            public,
            hidden,
        }),
        UseTree::Group(g) => {
            for t in &g.items {
                flatten_use(t, prefix, public, hidden, out);
            }
        }
    }
}

fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn is_visible(vis: &Visibility) -> bool {
    !matches!(vis, Visibility::Inherited)
}

/// Every `#[path = "…"]`.
fn path_attr(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| match &attr.meta {
        Meta::NameValue(nv) if nv.path.is_ident("path") => {
            syn::parse2::<syn::LitStr>(nv.value.to_token_stream())
                .ok()
                .map(|s| s.value())
        }
        _ => None,
    })
}

impl Crate {
    /// Reads renox-core from `src` (its `lib.rs` and every module file).
    fn load(src: &Path) -> Crate {
        let mut krate = Crate {
            modules: BTreeMap::new(),
            defs: Vec::new(),
        };
        let file = read(&src.join("lib.rs"));
        let doc = first_sentence(&doc_text(&file.attrs));
        krate.add_module(
            String::new(),
            None,
            true,
            false,
            doc,
            file.items,
            src.to_path_buf(),
        );
        krate
    }

    #[allow(clippy::too_many_arguments)]
    fn add_module(
        &mut self,
        path: String,
        parent: Option<String>,
        public: bool,
        hidden: bool,
        doc: String,
        items: Vec<Item>,
        dir: PathBuf,
    ) {
        self.modules.insert(
            path.clone(),
            Module {
                parent,
                public,
                hidden,
                doc,
                ..Module::default()
            },
        );
        for item in items {
            self.add_item(&path, item, &dir);
        }
    }

    fn def(
        &mut self,
        module: &str,
        name: String,
        kind: &'static str,
        vis: &Visibility,
        attrs: &[Attribute],
    ) -> usize {
        let i = self.defs.len();
        self.defs.push(Def {
            name,
            kind,
            public: is_pub(vis),
            visible: is_visible(vis),
            hidden: is_hidden(attrs),
            doc: first_sentence(&doc_text(attrs)),
            members: Vec::new(),
            prelude: false,
            path: None,
        });
        self.modules.get_mut(module).unwrap().defs.push(i);
        i
    }

    fn add_item(&mut self, module: &str, item: Item, dir: &Path) {
        let attrs = match &item {
            Item::Const(i) => &i.attrs,
            Item::Enum(i) => &i.attrs,
            Item::Fn(i) => &i.attrs,
            Item::Impl(i) => &i.attrs,
            Item::Macro(i) => &i.attrs,
            Item::Mod(i) => &i.attrs,
            Item::Static(i) => &i.attrs,
            Item::Struct(i) => &i.attrs,
            Item::Trait(i) => &i.attrs,
            Item::Type(i) => &i.attrs,
            Item::Use(i) => &i.attrs,
            _ => return,
        };
        if !cfg_enabled(attrs) {
            return;
        }
        match item {
            Item::Mod(m) => {
                let name = m.ident.to_string();
                let child = join(module, &name);
                let hidden = is_hidden(&m.attrs) || self.modules[module].hidden;
                let outer = first_sentence(&doc_text(&m.attrs));
                let (items, inner, child_dir) = match m.content {
                    Some((_, items)) => (items, String::new(), dir.join(&name)),
                    None => {
                        let file = match path_attr(&m.attrs) {
                            Some(p) => dir.join(p),
                            None if dir.join(format!("{name}.rs")).is_file() => {
                                dir.join(format!("{name}.rs"))
                            }
                            None => dir.join(&name).join("mod.rs"),
                        };
                        let parsed = read(&file);
                        let child_dir = if file.file_name().is_some_and(|f| f == "mod.rs") {
                            file.parent().unwrap().to_path_buf()
                        } else {
                            file.with_extension("")
                        };
                        (
                            parsed.items,
                            first_sentence(&doc_text(&parsed.attrs)),
                            child_dir,
                        )
                    }
                };
                let doc = if outer.is_empty() { inner } else { outer };
                self.modules
                    .get_mut(module)
                    .unwrap()
                    .children
                    .insert(name, child.clone());
                self.add_module(
                    child,
                    Some(module.to_owned()),
                    is_pub(&m.vis),
                    hidden,
                    doc,
                    items,
                    child_dir,
                );
            }
            Item::Struct(s) => {
                self.def(module, s.ident.to_string(), "struct", &s.vis, &s.attrs);
            }
            Item::Enum(e) => {
                let i = self.def(module, e.ident.to_string(), "enum", &e.vis, &e.attrs);
                let name = e.ident.to_string();
                for v in &e.variants {
                    if is_hidden(&v.attrs) || !cfg_enabled(&v.attrs) {
                        continue;
                    }
                    let mut variant = v.clone();
                    variant.attrs.clear();
                    variant.discriminant = None;
                    if let Fields::Named(_) = variant.fields {
                        // Field names are noise in a completion's detail.
                    }
                    self.defs[i].members.push(Member {
                        name: v.ident.to_string(),
                        kind: "variant".into(),
                        sig: tidy(&variant.to_token_stream().to_string()),
                        doc: first_sentence(&doc_text(&v.attrs)),
                        returns: Some(name.clone()),
                    });
                }
            }
            Item::Trait(t) => {
                let i = self.def(module, t.ident.to_string(), "trait", &t.vis, &t.attrs);
                let generics = generic_names(&t.generics);
                for item in &t.items {
                    if let Some(m) = trait_member(item, &generics) {
                        self.defs[i].members.push(m);
                    }
                }
            }
            Item::Fn(f) => {
                if !f.sig.ident.to_string().starts_with("__") {
                    let i = self.def(module, f.sig.ident.to_string(), "fn", &f.vis, &f.attrs);
                    let _ = i;
                }
            }
            Item::Const(c) => {
                self.def(module, c.ident.to_string(), "const", &c.vis, &c.attrs);
            }
            Item::Static(s) => {
                self.def(module, s.ident.to_string(), "static", &s.vis, &s.attrs);
            }
            Item::Type(t) => {
                self.def(module, t.ident.to_string(), "type", &t.vis, &t.attrs);
            }
            Item::Use(u) => {
                let mut out = Vec::new();
                flatten_use(
                    &u.tree,
                    &mut Vec::new(),
                    is_pub(&u.vis),
                    is_hidden(&u.attrs),
                    &mut out,
                );
                self.modules.get_mut(module).unwrap().uses.extend(out);
            }
            Item::Impl(imp) => {
                if imp.trait_.is_none() {
                    self.modules.get_mut(module).unwrap().impls.push(imp);
                }
            }
            Item::Macro(m) => {
                if m.mac.path.is_ident("macro_rules") {
                    if let Some(name) = &m.ident
                        && let Some(t) = template(m.mac.tokens.clone())
                    {
                        self.modules
                            .get_mut(module)
                            .unwrap()
                            .templates
                            .insert(name.to_string(), t);
                    }
                } else {
                    self.macro_enum(module, m.mac.tokens.clone());
                }
            }
            _ => {}
        }
    }

    /// An item macro whose input declares `pub enum Name … { Variant = …, }`
    /// (`setting_enum!`): the enum and its variants, with their docs.
    fn macro_enum(&mut self, module: &str, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        let is = |j: usize, s: &str| matches!(tokens.get(j), Some(TokenTree::Ident(x)) if x == s);
        let Some(at) = (0..tokens.len()).find(|&j| is(j, "pub") && is(j + 1, "enum")) else {
            return;
        };
        let Some(TokenTree::Ident(name)) = tokens.get(at + 2) else {
            return;
        };
        let Some(TokenTree::Group(body)) = tokens[at + 3..]
            .iter()
            .find(|t| matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace))
        else {
            return;
        };
        let doc = docs_before(&tokens, at);
        let i = self.defs.len();
        self.defs.push(Def {
            name: name.to_string(),
            kind: "enum",
            public: true,
            visible: true,
            hidden: false,
            doc: first_sentence(&doc),
            members: Vec::new(),
            prelude: false,
            path: None,
        });
        self.modules.get_mut(module).unwrap().defs.push(i);
        for variant in split_commas(body.stream()) {
            let Some(k) = variant
                .iter()
                .position(|t| matches!(t, TokenTree::Ident(_)))
            else {
                continue;
            };
            let TokenTree::Ident(v) = &variant[k] else {
                continue;
            };
            self.defs[i].members.push(Member {
                name: v.to_string(),
                kind: "variant".into(),
                sig: v.to_string(),
                doc: first_sentence(&docs_before(&variant, k)),
                returns: Some(name.to_string()),
            });
        }
    }

    /// What `name` means inside `module`: a child module, an item, an import.
    fn lookup(&self, module: &str, name: &str, depth: usize) -> Vec<Target> {
        if depth > 24 {
            return Vec::new();
        }
        let Some(m) = self.modules.get(module) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(child) = m.children.get(name) {
            out.push(Target::Module(child.clone()));
        }
        out.extend(
            m.defs
                .iter()
                .filter(|&&i| self.defs[i].name == name)
                .map(|&i| Target::Def(i)),
        );
        for u in m.uses.iter().filter(|u| u.alias == name) {
            out.extend(self.resolve(module, &u.path, depth + 1));
        }
        if out.is_empty() {
            for u in m.uses.iter().filter(|u| u.alias == "*") {
                for t in self.resolve(module, &u.path, depth + 1) {
                    if let Target::Module(source) = t {
                        out.extend(self.lookup_visible(&source, name, depth + 1));
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// `name` as a glob import of `module` sees it: items with a `pub`
    /// (of any reach), child modules and imports.
    fn lookup_visible(&self, module: &str, name: &str, depth: usize) -> Vec<Target> {
        self.lookup(module, name, depth)
            .into_iter()
            .filter(|t| match t {
                Target::Def(i) => self.defs[*i].visible,
                _ => true,
            })
            .collect()
    }

    /// A `use` path, from `module`.
    fn resolve(&self, module: &str, path: &[String], depth: usize) -> Vec<Target> {
        let Some((first, rest)) = path.split_first() else {
            return Vec::new();
        };
        let mut current = match first.as_str() {
            "crate" => vec![Target::Module(String::new())],
            "self" => vec![Target::Module(module.to_owned())],
            "super" => vec![Target::Module(
                self.modules[module].parent.clone().unwrap_or_default(),
            )],
            name => {
                let found = self.lookup(module, name, depth);
                if found.is_empty() {
                    vec![Target::External(name.to_owned())]
                } else {
                    found
                }
            }
        };
        for seg in rest {
            let mut next = Vec::new();
            for t in &current {
                match t {
                    Target::Module(m) if seg == "super" => next.push(Target::Module(
                        self.modules[m].parent.clone().unwrap_or_default(),
                    )),
                    Target::Module(m) => next.extend(self.lookup(m, seg, depth + 1)),
                    Target::External(e) => next.push(Target::External(format!("{e}::{seg}"))),
                    Target::Def(_) => {}
                }
            }
            current = next;
        }
        current
    }

    /// What `module` makes public: `pub` items and modules and `pub use`s.
    fn exports(&self, module: &str, depth: usize) -> BTreeMap<String, Vec<Target>> {
        let mut out: BTreeMap<String, Vec<Target>> = BTreeMap::new();
        if depth > 8 {
            return out;
        }
        let m = &self.modules[module];
        for (name, child) in &m.children {
            if self.modules[child].public && !self.modules[child].hidden {
                out.entry(name.clone())
                    .or_default()
                    .push(Target::Module(child.clone()));
            }
        }
        for &i in &m.defs {
            let d = &self.defs[i];
            if d.public && !d.hidden && !d.name.starts_with("__") {
                out.entry(d.name.clone()).or_default().push(Target::Def(i));
            }
        }
        for u in m
            .uses
            .iter()
            .filter(|u| u.public && !u.hidden && u.alias != "*")
        {
            if u.alias.starts_with("__") {
                continue;
            }
            let targets: Vec<_> = self
                .resolve(module, &u.path, 0)
                .into_iter()
                .filter(|t| match t {
                    Target::Def(i) => !self.defs[*i].hidden,
                    Target::Module(p) => !self.modules[p].hidden,
                    Target::External(_) => true,
                })
                .collect();
            out.entry(u.alias.clone()).or_default().extend(targets);
        }
        for u in m
            .uses
            .iter()
            .filter(|u| u.public && !u.hidden && u.alias == "*")
        {
            for t in self.resolve(module, &u.path, 0) {
                if let Target::Module(source) = t {
                    for (name, targets) in self.exports(&source, depth + 1) {
                        out.entry(name).or_insert(targets);
                    }
                }
            }
        }
        for targets in out.values_mut() {
            targets.sort();
            targets.dedup();
        }
        out.retain(|_, t| !t.is_empty());
        out
    }
}

fn read(path: &Path) -> syn::File {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    syn::parse_file(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn generic_names(generics: &syn::Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(t.ident.to_string()),
            _ => None,
        })
        .collect()
}

/// Names whose type the chain rule can't follow further.
const STD: &[&str] = &[
    "Vec",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "Arc",
    "Rc",
    "Cow",
    "Duration",
    "Instant",
    "SystemTime",
    "PathBuf",
    "Pin",
    "Cell",
    "RefCell",
    "Mutex",
    "RwLock",
    "Box",
    "Ordering",
    "Range",
    "Infallible",
    "bool",
    "char",
    "str",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "f32",
    "f64",
    "Value",
];

/// The type a call returns, for the chain rule: futures, `Result`, `Option`
/// and references unwrapped, `Self` as `self_name`, the last segment without
/// generics; `None` for a generic parameter, a tuple, `()` or a std type.
fn returns(output: &ReturnType, self_name: &str, generics: &[String]) -> Option<String> {
    match output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => unwrap_type(ty, self_name, generics),
    }
}

fn unwrap_type(ty: &Type, self_name: &str, generics: &[String]) -> Option<String> {
    match ty {
        Type::Reference(r) => unwrap_type(&r.elem, self_name, generics),
        Type::Paren(p) => unwrap_type(&p.elem, self_name, generics),
        Type::Group(g) => unwrap_type(&g.elem, self_name, generics),
        Type::ImplTrait(t) => t.bounds.iter().find_map(|b| match b {
            syn::TypeParamBound::Trait(tb) => {
                let seg = tb.path.segments.last()?;
                if seg.ident != "Future" && seg.ident != "IntoFuture" {
                    return None;
                }
                let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
                    return None;
                };
                args.args.iter().find_map(|a| match a {
                    syn::GenericArgument::AssocType(at) if at.ident == "Output" => {
                        unwrap_type(&at.ty, self_name, generics)
                    }
                    _ => None,
                })
            }
            _ => None,
        }),
        Type::Path(p) if p.qself.is_none() => {
            let seg = p.path.segments.last()?;
            let name = seg.ident.to_string();
            if name == "Result" || name == "Option" {
                let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
                    return None;
                };
                return args
                    .args
                    .iter()
                    .find_map(|a| match a {
                        syn::GenericArgument::Type(t) => Some(t),
                        _ => None,
                    })
                    .and_then(|t| unwrap_type(t, self_name, generics));
            }
            if name == "Self" {
                return Some(self_name.to_owned());
            }
            if p.path.segments.len() == 1 && generics.contains(&name) {
                return None;
            }
            if STD.contains(&name.as_str()) {
                return None;
            }
            Some(name)
        }
        _ => None,
    }
}

/// A signature without its `where` clause, as text.
fn sig_text(prefix: &str, sig: &syn::Signature) -> String {
    let mut sig = sig.clone();
    sig.generics.where_clause = None;
    tidy(&format!("{prefix} {}", sig.to_token_stream()))
}

fn has_receiver(sig: &syn::Signature) -> bool {
    sig.inputs
        .iter()
        .any(|a| matches!(a, syn::FnArg::Receiver(_)))
}

fn trait_member(item: &TraitItem, generics: &[String]) -> Option<Member> {
    match item {
        TraitItem::Fn(f) => {
            if is_hidden(&f.attrs)
                || !cfg_enabled(&f.attrs)
                || f.sig.ident.to_string().starts_with("__")
            {
                return None;
            }
            let mut g = generics.to_vec();
            g.extend(generic_names(&f.sig.generics));
            Some(Member {
                name: f.sig.ident.to_string(),
                kind: if has_receiver(&f.sig) { "method" } else { "fn" }.into(),
                sig: sig_text("", &f.sig),
                doc: first_sentence(&doc_text(&f.attrs)),
                returns: returns(&f.sig.output, "Self", &g),
            })
        }
        TraitItem::Const(c) => {
            if is_hidden(&c.attrs) || !cfg_enabled(&c.attrs) {
                return None;
            }
            Some(Member {
                name: c.ident.to_string(),
                kind: "const".into(),
                sig: tidy(&format!("const {}: {}", c.ident, c.ty.to_token_stream())),
                doc: first_sentence(&doc_text(&c.attrs)),
                returns: unwrap_type(&c.ty, "Self", generics),
            })
        }
        TraitItem::Type(t) => {
            if is_hidden(&t.attrs) || !cfg_enabled(&t.attrs) {
                return None;
            }
            Some(Member {
                name: t.ident.to_string(),
                kind: "type".into(),
                sig: format!("type {}", t.ident),
                doc: first_sentence(&doc_text(&t.attrs)),
                returns: None,
            })
        }
        _ => None,
    }
}

/// A `pub fn` of an inherent impl, as a member.
fn impl_fn_member(
    f: &syn::ImplItemFn,
    self_name: &str,
    generics: &[String],
    doc: Option<String>,
) -> Option<Member> {
    if !is_pub(&f.vis)
        || is_hidden(&f.attrs)
        || !cfg_enabled(&f.attrs)
        || f.sig.ident.to_string().starts_with("__")
    {
        return None;
    }
    let mut g = generics.to_vec();
    g.extend(generic_names(&f.sig.generics));
    Some(Member {
        name: f.sig.ident.to_string(),
        kind: if has_receiver(&f.sig) { "method" } else { "fn" }.into(),
        sig: sig_text("pub", &f.sig),
        doc: doc.unwrap_or_else(|| first_sentence(&doc_text(&f.attrs))),
        returns: returns(&f.sig.output, self_name, &g),
    })
}

/// The doc text of `# [doc = "…"]` tokens.
fn doc_of_tokens(group: &proc_macro2::Group) -> Option<String> {
    let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
    match tokens.as_slice() {
        [
            TokenTree::Ident(i),
            TokenTree::Punct(eq),
            TokenTree::Literal(lit),
        ] if i == "doc" && eq.as_char() == '=' => {
            let text = syn::parse2::<syn::LitStr>(TokenTree::Literal(lit.clone()).into())
                .ok()?
                .value();
            Some(text.strip_prefix(' ').unwrap_or(&text).to_owned())
        }
        _ => None,
    }
}

/// The `# [doc = "…"]` lines right before `tokens[at]`, as text.
fn docs_before(tokens: &[TokenTree], at: usize) -> String {
    let mut doc = Vec::new();
    let mut j = at;
    while j >= 2 {
        match (&tokens[j - 2], &tokens[j - 1]) {
            (TokenTree::Punct(p), TokenTree::Group(g)) if p.as_char() == '#' => {
                if let Some(line) = doc_of_tokens(g) {
                    doc.push(line);
                }
                j -= 2;
            }
            _ => break,
        }
    }
    doc.reverse();
    doc.join("\n")
}

/// A `macro_rules!` body's method template, if its transcriber has exactly
/// one `pub fn $name`.
fn template(body: TokenStream) -> Option<Template> {
    let tokens: Vec<TokenTree> = body.into_iter().collect();
    // Rules: `(matcher) => {transcriber}` separated by `;`.
    let mut found = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Group(g) = &tokens[i]
            && i >= 2
            && matches!(&tokens[i - 1], TokenTree::Punct(p) if p.as_char() == '>')
        {
            find_pub_fn(g.stream(), &mut found);
        }
        i += 1;
    }
    if found.len() != 1 {
        return None;
    }
    found.pop()
}

fn find_pub_fn(stream: TokenStream, found: &mut Vec<Template>) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    for (i, t) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = t {
            find_pub_fn(g.stream(), found);
        }
        let is = |j: usize, s: &str| matches!(tokens.get(j), Some(TokenTree::Ident(x)) if x == s);
        let dollar = matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '$');
        if is(i, "pub") && is(i + 1, "fn") && dollar {
            let Some(TokenTree::Ident(var)) = tokens.get(i + 3) else {
                continue;
            };
            // The doc lines right before it, written in the transcriber.
            let doc = docs_before(&tokens, i);
            let sig: Vec<TokenTree> = tokens[i..]
                .iter()
                .take_while(|t| {
                    !matches!(t, TokenTree::Ident(x) if x == "where")
                        && !matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace)
                })
                .cloned()
                .collect();
            found.push(Template {
                var: var.to_string(),
                doc: first_sentence(&doc),
                sig,
            });
        }
    }
}

/// The members a template invocation (`method!(get, post)`) defines.
fn expand(
    template: &Template,
    tokens: TokenStream,
    self_name: &str,
    generics: &[String],
) -> Vec<Member> {
    let mut out = Vec::new();
    for segment in split_commas(tokens) {
        let mut doc = Vec::new();
        let mut name = None;
        let mut k = 0;
        while k < segment.len() {
            match (&segment[k], segment.get(k + 1)) {
                (TokenTree::Punct(p), Some(TokenTree::Group(g))) if p.as_char() == '#' => {
                    if let Some(line) = doc_of_tokens(g) {
                        doc.push(line);
                    }
                    k += 2;
                }
                (TokenTree::Ident(i), _) => {
                    name = Some(i.to_string());
                    break;
                }
                _ => k += 1,
            }
        }
        let Some(name) = name else { continue };
        // The template's signature with `$var` replaced by the name.
        let mut sig = TokenStream::new();
        let mut k = 0;
        while k < template.sig.len() {
            match (&template.sig[k], template.sig.get(k + 1)) {
                (TokenTree::Punct(p), Some(TokenTree::Ident(v)))
                    if p.as_char() == '$' && *v == template.var =>
                {
                    sig.extend([TokenTree::Ident(proc_macro2::Ident::new(
                        &name,
                        proc_macro2::Span::call_site(),
                    ))]);
                    k += 2;
                }
                (t, _) => {
                    sig.extend([t.clone()]);
                    k += 1;
                }
            }
        }
        let doc = if doc.is_empty() {
            template.doc.clone()
        } else {
            first_sentence(&doc.join("\n"))
        };
        let text = format!("{sig} {{}}");
        if let Ok(f) = syn::parse_str::<syn::ImplItemFn>(&text)
            && let Some(m) = impl_fn_member(&f, self_name, generics, Some(doc))
        {
            out.push(m);
        }
    }
    out
}

/// The facade crate's (`crates/renox/src/lib.rs`) `pub use renox_macros::{…}`
/// items, with their kind and doc from renox-macros.
fn macros(macros_src: &Path) -> BTreeMap<String, (String, String)> {
    let file = read(&macros_src.join("lib.rs"));
    let mut out = BTreeMap::new();
    for item in file.items {
        let Item::Fn(f) = item else { continue };
        let doc = first_sentence(&doc_text(&f.attrs));
        for attr in &f.attrs {
            let (kind, name) = match &attr.meta {
                Meta::List(list) if list.path.is_ident("proc_macro_derive") => {
                    let Some(TokenTree::Ident(name)) = list.tokens.clone().into_iter().next()
                    else {
                        continue;
                    };
                    ("derive", name.to_string())
                }
                Meta::Path(p) if p.is_ident("proc_macro") => ("macro", f.sig.ident.to_string()),
                Meta::Path(p) if p.is_ident("proc_macro_attribute") => {
                    ("attribute", f.sig.ident.to_string())
                }
                _ => continue,
            };
            out.insert(name, (kind.to_owned(), doc.clone()));
        }
    }
    out
}

/// Renox's API from the source under `root` (the repository).
pub fn extract(root: &Path) -> Api {
    let mut krate = Crate::load(&root.join("crates/renox-core/src"));

    // Members of inherent impls, attached to the type they name.
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, d) in krate.defs.iter().enumerate() {
        if matches!(d.kind, "struct" | "enum" | "type") {
            by_name.entry(d.name.clone()).or_default().push(i);
        }
    }
    let mut attach: Vec<(usize, Member)> = Vec::new();
    for (module, m) in &krate.modules {
        for imp in &m.impls {
            let Type::Path(p) = &*imp.self_ty else {
                continue;
            };
            let Some(seg) = p.path.segments.last() else {
                continue;
            };
            let name = seg.ident.to_string();
            let mut targets: Vec<usize> = krate
                .lookup(module, &name, 0)
                .into_iter()
                .filter_map(|t| match t {
                    Target::Def(i) if matches!(krate.defs[i].kind, "struct" | "enum" | "type") => {
                        Some(i)
                    }
                    _ => None,
                })
                .collect();
            if targets.is_empty() {
                targets = by_name.get(&name).cloned().unwrap_or_default();
            }
            if targets.len() > 1 {
                eprintln!(
                    "editor_data: `impl {name}` in {module} matches {} types; attached to each",
                    targets.len()
                );
            }
            let generics = generic_names(&imp.generics);
            for item in &imp.items {
                let members = match item {
                    ImplItem::Fn(f) => impl_fn_member(f, &name, &generics, None)
                        .into_iter()
                        .collect(),
                    ImplItem::Const(c)
                        if is_pub(&c.vis) && !is_hidden(&c.attrs) && cfg_enabled(&c.attrs) =>
                    {
                        vec![Member {
                            name: c.ident.to_string(),
                            kind: "const".into(),
                            sig: tidy(&format!(
                                "pub const {}: {}",
                                c.ident,
                                c.ty.to_token_stream()
                            )),
                            doc: first_sentence(&doc_text(&c.attrs)),
                            returns: unwrap_type(&c.ty, &name, &generics),
                        }]
                    }
                    ImplItem::Macro(mac) if cfg_enabled(&mac.attrs) => {
                        let Some(macro_name) = mac.mac.path.get_ident().map(|i| i.to_string())
                        else {
                            continue;
                        };
                        // macro_rules! are textually scoped: this module or an ancestor.
                        let mut scope = Some(module.clone());
                        let mut found = None;
                        while let Some(s) = scope {
                            if let Some(t) = krate.modules[&s].templates.get(&macro_name) {
                                found = Some(t);
                                break;
                            }
                            scope = krate.modules[&s].parent.clone();
                        }
                        match found {
                            Some(t) => expand(t, mac.mac.tokens.clone(), &name, &generics),
                            None => Vec::new(),
                        }
                    }
                    _ => Vec::new(),
                };
                for target in &targets {
                    for m in &members {
                        attach.push((*target, m.clone()));
                    }
                }
            }
        }
    }
    for (i, m) in attach {
        krate.defs[i].members.push(m);
    }

    // Public paths: breadth first from the root, so the first path found is
    // the shortest.
    let mut items: Vec<ApiItem> = Vec::new();
    let mut module_paths: BTreeMap<String, String> = BTreeMap::new();
    let mut reexports: BTreeMap<String, ApiItem> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::from([(String::new(), "renox".to_owned())]);
    let mut seen = BTreeSet::from([String::new()]);
    while let Some((module, prefix)) = queue.pop_front() {
        for (name, targets) in krate.exports(&module, 0) {
            let path = format!("{prefix}::{name}");
            for t in targets {
                match t {
                    Target::Def(i) => {
                        let d = &mut krate.defs[i];
                        if d.path.is_none() {
                            d.path = Some(path.clone());
                        }
                    }
                    Target::Module(m) => {
                        if seen.insert(m.clone()) {
                            module_paths.insert(m.clone(), path.clone());
                            queue.push_back((m, path.clone()));
                        }
                    }
                    Target::External(from) => {
                        reexports.entry(path.clone()).or_insert(ApiItem {
                            path: path.clone(),
                            name: name.clone(),
                            kind: "reexport".into(),
                            from: Some(from),
                            prelude: false,
                            doc: String::new(),
                            members: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    // The facade: macros, `pub use axum;`, and the prelude.
    let facade = read(&root.join("crates/renox/src/lib.rs"));
    let macro_docs = macros(&root.join("crates/renox-macros/src"));
    let mut macro_items: BTreeMap<String, ApiItem> = BTreeMap::new();
    for item in &facade.items {
        match item {
            Item::Use(u) if is_pub(&u.vis) && cfg_enabled(&u.attrs) => {
                let mut uses = Vec::new();
                flatten_use(&u.tree, &mut Vec::new(), true, false, &mut uses);
                for u in uses {
                    match u.path.first().map(String::as_str) {
                        Some("renox_core") => {}
                        Some("renox_macros") => {
                            if let Some((kind, doc)) = macro_docs.get(&u.alias) {
                                macro_items.insert(
                                    u.alias.clone(),
                                    ApiItem {
                                        path: format!("renox::{}", u.alias),
                                        name: u.alias.clone(),
                                        kind: kind.clone(),
                                        from: None,
                                        prelude: false,
                                        doc: doc.clone(),
                                        members: Vec::new(),
                                    },
                                );
                            }
                        }
                        _ => {
                            let path = format!("renox::{}", u.alias);
                            reexports.entry(path.clone()).or_insert(ApiItem {
                                path,
                                name: u.alias.clone(),
                                kind: "reexport".into(),
                                from: Some(u.path.join("::")),
                                prelude: false,
                                doc: String::new(),
                                members: Vec::new(),
                            });
                        }
                    }
                }
            }
            Item::Mod(m) if m.ident == "prelude" => {
                let Some((_, prelude_items)) = &m.content else {
                    continue;
                };
                items.push(ApiItem {
                    path: "renox::prelude".into(),
                    name: "prelude".into(),
                    kind: "mod".into(),
                    from: None,
                    prelude: false,
                    doc: first_sentence(&doc_text(&m.attrs)),
                    members: Vec::new(),
                });
                for item in prelude_items {
                    let Item::Use(u) = item else { continue };
                    let mut uses = Vec::new();
                    flatten_use(&u.tree, &mut Vec::new(), true, false, &mut uses);
                    for u in uses {
                        match u.path.first().map(String::as_str) {
                            Some("renox_core") => {
                                for t in krate.resolve("", &u.path[1..], 0) {
                                    match t {
                                        Target::Def(i) => krate.defs[i].prelude = true,
                                        Target::External(from) => {
                                            mark_or_add(&mut reexports, &u.alias, from)
                                        }
                                        Target::Module(_) => {}
                                    }
                                }
                            }
                            Some("renox_macros") => {
                                if let Some(item) = macro_items.get_mut(&u.alias) {
                                    item.prelude = true;
                                }
                            }
                            _ => mark_or_add(&mut reexports, &u.alias, u.path.join("::")),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if std::env::var("EDITOR_DATA_DEBUG").is_ok() {
        for (module, m) in &krate.modules {
            for &i in &m.defs {
                let d = &krate.defs[i];
                if d.public && d.path.is_none() && !d.hidden {
                    eprintln!("unreached: {module}::{} ({})", d.name, d.kind);
                }
            }
        }
    }
    for d in &krate.defs {
        let Some(path) = &d.path else { continue };
        let mut members = d.members.clone();
        members.sort_by(|a, b| (&a.name, &a.sig).cmp(&(&b.name, &b.sig)));
        members.dedup_by(|a, b| a.name == b.name && a.sig == b.sig);
        items.push(ApiItem {
            path: path.clone(),
            name: d.name.clone(),
            kind: d.kind.into(),
            from: None,
            prelude: d.prelude,
            doc: d.doc.clone(),
            members,
        });
    }
    for (module, path) in &module_paths {
        items.push(ApiItem {
            path: path.clone(),
            name: path.rsplit("::").next().unwrap().to_owned(),
            kind: "mod".into(),
            from: None,
            prelude: false,
            doc: krate.modules[module].doc.clone(),
            members: Vec::new(),
        });
    }
    items.extend(reexports.into_values());
    items.extend(macro_items.into_values());
    items.sort_by(|a, b| (&a.path, &a.kind).cmp(&(&b.path, &b.kind)));
    items.dedup_by(|a, b| a.path == b.path && a.kind == b.kind);
    Api {
        version: 1,
        renox: env!("CARGO_PKG_VERSION").to_owned(),
        items,
    }
}

/// Marks the re-export of `from` as in the prelude, or adds it at
/// `renox::prelude::alias` when no other path has it.
fn mark_or_add(reexports: &mut BTreeMap<String, ApiItem>, alias: &str, from: String) {
    if let Some(item) = reexports
        .values_mut()
        .find(|i| i.from.as_deref() == Some(&from) && i.name == alias)
    {
        item.prelude = true;
        return;
    }
    let path = format!("renox::prelude::{alias}");
    reexports.insert(
        path.clone(),
        ApiItem {
            path,
            name: alias.to_owned(),
            kind: "reexport".into(),
            from: Some(from),
            prelude: true,
            doc: String::new(),
            members: Vec::new(),
        },
    );
}

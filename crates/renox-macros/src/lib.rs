//! Procedural macros for Renox. Use them through the `renox` crate:
//! `#[derive(Model, FromRow, DbEnum, Validate)]`, `renox::migrations!()`,
//! `renox::embedded!()` and `#[renox::test]`.
#![warn(missing_docs)]

mod db_enum;
mod embedded;
mod from_row;
mod migrations;
mod model;
mod validate;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Implements `renox::db::Model` for a struct with named fields.
///
/// ```
/// # use renox::prelude::*;
/// # use serde::Serialize;
/// #[derive(Model, Serialize, Default)]
/// #[model(table = "products", soft_deletes)]
/// struct Product {
///     id: i64,
///     name: String,
///     #[model(skip)]
///     label: String, // not a column; filled with Default when loading
///     created_at: Option<DateTime>,
///     updated_at: Option<DateTime>,
///     deleted_at: Option<DateTime>,
/// }
/// ```
///
/// - `table` defaults to the struct name in snake_case (no pluralisation).
/// - An `id` field is required; its type is the key (`i64`, `Ulid`, `Uuid` or
///   `String`, see `renox::db::ModelKey`).
/// - `created_at` / `updated_at` fields are filled on save.
/// - `soft_deletes` needs a `deleted_at: Option<DateTime>` field.
/// - `search = "title, body"` names the text columns full-text search looks
///   in (`Model::search`, `renox::db::search`), most important first;
///   `search_language = "simple"` changes the language from `english`.
/// - Every column also gets a typed constant named after the field in
///   SHOUTY_SNAKE_CASE (`Product::PRICE: Col<Product, i64>`), for
///   `Query::where_` and friends; `skip` fields get none. `no_typed_columns`
///   turns them off, for a struct with its own associated items of those names.
/// - `default_scope = "path::to::fn"` and `hooks`: see `renox::db::Model`.
/// - `form` also generates `<Model>Form`, with the model's visibility: every
///   field except `id`, `created_at`, `updated_at`, `deleted_at` and
///   `#[model(skip)]`/`#[form(skip)]` fields, in their own types, deriving
///   `Deserialize`, `Serialize` and `Validate` (so it works as `Valid<…>` and as
///   a renox-admin form). `#[form(validate(required, max = 20))]` on a field
///   becomes `#[validate(required, max = 20)]`; `bool` fields get
///   `#[serde(default)]`. No rules come from types. `<Model>Form::fill(self,
///   &mut Model)` copies the values onto a record. `#[form(…)]` without
///   `#[model(form)]` is a compile error.
#[proc_macro_derive(Model, attributes(model, form))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    model::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Implements `renox::Validate` from `#[validate(…)]` attributes on the
/// fields, for forms that need only rules:
///
/// ```
/// # use renox::prelude::*;
/// #[derive(serde::Deserialize, Validate)]
/// struct Signup {
///     #[validate(required, max = 100, label = "Full name")]
///     name: String,
///     #[validate(required, email, unique("users", "email"))]
///     email: String,
///     #[validate(required, min = 8, confirmed(&self.password_confirmation))]
///     password: String,
///     password_confirmation: String,
///     #[validate(max = 5, each(required, max = 20), distinct)]
///     tags: Vec<String>,
///     #[validate(rename = "t-shirt", one_of(&["S", "M", "L"]))]
///     size: String,
/// }
/// ```
///
/// Each item is a call on the field's rules, in order: `required` is
/// `.required()`, `max = 100` is `.max(100)`, `unique("users", "email")` is
/// `.unique("users", "email")`, so every rule of `renox::validation::Field`
/// works, and arguments may use `self`. Three are special: `each(…)` applies
/// rules to each item of a list (errors `tags.0`, …), `distinct` refuses
/// repeated items, and `rename = "…"` names the field as the form does. For
/// `prepare`, `authorize` and `after`, add `#[validate(hooks)]` on the struct
/// and `impl renox::validation::ValidateHooks`. Anything else: implement
/// `Validate` by hand.
#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    validate::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Implements `renox::db::FromRow`, so `sql(…).fetch_as::<T>()` can read
/// rows of any query (joins, aggregates, a few columns) into the struct.
///
/// ```
/// # use renox::prelude::*;
/// # use serde::Serialize;
/// #[derive(FromRow, Serialize)]
/// struct ProductRow {
///     id: i64,
///     name: String,
///     #[row(rename = "category_name")]
///     category: Option<String>,
///     #[row(skip)]
///     note: String, // Default
/// }
/// ```
///
/// `derive(Model)` implements `FromRow` too.
#[proc_macro_derive(FromRow, attributes(row))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    from_row::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// A fieldless enum stored as text: in the database (a `TEXT` column), in
/// forms (`<select>`), in JSON and in templates. Variants are stored in
/// snake_case (`OnHold` → `on_hold`) unless renamed with `#[db(rename = "…")]`.
///
/// Generates `as_str()`, `ALL` (every variant, e.g. for a `<select>`),
/// `Display`, `FromStr`, `Serialize`, `Deserialize`, `ToDbValue`, and
/// decoding on every database, so the enum can be a model field.
///
/// ```
/// # use renox::prelude::*;
/// #[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
/// enum Status {
///     #[default]
///     Draft,
///     Published,
///     #[db(rename = "hidden")]
///     Archived,
/// }
///
/// assert_eq!(Status::Archived.as_str(), "hidden");
/// assert_eq!("published".parse::<Status>().unwrap(), Status::Published);
/// assert_eq!(Status::ALL.len(), 3);
/// ```
#[proc_macro_derive(DbEnum, attributes(db))]
pub fn derive_db_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    db_enum::expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Embeds the SQL migrations of a directory (default `migrations`, relative
/// to the crate root) as `&'static [renox::db::Migration]`.
///
/// Files are `<timestamp>_<name>.up.sql` with an optional matching
/// `.down.sql`, or a plain `<timestamp>_<name>.sql` that can't be rolled back.
/// Add a `build.rs` with `println!("cargo:rerun-if-changed=migrations");` so
/// new files are picked up (`rnx new` creates it).
#[proc_macro]
pub fn migrations(input: TokenStream) -> TokenStream {
    migrations::expand(input.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Marks an async test, like `#[tokio::test]`, using the Tokio that Renox
/// re-exports, so apps don't need `tokio` as a dependency.
///
/// ```
/// # use renox::prelude::*;
/// # use renox::testing::TestApp;
/// #[renox::test]
/// async fn home_page() {
///     let app = TestApp::new(App::new()).await;
///     app.get("/health").await.assert_ok();
/// }
/// ```
#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = proc_macro2::TokenStream::from(attr);
    let item = proc_macro2::TokenStream::from(item);
    let extra = if attr.is_empty() {
        quote::quote! {}
    } else {
        quote::quote! { #attr, }
    };
    quote::quote! {
        #[::renox::tokio::test(#extra crate = "::renox::tokio")]
        #item
    }
    .into()
}

/// Embeds `resources/views`, `resources/lang` and `public` in the binary, so
/// a release build runs from a single file:
///
/// ```
/// # use renox::prelude::*;
/// # let _ =
/// App::new().embed(renox::embedded!())
/// # ;
/// ```
///
/// Files are read from disk while `APP_DEBUG` is on (templates reload), and
/// from the binary otherwise. Add `cargo:rerun-if-changed=resources` and
/// `=public` to `build.rs` so new files are picked up (`rnx new` does).
#[proc_macro]
pub fn embedded(input: TokenStream) -> TokenStream {
    embedded::expand(input.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

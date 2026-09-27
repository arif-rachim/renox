//! Procedural macros for Renox. Use them through the `renox` crate:
//! `renox::Model` and `renox::migrations!`.

mod db_enum;
mod embedded;
mod migrations;
mod model;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Implements `renox::db::Model` for a struct with named fields.
///
/// ```ignore
/// #[derive(Model, Serialize, Default)]
/// #[model(table = "produk", soft_deletes)]
/// struct Produk {
///     id: i64,
///     nama: String,
///     #[model(skip)]
///     label: String, // not a column; filled with Default when loading
///     created_at: Option<DateTime>,
///     updated_at: Option<DateTime>,
///     deleted_at: Option<DateTime>,
/// }
/// ```
///
/// - `table` defaults to the struct name in snake_case (no pluralisation).
/// - An `id: i64` field is required.
/// - `created_at` / `updated_at` fields are filled on save.
/// - `soft_deletes` needs a `deleted_at: Option<DateTime>` field.
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    model::expand(input)
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
/// ```ignore
/// #[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
/// enum Status {
///     #[default]
///     Draft,
///     Published,
///     #[db(rename = "hidden")]
///     Archived,
/// }
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
/// ```ignore
/// #[renox::test]
/// async fn home_page() {
///     let app = TestApp::new(toko::app()).await;
///     app.get("/").await.assert_ok();
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
/// ```ignore
/// App::new().embed(renox::embedded!())
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

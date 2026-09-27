//! Procedural macros for Renox. Use them through the `renox` crate:
//! `renox::Model` and `renox::migrations!`.

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

/// Embeds the SQL migrations of a directory (default `migrations`, relative
/// to the crate root) as `&'static [renox::db::Migration]`.
///
/// Files are `<timestamp>_<name>.up.sql` with an optional matching
/// `.down.sql`, or a plain `<timestamp>_<name>.sql` that can't be rolled back.
/// Add a `build.rs` with `println!("cargo:rerun-if-changed=migrations");` so
/// new files are picked up (`renox new` creates it).
#[proc_macro]
pub fn migrations(input: TokenStream) -> TokenStream {
    migrations::expand(input.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, LitStr, Result};

#[derive(Default)]
struct Files {
    up: Option<PathBuf>,
    down: Option<PathBuf>,
}

pub fn expand(input: TokenStream) -> Result<TokenStream> {
    let dir = if input.is_empty() {
        "migrations".to_owned()
    } else {
        syn::parse2::<LitStr>(input)?.value()
    };
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| Error::new(Span::call_site(), "CARGO_MANIFEST_DIR is not set"))?;
    let path = PathBuf::from(root).join(&dir);

    let mut migrations: BTreeMap<String, Files> = BTreeMap::new();
    if path.is_dir() {
        let entries = std::fs::read_dir(&path).map_err(|e| {
            Error::new(
                Span::call_site(),
                format!("reading {}: {e}", path.display()),
            )
        })?;
        for entry in entries.flatten() {
            let file = entry.path();
            let Some(file_name) = file.file_name().and_then(|n| n.to_str()).map(str::to_owned)
            else {
                continue;
            };
            if let Some(name) = file_name.strip_suffix(".down.sql") {
                migrations.entry(name.to_owned()).or_default().down = Some(file);
            } else if let Some(name) = file_name.strip_suffix(".up.sql") {
                migrations.entry(name.to_owned()).or_default().up = Some(file);
            } else if let Some(name) = file_name.strip_suffix(".sql") {
                migrations.entry(name.to_owned()).or_default().up = Some(file);
            }
        }
    }

    let mut items = Vec::new();
    for (name, files) in &migrations {
        let Some(up) = &files.up else {
            return Err(Error::new(
                Span::call_site(),
                format!("migration `{name}` has a .down.sql but no .up.sql in {dir}"),
            ));
        };
        let up = up.to_string_lossy().into_owned();
        let down = match &files.down {
            Some(down) => {
                let down = down.to_string_lossy().into_owned();
                quote! { ::core::option::Option::Some(::core::include_str!(#down)) }
            }
            None => quote! { ::core::option::Option::None },
        };
        items.push(quote! {
            ::renox::db::Migration { name: #name, up: ::core::include_str!(#up), down: #down }
        });
    }

    Ok(quote! {{
        const MIGRATIONS: &[::renox::db::Migration] = &[#(#items),*];
        MIGRATIONS
    }})
}

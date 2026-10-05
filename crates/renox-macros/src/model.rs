use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, LitStr, Result, Type};

struct Field {
    ident: syn::Ident,
    name: String,
    ty: Type,
    skip: bool,
}

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "Model can't be derived for generic structs",
        ));
    }

    let mut table = ident.to_string().to_snake_case();
    let mut soft_deletes = false;
    let mut default_scope: Option<syn::Path> = None;
    let mut hooks = false;
    let mut search: Option<LitStr> = None;
    let mut search_language: Option<LitStr> = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("model")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                table = meta.value()?.parse::<LitStr>()?.value();
                Ok(())
            } else if meta.path.is_ident("soft_deletes") {
                soft_deletes = true;
                Ok(())
            } else if meta.path.is_ident("hooks") {
                hooks = true;
                Ok(())
            } else if meta.path.is_ident("default_scope") {
                default_scope = Some(meta.value()?.parse::<LitStr>()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("search") {
                search = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("search_language") {
                search_language = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else {
                Err(meta.error(
                    "expected `table = \"...\"`, `soft_deletes`, `hooks`, `default_scope = \"path::to::fn\"`, `search = \"col, col\"` or `search_language = \"...\"`",
                ))
            }
        })?;
    }

    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(
            ident,
            "Model can only be derived for structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(Error::new_spanned(
            ident,
            "Model needs a struct with named fields",
        ));
    };

    let mut fields = Vec::new();
    for field in &named.named {
        let mut skip = false;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("model")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else {
                    Err(meta.error("expected `skip`"))
                }
            })?;
        }
        let field_ident = field.ident.clone().expect("named fields have identifiers");
        fields.push(Field {
            name: field_ident.to_string().trim_start_matches("r#").to_owned(),
            ident: field_ident,
            ty: field.ty.clone(),
            skip,
        });
    }

    let column = |name: &str| fields.iter().find(|f| f.name == name && !f.skip);
    let Some(id_field) = column("id") else {
        return Err(Error::new_spanned(
            ident,
            "a Model needs an `id` field (`i64`, `Ulid`, `Uuid` or `String`)",
        ));
    };
    let key_type = &id_field.ty;
    let deleted_at = column("deleted_at");
    if soft_deletes && deleted_at.is_none() {
        return Err(Error::new_spanned(
            ident,
            "`soft_deletes` needs a `deleted_at: Option<DateTime>` field",
        ));
    }

    let columns: Vec<&str> = fields
        .iter()
        .filter(|f| !f.skip)
        .map(|f| f.name.as_str())
        .collect();

    let mut searchable: Vec<String> = Vec::new();
    if let Some(lit) = &search {
        for name in lit.value().split(',').map(str::trim) {
            if name.is_empty() {
                continue;
            }
            if name == "id" || !columns.contains(&name) {
                return Err(Error::new_spanned(
                    lit,
                    format!("`search`: `{name}` isn't a text column of this model"),
                ));
            }
            if searchable.iter().any(|s| s == name) {
                return Err(Error::new_spanned(
                    lit,
                    format!("`search`: `{name}` is listed twice"),
                ));
            }
            searchable.push(name.to_owned());
        }
        if searchable.is_empty() {
            return Err(Error::new_spanned(
                lit,
                "`search` needs at least one column: `search = \"title, body\"`",
            ));
        }
    }
    let search_const = (!searchable.is_empty()).then(|| {
        quote! { const SEARCHABLE: &'static [&'static str] = &[#(#searchable),*]; }
    });
    let language_const = match &search_language {
        Some(lit) => {
            let language = lit.value();
            if language.is_empty() || !language.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            {
                return Err(Error::new_spanned(
                    lit,
                    "`search_language` is a lowercase name such as `english` or `simple`",
                ));
            }
            if searchable.is_empty() {
                return Err(Error::new_spanned(
                    lit,
                    "`search_language` needs `search = \"…\"` too",
                ));
            }
            Some(quote! { const SEARCH_LANGUAGE: &'static str = #language; })
        }
        None => None,
    };

    let from_row = fields.iter().map(|f| {
        let (ident, name) = (&f.ident, &f.name);
        if f.skip {
            quote! { #ident: ::core::default::Default::default() }
        } else {
            quote! { #ident: row.try_get(#name)? }
        }
    });

    let values = fields
        .iter()
        .filter(|f| !f.skip && f.name != "id")
        .map(|f| {
            let ident = &f.ident;
            quote! { ::renox::db::ToDbValue::to_db_value(&self.#ident) }
        });

    let created = column("created_at").map(|f| {
        let ident = &f.ident;
        if is_option(&f.ty) {
            quote! { if creating && self.#ident.is_none() { self.#ident = ::core::option::Option::Some(now); } }
        } else {
            quote! { if creating { self.#ident = now; } }
        }
    });
    let updated = column("updated_at").map(|f| {
        let ident = &f.ident;
        if is_option(&f.ty) {
            quote! { self.#ident = ::core::option::Option::Some(now); }
        } else {
            quote! { self.#ident = now; }
        }
    });
    // `replicate()`: a copy gets new timestamps when it is saved.
    let forget_timestamps = ["created_at", "updated_at"].map(|name| {
        column(name).filter(|f| is_option(&f.ty)).map(|f| {
            let ident = &f.ident;
            quote! { self.#ident = ::core::option::Option::None; }
        })
    });
    let set_deleted_at = deleted_at.filter(|_| soft_deletes).map(|f| {
        let ident = &f.ident;
        quote! {
            fn set_deleted_at(&mut self, at: ::core::option::Option<::renox::db::DateTime>) {
                self.#ident = at;
            }
        }
    });

    let default_scope_fn = default_scope.map(|path| {
        quote! {
            fn default_scope(query: ::renox::db::Query<Self>) -> ::renox::db::Query<Self> {
                #path(query)
            }
        }
    });

    let hooks_fns = hooks.then(|| {
        quote! {
            fn saving(&mut self, creating: bool) -> ::renox::Result {
                <Self as ::renox::db::ModelHooks>::saving(self, creating)
            }
            fn saved(&self, created: bool) -> impl ::std::future::Future<Output = ::renox::Result> + Send {
                <Self as ::renox::db::ModelHooks>::saved(self, created)
            }
            fn deleting(&self) -> ::renox::Result {
                <Self as ::renox::db::ModelHooks>::deleting(self)
            }
            fn deleted(&self) -> impl ::std::future::Future<Output = ::renox::Result> + Send {
                <Self as ::renox::db::ModelHooks>::deleted(self)
            }
        }
    });

    let from_row_impl = crate::from_row::from_row_impl(ident, quote! { #(#from_row),* });
    Ok(quote! {
        #from_row_impl

        impl ::renox::db::Model for #ident {
            const TABLE: &'static str = #table;
            const COLUMNS: &'static [&'static str] = &[#(#columns),*];
            const SOFT_DELETES: bool = #soft_deletes;
            #search_const
            #language_const

            type Key = #key_type;

            #[allow(clippy::clone_on_copy)]
            fn id(&self) -> Self::Key {
                ::core::clone::Clone::clone(&self.id)
            }

            fn set_id(&mut self, id: Self::Key) {
                self.id = id;
            }

            fn values(&self) -> ::std::vec::Vec<::renox::db::DbValue> {
                ::std::vec![#(#values),*]
            }

            #[allow(unused_variables)]
            fn touch(&mut self, now: ::renox::db::DateTime, creating: bool) {
                #created
                #updated
            }

            fn forget_timestamps(&mut self) {
                #(#forget_timestamps)*
            }

            #set_deleted_at

            #default_scope_fn

            #hooks_fns
        }
    })
}

fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|s| s.ident == "Option"))
}

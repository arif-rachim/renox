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
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("model")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                table = meta.value()?.parse::<LitStr>()?.value();
                Ok(())
            } else if meta.path.is_ident("soft_deletes") {
                soft_deletes = true;
                Ok(())
            } else {
                Err(meta.error("expected `table = \"...\"` or `soft_deletes`"))
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
    if column("id").is_none() {
        return Err(Error::new_spanned(
            ident,
            "a Model needs an `id: i64` field",
        ));
    }
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
    let set_deleted_at = deleted_at.filter(|_| soft_deletes).map(|f| {
        let ident = &f.ident;
        quote! {
            fn set_deleted_at(&mut self, at: ::core::option::Option<::renox::db::DateTime>) {
                self.#ident = at;
            }
        }
    });

    Ok(quote! {
        impl ::renox::db::Model for #ident {
            const TABLE: &'static str = #table;
            const COLUMNS: &'static [&'static str] = &[#(#columns),*];
            const SOFT_DELETES: bool = #soft_deletes;

            fn id(&self) -> i64 {
                self.id
            }

            fn set_id(&mut self, id: i64) {
                self.id = id;
            }

            fn from_row(
                row: &::renox::db::Row,
            ) -> ::core::result::Result<Self, ::renox::sqlx::Error> {
                ::core::result::Result::Ok(Self { #(#from_row),* })
            }

            fn values(&self) -> ::std::vec::Vec<::renox::db::DbValue> {
                ::std::vec![#(#values),*]
            }

            #[allow(unused_variables)]
            fn touch(&mut self, now: ::renox::db::DateTime, creating: bool) {
                #created
                #updated
            }

            #set_deleted_at
        }
    })
}

fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|s| s.ident == "Option"))
}

//! An admin panel for Renox apps, generated from their models (Laravel's
//! Filament resources): each model is declared once, as an
//! [`AdminResource`] (its grid columns, its form fields, its policy), and
//! the panel gives it a list with search, filters, sorting, bulk actions
//! and exports, and create, edit, view and delete pages, on the UI kit and
//! `renox::grid`.
//!
//! ```
//! use renox::prelude::*;
//! use renox_admin::Admin;
//! # use renox::grid::Column;
//! # use renox_admin::{AdminResource, Field};
//! # #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, name: String }
//! # impl Policy for Product { fn allows(&self, _: &User, _: &str) -> bool { true } }
//! # #[derive(serde::Deserialize, serde::Serialize, Validate)] struct ProductForm { #[validate(required)] name: String }
//! # struct Products;
//! # impl AdminResource for Products {
//! #     type Model = Product;
//! #     type Form = ProductForm;
//! #     fn label(&self) -> &str { "Product" }
//! #     fn plural_label(&self) -> &str { "Products" }
//! #     fn columns(&self) -> Vec<Column> { vec![Column::text("name", "Name")] }
//! #     fn fields(&self) -> Vec<Field> { vec![Field::text("name", "Name")] }
//! #     fn fill(&self, p: &mut Product, f: ProductForm) { p.name = f.name; }
//! # }
//!
//! # let _ =
//! App::new()
//!     .module(Auth::new())
//!     .module(
//!         Admin::new()
//!             .title("Back office")
//!             .authorize(|user| user.has_role("staff")) // who may open the panel
//!             .resource(Products),
//!     )
//! # ;
//! ```
//!
//! The panel lives at `/admin` (`path`), behind a login. Nobody gets in
//! until [`Admin::authorize`] (or [`Admin::gate`]) says who may; then each
//! page asks the resource's [`AdminResource::allows`], which by default
//! asks the model's [`Policy`] (`viewAny`, `view`, `create`, `update`,
//! `delete`, …). Pages are built-in templates (`renox-admin/*.html`) that
//! an app replaces with files of the same name. The guide is
//! docs/admin.md in the Renox repository.

#![warn(missing_docs)]

use std::fmt;
use std::sync::Arc;

use renox::prelude::*;

pub mod entry;
pub mod field;
mod panel;
pub mod resource;

pub use entry::Entry;
pub use field::{Field, FieldKind};
pub use resource::{ActionContext, AdminAction, AdminResource, BoxFuture, Filter};

use panel::{Access, Holder, Listed, Panel};

/// The templates, compiled in. An app replaces one with a file of the same
/// name under its views directory (`resources/views/renox-admin/layout.html`).
const VIEWS: &[(&str, &str)] = &[
    (
        "renox-admin/layout.html",
        include_str!("../views/layout.html"),
    ),
    (
        "renox-admin/dashboard.html",
        include_str!("../views/dashboard.html"),
    ),
    (
        "renox-admin/index.html",
        include_str!("../views/index.html"),
    ),
    ("renox-admin/form.html", include_str!("../views/form.html")),
    ("renox-admin/show.html", include_str!("../views/show.html")),
    (
        "renox-admin/fields.html",
        include_str!("../views/fields.html"),
    ),
];

/// The admin panel module: its address, its title, who may use it and its
/// resources.
#[derive(Clone)]
pub struct Admin {
    path: String,
    title: String,
    access: Option<Access>,
    resources: Vec<Arc<dyn Listed>>,
}

impl Default for Admin {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Admin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Admin")
            .field("path", &self.path)
            .field("title", &self.title)
            .field(
                "resources",
                &self.resources.iter().map(|r| r.slug()).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Admin {
    /// A panel at `/admin`, titled "Admin", with no resources yet, that
    /// nobody may open until [`authorize`](Self::authorize) says who may.
    pub fn new() -> Self {
        Self {
            path: "/admin".into(),
            title: "Admin".into(),
            access: None,
            resources: Vec::new(),
        }
    }

    /// Where the panel lives (`/admin` by default). The route names stay
    /// `admin.*`.
    ///
    /// # Panics
    ///
    /// If `path` doesn't start with `/` or ends with one.
    pub fn path(mut self, path: &str) -> Self {
        assert!(
            path.starts_with('/') && !path.ends_with('/'),
            "Admin::path(\"{path}\"): it must start with `/` and not end with one"
        );
        self.path = path.to_owned();
        self
    }

    /// The panel's name, at the top of its navigation and in page titles.
    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_owned();
        self
    }

    /// Who may open the panel at all: logged-in users for whom `allows`
    /// says yes (others get 403, guests go to log in). Without it, nobody
    /// may.
    pub fn authorize(mut self, allows: impl Fn(&AuthUser) -> bool + Send + Sync + 'static) -> Self {
        self.access = Some(Arc::new(allows));
        self
    }

    /// Lets in the users the gate `name` lets through (`App::gate`, or a
    /// permission of that name with the `Permissions` module, after
    /// `App::gate_before`).
    pub fn gate(self, name: &str) -> Self {
        let name = name.to_owned();
        self.authorize(move |user| user.allows(&name))
    }

    /// Adds a resource, after the ones before it in the navigation.
    ///
    /// # Panics
    ///
    /// If its slug isn't letters, digits, `_` and `-`, or another resource
    /// has it.
    pub fn resource<R: AdminResource>(mut self, resource: R) -> Self {
        let slug = resource.slug().to_owned();
        assert!(
            !slug.is_empty()
                && slug.len() <= 64
                && slug
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "Admin::resource: the slug `{slug}` may only have letters, digits, `_` and `-`"
        );
        assert!(
            self.resources.iter().all(|r| r.slug() != slug),
            "Admin::resource: two resources have the slug `{slug}`"
        );
        self.resources.push(Arc::new(Holder(Arc::new(resource))));
        self
    }
}

impl Module for Admin {
    fn name(&self) -> &'static str {
        "admin"
    }

    fn routes(&self) -> Routes {
        panel::routes(Arc::new(Panel {
            path: self.path.clone(),
            title: self.title.clone(),
            access: self.access.clone(),
            resources: self.resources.clone(),
        }))
    }

    fn register(&self, app: &mut Registry) {
        app.templates(|env| {
            for (name, source) in VIEWS {
                // The app's own file of that name wins.
                if env.get_template(name).is_err() {
                    let _ = env.add_template(name, source);
                }
            }
        });
    }
}

/// Compiles the Rust in docs/admin.md (the guide) as doctests.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/admin.md")]
pub struct Guide;

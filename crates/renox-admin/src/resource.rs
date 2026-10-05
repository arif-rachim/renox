//! What an app declares for each model in the panel: [`AdminResource`],
//! with its [`AdminAction`]s and [`Filter`]s.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use renox::db::{Model, Query};
use renox::grid::{Column, Grid};
use renox::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Entry, Field};

/// A boxed future that can be sent between threads, as an action returns.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A model in the admin panel (Filament's resources), declared once: the
/// grid's columns, the form's fields, its filters and actions, and who may
/// do what (its [`Policy`]).
///
/// ```
/// use renox::grid::Column;
/// use renox::prelude::*;
/// use renox_admin::{AdminResource, Field};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Model, Serialize, Default, Clone)]
/// struct Product { id: i64, name: String, price: i64, active: bool }
///
/// impl Policy for Product {
///     fn allows(&self, user: &User, ability: &str) -> bool {
///         // Everyone in the panel looks; only managers change things.
///         matches!(ability, "viewAny" | "view") || user.has_role("manager")
///     }
/// }
///
/// #[derive(Deserialize, Serialize, Validate)]
/// struct ProductForm {
///     #[validate(required, max = 100)]
///     name: String,
///     #[validate(required, min = 0)]
///     price: i64,
///     active: bool,
/// }
///
/// struct Products;
///
/// impl AdminResource for Products {
///     type Model = Product;
///     type Form = ProductForm;
///
///     fn label(&self) -> &str { "Product" }
///     fn plural_label(&self) -> &str { "Products" }
///
///     fn columns(&self) -> Vec<Column> {
///         vec![
///             Column::text("name", "Name").searchable(),
///             Column::money("price", "Price"),
///             Column::bool("active", "Active"),
///         ]
///     }
///
///     fn fields(&self) -> Vec<Field> {
///         vec![
///             Field::text("name", "Name").required(),
///             Field::money("price", "Price").required(),
///             Field::toggle("active", "Active").default_value(true),
///         ]
///     }
///
///     fn fill(&self, product: &mut Product, form: ProductForm) {
///         product.name = form.name;
///         product.price = form.price;
///         product.active = form.active;
///     }
/// }
/// ```
pub trait AdminResource: Send + Sync + 'static {
    /// The model the resource manages. `Default` gives the record a create
    /// page starts from, and the one list-level abilities are asked about.
    type Model: Model + Policy + Serialize + Default + Send + Sync;

    /// The form of the create and edit pages, checked with its own rules
    /// (`Valid<T>`). `Serialize` refills it after [`rules`](Self::rules)
    /// fails (passwords aren't refilled).
    type Form: DeserializeOwned + Validate + Serialize + Send + 'static;

    /// One record's name: "Product".
    fn label(&self) -> &str;

    /// Several records' name, in the navigation and the list's title:
    /// "Products".
    fn plural_label(&self) -> &str;

    /// What a record's view and edit pages call it: "Product #7" by
    /// default; its name reads better (`product.name.clone()`).
    fn record_title(&self, record: &Self::Model) -> String {
        format!("{} #{}", self.label(), record.id())
    }

    /// The resource's part of the address and of the route names
    /// (`/admin/products`, `admin.products.index`): letters, digits, `_`
    /// and `-`. The model's table by default.
    fn slug(&self) -> &str {
        Self::Model::TABLE
    }

    /// A heading the navigation puts the resource under ("Shop").
    fn navigation_group(&self) -> Option<&str> {
        None
    }

    /// The list's columns, left to right (`renox::grid`). Make some
    /// `.searchable()` for the list's search box.
    fn columns(&self) -> Vec<Column>;

    /// The create and edit pages' fields, in order.
    fn fields(&self) -> Vec<Field>;

    /// Copies a valid form into the record, before it is saved (a new
    /// record starts from `Default`).
    fn fill(&self, record: &mut Self::Model, form: Self::Form);

    /// Rules that need the record being edited (`None` while creating),
    /// checked after the form's own, e.g. a unique value that may stay the
    /// record's own: `v.field("sku", &form.sku).unique("products", "sku")`
    /// followed by `.ignore(record.id)` when there is one.
    fn rules(&self, form: &Self::Form, record: Option<&Self::Model>, v: &mut Validator) {
        let _ = (form, record, v);
    }

    /// The details of a record's view page. Without any, the resource has
    /// no view page and its rows open the edit page. By default, an entry
    /// per column ([`Entry::from_column`]).
    fn entries(&self) -> Vec<Entry> {
        self.columns()
            .iter()
            .filter_map(Entry::from_column)
            .collect()
    }

    /// Named filters over the list, shown as tabs above it ("Active",
    /// "Low stock"); the grid's columns filter on their own too.
    fn filters(&self) -> Vec<Filter<Self::Model>> {
        Vec::new()
    }

    /// The resource's own actions on selected rows (and, with
    /// [`AdminAction::row`], on one row).
    fn actions(&self) -> Vec<AdminAction<Self::Model>> {
        Vec::new()
    }

    /// Every query the panel runs for this resource starts here: the
    /// model's query (with its default scope) by default. Narrow it to
    /// what the panel may ever show.
    fn query(&self) -> Query<Self::Model> {
        Self::Model::query()
    }

    /// Changes the list's grid after the panel built it from the columns,
    /// e.g. `grid.sort_by("name").per_page(50).groups(&["status"])`.
    fn grid(&self, grid: Grid) -> Grid {
        grid
    }

    /// Whether `user` may do `ability` (`viewAny`, `view`, `create`,
    /// `update`, `delete`, `deleteAny`, `restore`, `restoreAny`,
    /// `forceDelete`, `forceDeleteAny`, or an action's own) on `record`.
    /// By default, the model's [`Policy`] after `App::gate_before`; when
    /// there's no record (the list, the create page, what the list
    /// offers), the policy is asked about a blank record
    /// (`Self::Model::default()`).
    fn allows(&self, user: &AuthUser, ability: &str, record: Option<&Self::Model>) -> bool {
        match record {
            Some(record) => user.can(ability, record),
            None => user.can(ability, &Self::Model::default()),
        }
    }
}

/// What an action's code gets besides the records.
#[derive(Clone)]
#[non_exhaustive]
pub struct ActionContext {
    /// The app's state, e.g. its database (`state.db`).
    pub state: AppState,
    /// Who chose the action.
    pub user: AuthUser,
}

type Run<M> = Arc<dyn Fn(Vec<M>, ActionContext) -> BoxFuture<'static, Result<Toast>> + Send + Sync>;

/// An action of a resource's list (Filament's bulk actions): a button over
/// the selected rows that runs `run` with their records and shows the
/// toast it returns. With [`row`](Self::row), each row's menu has it too.
///
/// ```
/// # use renox::prelude::*;
/// use renox_admin::AdminAction;
/// # #[derive(Model, serde::Serialize, Default, Clone)] struct Product { id: i64, active: bool }
///
/// let deactivate = AdminAction::new("deactivate", "Deactivate", |products: Vec<Product>, cx| async move {
///     let ids: Vec<i64> = products.iter().map(|p| p.id).collect();
///     let n = Product::query().where_in("id", ids).update(&cx.state.db, &[("active", &false)]).await?;
///     Ok(Toast::success(format!("{n} products deactivated.")))
/// })
/// .confirm("Deactivate the selected products?")
/// .row();
/// # let _ = deactivate;
/// ```
///
/// Every record must pass [`AdminResource::allows`] for the action's
/// ability (`update` by default), or nothing runs (403).
pub struct AdminAction<M> {
    key: String,
    label: String,
    ability: String,
    confirm: Option<String>,
    danger: bool,
    bulk: bool,
    row: bool,
    run: Run<M>,
}

impl<M> Clone for AdminAction<M> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            label: self.label.clone(),
            ability: self.ability.clone(),
            confirm: self.confirm.clone(),
            danger: self.danger,
            bulk: self.bulk,
            row: self.row,
            run: self.run.clone(),
        }
    }
}

impl<M> fmt::Debug for AdminAction<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdminAction")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("ability", &self.ability)
            .finish_non_exhaustive()
    }
}

impl<M: Send + 'static> AdminAction<M> {
    /// An action named `key` (letters, digits, `_` and `-`; part of its
    /// address), shown as `label`, that runs `run`.
    pub fn new<F, Fut>(key: &str, label: &str, run: F) -> Self
    where
        F: Fn(Vec<M>, ActionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Toast>> + Send + 'static,
    {
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
            ability: "update".into(),
            confirm: None,
            danger: false,
            bulk: true,
            row: false,
            run: Arc::new(move |records, cx| Box::pin(run(records, cx))),
        }
    }

    /// The ability each record must allow (`update` by default).
    pub fn ability(mut self, ability: &str) -> Self {
        self.ability = ability.to_owned();
        self
    }

    /// Asks first, in a dialog with this question.
    pub fn confirm(mut self, question: &str) -> Self {
        self.confirm = Some(question.to_owned());
        self
    }

    /// Shown in red, for actions that can't be undone.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Also in each row's menu, for that row alone.
    pub fn row(mut self) -> Self {
        self.row = true;
        self
    }

    /// Only in each row's menu, not over the selection.
    pub fn row_only(mut self) -> Self {
        self.row = true;
        self.bulk = false;
        self
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn ability_name(&self) -> &str {
        &self.ability
    }

    pub(crate) fn question(&self) -> Option<&str> {
        self.confirm.as_deref()
    }

    pub(crate) fn is_danger(&self) -> bool {
        self.danger
    }

    pub(crate) fn on_bulk(&self) -> bool {
        self.bulk
    }

    pub(crate) fn on_row(&self) -> bool {
        self.row
    }

    pub(crate) fn run(
        &self,
        records: Vec<M>,
        cx: ActionContext,
    ) -> BoxFuture<'static, Result<Toast>> {
        (self.run)(records, cx)
    }
}

type Scope<M> = Arc<dyn Fn(Query<M>) -> Query<M> + Send + Sync>;

/// A named filter over a resource's list (Filament's tabs and preset
/// filters), chosen with `?filter=key`.
///
/// ```
/// # use renox::prelude::*;
/// use renox_admin::Filter;
/// # #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, active: bool, stock: i64 }
///
/// let filters = vec![
///     Filter::new("active", "Active", |q| q.where_eq("active", true)),
///     Filter::new("low", "Low stock", |q| q.where_op("stock", "<", 5)),
/// ];
/// # let _: Vec<Filter<Product>> = filters;
/// ```
pub struct Filter<M> {
    key: String,
    label: String,
    scope: Scope<M>,
}

impl<M> Clone for Filter<M> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            label: self.label.clone(),
            scope: self.scope.clone(),
        }
    }
}

impl<M> fmt::Debug for Filter<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("key", &self.key)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl<M> Filter<M> {
    /// A filter named `key` (its value in `?filter=`), shown as `label`,
    /// that narrows the list's query with `scope`.
    pub fn new(
        key: &str,
        label: &str,
        scope: impl Fn(Query<M>) -> Query<M> + Send + Sync + 'static,
    ) -> Self {
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
            scope: Arc::new(scope),
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn apply(&self, query: Query<M>) -> Query<M> {
        (self.scope)(query)
    }
}

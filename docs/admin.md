# Admin panel

`renox-admin` makes the back office for you: you declare each model once, as a **resource**
(its list's columns, its form's fields, who may do what), and the panel gives it a list with
search, filters, sorting, bulk actions and exports, and pages to create, view, edit and delete
records. It is built on the [UI kit](ui.md) and the [data grid](grid.md), so it looks and
works like the rest of a Renox app.

`examples/admin` in the repository is a shop's back office with three resources and two roles,
and no page written by hand.

### In this guide

- adding the panel and saying who may open it;
- declaring a resource: columns, fields, the form and how it fills the model;
- the policy behind every page and button;
- filters, actions, soft deletes and the trash, exports;
- the view page, the routes, and replacing the panel's pages;
- how it maps to Laravel's Filament.

### Words you'll meet

| Word | What it means |
|---|---|
| **panel** | The admin pages: a sidebar of resources, a dashboard, and each resource's pages. |
| **resource** | One model in the panel, declared with the `AdminResource` trait. |
| **policy** | The model's `Policy`: whether a user may do an ability (`update`, `delete`…) to a record. |
| **ability** | A word for an action people take: `viewAny` (see the list), `create`, `update`… |
| **soft delete** | Marking a row deleted (`deleted_at`) instead of removing it, so it can come back. |

> [!NOTE]
> **Coming from Laravel:** this is Filament's resources. The last section,
> [Coming from Laravel and Filament](#coming-from-laravel-and-filament), maps one to the other.

## Adding the panel

Add the crate next to `renox`:

```toml
[dependencies]
renox = "1.0.0-rc.4"
renox-admin = "1.0.0-rc.4"
```

Then add the `Admin` module, after `Auth` (the panel is behind a login):

```rust
use renox::prelude::*;
use renox_admin::Admin;
# use renox::grid::Column;
# use renox_admin::{AdminResource, Field};
# #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, name: String }
# impl Policy for Product { fn allows(&self, _: &User, _: &str) -> bool { true } }
# #[derive(serde::Deserialize, serde::Serialize, Validate)] struct ProductForm { #[validate(required)] name: String }
# struct ProductResource;
# impl AdminResource for ProductResource {
#     type Model = Product;
#     type Form = ProductForm;
#     fn label(&self) -> &str { "Product" }
#     fn plural_label(&self) -> &str { "Products" }
#     fn columns(&self) -> Vec<Column> { vec![Column::text("name", "Name")] }
#     fn fields(&self) -> Vec<Field> { vec![Field::text("name", "Name")] }
#     fn fill(&self, p: &mut Product, f: ProductForm) { p.name = f.name; }
# }

/// The app, with the panel at /admin.
pub fn app() -> App {
    App::new()
        .module(Auth::new())
        .module(Admin::new()
            .title("Corner Shop")                     // the sidebar's name
            .authorize(|user| user.has_role("staff")) // who may open the panel
            .resource(ProductResource))
}
```

What each part does:

- `Admin::new()` makes a panel at `/admin`. `.path("/backoffice")` moves it; the route names
  stay `admin.*`.
- `.title(…)` names the panel in its sidebar and page titles.
- `.authorize(|user| …)` says who may open the panel at all. **Nobody may until you say so**:
  without it every page answers 403 (forbidden). Guests are sent to log in. `.gate("admin")`
  is the same with a gate or a permission of that name ([authorization.md](authorization.md)).
- `.resource(…)` adds a resource. They show in the sidebar in the order you add them.

The panel's pages:

- `/admin`: the dashboard, with how many records each resource has;
- `/admin/products`: the list;
- `/admin/products/create` and `/admin/products/{id}/edit`: the forms;
- `/admin/products/{id}`: the view page.

## Declaring a resource

A resource is a type that implements `AdminResource`. It names two types:

- `Model`: the model it manages. It needs `Serialize` (the pages read its fields), `Default` (a
  new record starts from it) and `Policy` (see [The policy](#the-policy)).
- `Form`: the create and edit form, with its own rules (`Validate`), exactly as a handler's
  `Valid<T>` would read it. It needs `Serialize` too, to refill the form after an error.

```rust
use renox::chrono::NaiveDate;
use renox::grid::Column;
use renox::prelude::*;
use renox_admin::{AdminResource, Field};
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Default)]
#[model(table = "products")]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub price: i64,
    pub status: String,
    pub active: bool,
    pub category_id: Option<i64>,
    pub released_on: Option<NaiveDate>,
}

impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        // Everyone in the panel looks; managers change things.
        matches!(ability, "viewAny" | "view") || user.has_role("manager")
    }
}

/// The create and edit form: Renox checks these rules before anything is saved.
#[derive(Deserialize, Serialize, Validate)]
pub struct ProductForm {
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, min = 0)]
    pub price: i64,
    #[validate(required, one_of(&["draft", "live"]))]
    pub status: String,
    pub active: bool,
    #[validate(exists("categories", "id"))]
    pub category_id: Option<i64>,
    pub released_on: Option<NaiveDate>,
}

pub struct ProductResource;

impl AdminResource for ProductResource {
    type Model = Product;
    type Form = ProductForm;

    fn label(&self) -> &str { "Product" }
    fn plural_label(&self) -> &str { "Products" }

    /// The list: a data grid with these columns.
    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name").searchable(),
            Column::money("price", "Price"),
            Column::select("status", "Status", [("draft", "Draft"), ("live", "Live")]),
            Column::bool("active", "Active"),
            Column::related("category", "Category", "categories", "category_id", "name"),
        ]
    }

    /// The create and edit pages: these fields, in a two-column form.
    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("name", "Name").required(),
            Field::money("price", "Price").required(),
            Field::select("status", "Status", [("draft", "Draft"), ("live", "Live")])
                .required()
                .default_value("draft"),
            Field::toggle("active", "Active").default_value(true),
            Field::belongs_to("category_id", "Category", "categories", "name"),
            Field::date("released_on", "Released"),
        ]
    }

    /// A valid form, copied into the record before it is saved.
    fn fill(&self, product: &mut Product, form: ProductForm) {
        product.name = form.name;
        product.price = form.price;
        product.status = form.status;
        product.active = form.active;
        product.category_id = form.category_id;
        product.released_on = form.released_on;
    }
}
```

What's going on:

- `label` and `plural_label` name one record and several. Renox doesn't make plurals for you
  (not every language adds an "s").
- `columns()` are [grid](grid.md) columns, with every option the grid has: `.searchable()`
  puts a search box over the list, each heading filters and sorts by its kind, and
  `Column::related` shows a value from another table.
- `fields()` are the form's fields, left to right, top to bottom.
- `fill` copies a valid form into the record. Creating starts from `Product::default()`;
  editing starts from the record as it is. Then the panel saves it (model hooks run).

The rest of the trait has defaults you can change:

| Method | Default | What it's for |
|---|---|---|
| `slug()` | the model's table | the resource's part of the address and route names: `/admin/products`, `admin.products.index` |
| `navigation_group()` | none | a heading in the sidebar ("Catalog") |
| `record_title(record)` | "Product #7" | what the view and edit pages call a record (its name reads better) |
| `rules(form, record, v)` | none | rules that need the record (below) |
| `entries()` | one per column | the view page's details; none means no view page |
| `filters()` | none | named filters, as tabs over the list |
| `actions()` | none | the resource's own actions on selected rows |
| `query()` | `Model::query()` | where every query starts: narrow what the panel may ever show |
| `grid(grid)` | as built | changes to the list's grid: `grid.sort_by("name").cards_on_mobile()` |
| `allows(user, ability, record)` | the model's policy | who may do what ([The policy](#the-policy)) |

### Fields

Each `Field` constructor takes the form's field name (the same as the model's, usually) and its
label. The kind decides which of the kit's fields draws it:

| Field | Draws | In the form |
|---|---|---|
| `text`, `email`, `url`, `tel` | a text box of that type | `String` |
| `password` | a password box with a "show" button, empty on the edit page | `Option<String>` (change it only when one was typed) |
| `textarea` | several lines (`.rows(n)`) | `String` |
| `number` | a number box (`.step("0.01")`, `.min(0)`, `.max(99)`) | a number |
| `money` | a number box with `APP_CURRENCY`'s code before it | the amount as the model stores it |
| `date` | the kit's date picker | `NaiveDate` |
| `datetime` | a date-and-time box | `NaiveDateTime` |
| `select(name, label, options)` | a select of `(value, label)` pairs (`.searchable()`) | the value |
| `checkbox`, `toggle` | a checkbox, or a switch | `bool` (unticked is `false`) |
| `belongs_to(name, label, table, title)` | a searchable select of `table`'s rows, by `title` | the row's id |

> [!NOTE]
> `money` takes the amount as the model stores it: the smallest unit, as the grid's `money`
> columns read it. For a currency without decimals (IDR, JPY) that's the amount people write.
> For cents, take a decimal in the form (`Field::number(…).step("0.01")`) and convert it in
> `fill`.

Options every field takes:

- `.required()` marks it for people and the browser. The form's rules decide what's valid.
- `.hint("…")`, `.placeholder("…")`, `.prefix("https://")`, `.suffix("kg")`.
- `.span_full()` takes the form's whole width.
- `.default_value(…)` is what the create page starts with.
- `.only_on_create()`, `.only_on_edit()`, and `.readonly_on_edit()` (shown, still sent, but
  not changeable once the record exists).
- `.autocomplete("off")`.

A `belongs_to` field lists the first 1,000 rows of its table. Pair it with
`#[validate(exists("categories", "id"))]` in the form, so only real rows are accepted.

### Rules that need the record

The form's own rules can't see the record being edited. A unique value is the usual case: a
product's SKU may be its own, but not another product's. `rules` runs after the form's rules,
with the record when editing (`None` while creating):

```rust
# use renox::prelude::*;
# use renox::grid::Column;
# use renox_admin::{AdminResource, Field};
# #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, sku: String }
# impl Policy for Product { fn allows(&self, _: &User, _: &str) -> bool { true } }
# #[derive(serde::Deserialize, serde::Serialize, Validate)] struct ProductForm { #[validate(required)] sku: String }
# struct ProductResource;
# impl AdminResource for ProductResource {
#     type Model = Product;
#     type Form = ProductForm;
#     fn label(&self) -> &str { "Product" }
#     fn plural_label(&self) -> &str { "Products" }
#     fn columns(&self) -> Vec<Column> { vec![] }
#     fn fields(&self) -> Vec<Field> { vec![] }
#     fn fill(&self, p: &mut Product, f: ProductForm) { p.sku = f.sku; }
/// The SKU is unique among the products, but a product keeps its own.
fn rules(&self, form: &ProductForm, product: Option<&Product>, v: &mut Validator) {
    let rule = v.field("sku", &form.sku).label("SKU").unique("products", "sku");
    if let Some(product) = product {
        rule.ignore(product.id);
    }
}
# }
```

A failure answers as the form's own rules do: the errors under their fields.

### How the forms behave

- The forms are sent with htmx. Errors show under their fields (a 422 answer) and the form
  keeps what was typed. A success goes back to the list, with a toast ("Product saved.").
- They check each field as you leave it (live validation), with the form's rules.
- Without JavaScript they are plain forms: an error comes back to the form with what was typed.
- Ctrl+S (⌘S on a Mac) saves; N on the list opens the create page.

## The policy

Every page and button asks `AdminResource::allows(user, ability, record)`. By default it asks
the model's `Policy` (after `App::gate_before`), with these abilities:

| Ability | Asked before |
|---|---|
| `viewAny` | the list, its exports, the resource's link in the sidebar and its dashboard figure |
| `view` | a record's view page |
| `create` | the create page, and the list's "New" button |
| `update` | the edit page and saving it; the list's "Edit" link; an action's default |
| `delete` | deleting a record |
| `deleteAny` | offering "Delete" over selected rows (each record is asked `delete` too) |
| `restore`, `restoreAny` | bringing back deleted records; the "Trash" tab |
| `forceDelete`, `forceDeleteAny` | deleting records for good, from the trash |

A record-level ability gets the record. For the list-level questions (`viewAny`, `create`,
`deleteAny`, and whether the list offers "Edit" or "Delete" at all), there's no record yet,
so the policy gets a blank one (`Product::default()`, id 0). A policy that decides by role
works for both:

```rust
# use renox::prelude::*;
# #[derive(Model, serde::Serialize, Default)] struct Product { id: i64 }
impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "viewAny" | "view" => true,
            "create" | "update" => user.has_permission("catalog.manage"),
            _ => user.has_permission("records.delete"), // delete, restore, …
        }
    }
}
```

A policy that decides by the record (only the author edits) also works: each record is
checked when it's opened or changed. The list asks about a blank record, though, so it would
hide its "Edit" link; override `allows` on the resource for a different answer when there's
no record.

What's refused answers 403, and the pages leave out the buttons the user may not use. Bulk
actions check every selected record: one refusal and nothing runs.

## Filters

Each column's heading already filters by its kind (see [grid.md](grid.md)). On top of that,
`filters()` adds named filters, shown as tabs over the list ("All", then yours):

```rust
# use renox::prelude::*;
use renox_admin::Filter;
# #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, status: String, stock: i64 }
/// The products resource's tabs.
fn filters() -> Vec<Filter<Product>> {
    vec![
        Filter::new("live", "Live", |q| q.where_eq("status", "live")),
        Filter::new("low", "Low stock", |q| q.where_op("stock", "<", 5)),
    ]
}
# let _ = filters();
```

A tab is `?filter=live` in the address, so it has a link of its own. The grid's search,
filters and exports work inside it.

## Actions

The list has these actions out of the box, each shown only to those allowed:

- in each row's ⋯ menu: **View**, **Edit** and **Delete** (with a confirmation);
- over the selected rows: **Delete** (a checkbox per row, "Select all N matching" for every
  row the filters match).

`actions()` adds the resource's own. An action gets the selected records and answers with a
toast:

```rust
# use renox::prelude::*;
use renox_admin::AdminAction;
# #[derive(Model, serde::Serialize, Default)] struct Product { id: i64, status: String }
/// "Publish" over the selection, and in each row's menu.
fn publish() -> AdminAction<Product> {
    AdminAction::new("publish", "Publish", |products: Vec<Product>, cx| async move {
        let ids: Vec<i64> = products.iter().map(|p| p.id).collect();
        let n = Product::query()
            .where_in("id", ids)
            .update(&cx.state.db, &[("status", &"live")])
            .await?;
        Ok(Toast::success(format!("{n} published.")))
    })
    .confirm("Publish the selected products?") // asks first
    .row() // also in each row's menu
}
# let _ = publish();
```

- The key (`"publish"`) is part of the action's address: `POST /admin/products/actions/publish`
  (selected rows) and `POST /admin/products/{id}/actions/publish` (one row).
- `cx` is an `ActionContext`: `cx.state` (the database, the queue, the mailer…) and `cx.user`.
- `.ability("publish")` changes the ability each record must allow (`update` by default).
- `.danger()` shows it in red; `.row_only()` puts it only in the rows' menus.
- At most 10,000 records are loaded for one action.

## Soft deletes and the trash

When the model has soft deletes (`#[model(soft_deletes)]`, a `deleted_at` column), deleting
puts a record in the trash instead of removing it:

- the list gets a **Trash** tab (for those allowed `restoreAny`) listing the deleted records,
  with **Restore** and **Delete for good** (`restore`, `forceDelete`) per row and over the
  selection;
- a deleted record's view page says so, and offers the same.

Without soft deletes, "Delete" removes the row, and the confirmation says it can't be undone.

## Exports

Every list has the grid's export menu: CSV, a print page, and Excel when the app turns on the
`xlsx` feature (`renox-admin = { version = "…", features = ["xlsx"] }`, or renox's own `xlsx`
feature). An export holds every row the filters, the search and the tab match, in the columns
the user shows.

## The view page

A record's view page lists its details in the kit's infolist. By default there is one entry
per column, formatted by the column's kind (money, dates, yes/no, a select's labels as badges).
`entries()` sets them yourself:

```rust
use renox_admin::Entry;

/// The products' view page.
fn entries() -> Vec<Entry> {
    vec![
        Entry::text("name", "Name"),
        Entry::text("sku", "SKU").copyable(),
        Entry::new("price", "Price").format("money"),
        Entry::new("status", "Status").labels([("live", "Live")]).badge(),
        Entry::new("updated_at", "Last changed").format("since"),
        Entry::new("notes", "Notes").format("markdown").span_full(),
    ]
}
# assert_eq!(entries().len(), 6);
```

The formats are the kit's `entry` formats: `date`, `datetime`, `since`, `money`, `number`,
`bool`, `color`, `image` and `markdown`. Return no entries for a resource without a view page:
its rows open the edit page instead.

## Routes

The panel's routes are named, so templates and redirects can reach them with `route(…)`:

| Route | Name |
|---|---|
| `GET /admin` | `admin.dashboard` |
| `GET /admin/products` (and its exports) | `admin.products.index` |
| `GET /admin/products/create`, `POST /admin/products` | `admin.products.create`, `admin.products.store` |
| `GET /admin/products/{id}` | `admin.products.show` |
| `GET /admin/products/{id}/edit`, `PUT /admin/products/{id}` | `admin.products.edit`, `admin.products.update` |
| `DELETE /admin/products/{id}` | `admin.products.destroy` |
| `POST /admin/products/actions/{action}` | `admin.products.bulk` |
| `POST /admin/products/{id}/actions/{action}` | `admin.products.action` |

The built-in actions are `delete`, `restore` and `force-delete`. Every route needs a login,
and `rnx route:list` (the app's `route:list`) shows them.

## Replacing the panel's pages

The pages are templates of the crate. An app replaces one by having a file of the same name
under its views directory:

| Template | What it is |
|---|---|
| `renox-admin/layout.html` | the frame: sidebar, account menu, toasts (brand the panel here) |
| `renox-admin/dashboard.html` | the dashboard (`cards`: each resource's `label`, `count`, `url`) |
| `renox-admin/index.html` | a list |
| `renox-admin/form.html` | the create and edit page |
| `renox-admin/fields.html` | the macro that draws one field |
| `renox-admin/show.html` | the view page |

Every page gets `admin` (the panel: `title`, `nav`, …) and `resource` (`slug`, `label`,
`plural_label`, `index_url`, `create_url`).

Grid columns of the `custom` kind are drawn by the page. The list includes
`renox-admin/{slug}/cells.html` when the app has one, with `row` and `column`:

```html
{#- resources/views/renox-admin/products/cells.html -#}
{%- from "renox/ui.html" import badge -%}
{%- if column.key == "level" -%}
  {{ row.stock | number }} {% if row.stock < 5 %}{{ badge("Low", "warning") }}{% endif %}
{%- endif -%}
```

## Coming from Laravel and Filament

> [!NOTE]
> **Coming from Laravel:** if you know Filament, this table says what each of its resource
> features is called here. If you don't, you can skip it.

| Filament | renox-admin |
|---|---|
| `Panel::make()->path('admin')` | `Admin::new().path("/admin")` |
| `canAccessPanel()` | `Admin::authorize(\|user\| …)` or `.gate("admin")` |
| `class ProductResource extends Resource` | `impl AdminResource for ProductResource` |
| `$model`, `$navigationGroup`, `getRecordTitle()` | `type Model`, `navigation_group()`, `record_title()` |
| `table()` with `columns([...])` | `columns()` (the grid's `Column`s) |
| `form()` with `TextInput`, `Select`, `Toggle`, `DatePicker`, `Select::relationship()` | `fields()`: `Field::text`, `select`, `toggle`, `date`, `belongs_to` |
| the form's `->rules()`, `->unique(ignoreRecord: true)` | the form type's `Validate`, `rules()` with `.ignore(record.id)` |
| `mutateFormDataBeforeSave()` | `fill()` |
| `infolist()` | `entries()` |
| `getTabs()`, `filters()` | `filters()` (tabs); each column's own filter |
| `BulkAction`, `Action` | `AdminAction` (`.row()` for the row menu) |
| `DeleteBulkAction`, `RestoreAction`, `ForceDeleteAction`, `TrashedFilter` | built in, with soft deletes |
| `ExportAction` | built in (CSV, print, Excel with `xlsx`) |
| model policies (`viewAny`, `create`, `update`, `delete`, `deleteAny`, …) | the model's `Policy`, same ability names |
| `getEloquentQuery()` | `query()` |

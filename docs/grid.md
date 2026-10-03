# Data grids

`renox::grid` is a table for dashboards and back offices. It fills its container: only the rows
scroll, while the toolbar and the pagination stay in place, on a phone too. Each heading has a
filter that suits its column. Pages, sorting and filters are handled on the server and kept in
the URL, so a filtered page has a link of its own. examples/grid is a sales dashboard built
on it. This guide covers:

- defining a grid, the handler and the template;
- the kinds of columns and what they show;
- filters, search, the active-filter chips and the advanced filter;
- sorting, pages and the user's column choices;
- row details, editing in place, row order and merged cells;
- actions on rows and on a selection;
- summaries and groups;
- cards on phones, columns from other tables, exports and polling;
- several grids on one page, the query string, translations;
- how it maps to Laravel and Filament.

## Defining a grid

A grid is a `Grid` value: an id, columns and options. Building one is cheap, so a function that
returns it is the usual place for one. The handler, the export and the bulk actions can all
call that function.

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid, GridRequest};
# #[derive(Model, serde::Serialize, Default)]
# struct Order { id: i64, number: String, customer: String, status: String, total: i64,
#     ordered_on: renox::chrono::NaiveDate, trend: renox::db::Json<Vec<i64>> }

fn orders_grid() -> Grid {
    Grid::new("orders")
        .title("Orders")
        .column(Column::text("number", "Order").frozen().mobile())
        .column(Column::text("customer", "Name").under(["Customer"]).mobile())
        .column(Column::select("status", "Status", [("new", "New"), ("paid", "Paid")]).mobile())
        .column(Column::money("total", "Total").under(["Amounts"]))
        .column(Column::date("ordered_on", "Ordered"))
        .column(Column::custom("trend", "Last 7 days"))
        .column(Column::custom("actions", "").frozen_right())
        .sort_by("-ordered_on")
}

async fn index(request: GridRequest) -> Result<View> {
    let page = orders_grid()
        .page(Order::query(), &request)
        .await?
        .extend(|order| json!({ "up": order.trend.last() >= order.trend.first() }));
    Ok(view("orders/index.html", context! { orders => page }))
}
```

- `Grid::new(id)`: the id may only have letters, digits, `_` and `-` (at most 64). It keys each
  user's column choices and the element's id (`grid-orders`). `page` returns an error for any
  other id.
- `.title("Orders")` puts a heading above the grid, next to the row count.
- `.per_page(n)` sets the rows per page before the user picks a size (default 25, clamped to
  1–500). The page-size menu offers 10, 25, 50 and 100, plus `n`.
- `.sort_by("-ordered_on")` sets the order before the user sorts: a column key, with `-` in
  front for descending, or several keys separated by commas (`"region,city,-total"`). Ties are
  always broken by `id`.
- `grid.id()` and `grid.columns()` read the definition back. `column.key()` and `column.kind()`
  (a `Kind`) do the same for a column.

## The handler

`GridRequest` is an extractor. It holds the query string, the session, the logged-in user
(for their column choices) and the database. `grid.page(query, &request)` applies the
request's filters, search, sort and page to `query` and returns a `GridPage`, which you pass
to the view as it is. Any `Query` works as the starting point, so the grid shows only what
the query allows (`Order::query().where_eq("team_id", team)`, or a default scope).

- `page.extend(|order| json!({ … }))` adds values to each row that aren't columns of the model,
  such as a chart's points or a link. The values must be an object, and its keys join the row's.
  `custom` columns read them.
- `page.items()` returns the models on the page, and `page.total()` the number of rows that
  match the filters across all pages.
- `grid.filter(query, &request)` returns the same filters and sort as a `Query`, for totals or
  your own export over every filtered row.
- `GridRequest::new(&db, path, &params)` builds a request for tests and commands (a guest,
  UTC). `request.param("export")` reads one query string value.

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid, GridRequest};
# #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, status: String }
# fn orders_grid() -> Grid {
#     Grid::new("orders").column(Column::select("status", "Status", [("new", "New"), ("paid", "Paid")]))
# }
# async fn demo(db: Db) -> Result {
let request = GridRequest::new(&db, "/orders", &[("in.status", "paid"), ("sort", "-id")]);
let page = orders_grid().page(Order::query(), &request).await?;
println!("{} of {} paid orders on this page", page.items().len(), page.total());
let paid = orders_grid().filter(Order::query(), &request).count(&db).await?;
# let _ = paid;
# Ok(())
# }
```

## The template

The `grid` macro of `renox/grid.html` draws a page. A call block draws the `custom` columns:
it gets the row (the model's fields, the related values and what `extend` added) and the
column.

```html
{% from "renox/grid.html" import grid %}
{% from "renox/ui.html" import link_button %}
<main class="rx-grid-fill">
  {% call(row, column) grid(orders) %}
    {% if column.key == "trend" %}<span class="{{ 'rx-up' if row.up else 'rx-down' }}">{{ sparkline(row.trend) }}</span>
    {% elif column.key == "actions" %}{{ link_button(route('orders.show', {'id': row.id}), "Open", size="small") }}{% endif %}
  {% endcall %}
</main>
```

- Without a call block, `custom` columns stay empty: `{{ grid(orders) }}`.
- `grid(page, tools=…)` puts your own HTML in the toolbar, next to the grid's own buttons, for
  example a "New" button. Capture it with a block `set`, so it isn't escaped:

  ```html
  {% set new_order %}{{ link_button(route('orders.create'), "New order", variant="primary", size="small") }}{% endset %}
  {% call(row, column) grid(orders, tools=new_order) %}…{% endcall %}
  ```

- The call block is also asked for two special keys. `column.key == "_details"` is the row's
  details (see [Row details](#row-details)). `column.key == "_empty"` gives the buttons under
  an empty grid's message (see [Empty grids](#empty-grids)).
- The macro adds the grid's stylesheet and script itself (`renox_grid()`). The layout should
  have `{{ renox_ui() }}` (the grid uses the kit's buttons) and `{{ toasts() }}` (for actions
  and edits that answer with a `Toast`).
- The grid needs the element around it to have a height. Put `rx-page--fill` on `<body>` (the
  page as tall as the screen, its `<main>` taking the rest under the kit's `navbar`) and
  `rx-grid-fill` on the `<main>`, as examples/grid does.
- `{{ sparkline(values) }}` draws a small chart as inline SVG: a line, or bars with
  `kind="bars"`. `width` and `height` are in pixels (96 × 28 by default), and `label` is what
  screen readers hear (by default "first → last"). It uses the text color, so `rx-up` or
  `rx-down` around it colors it.

## Columns

Each constructor takes the model's column key and the heading's label.

| Column | Shows | Heading filter |
|---|---|---|
| `text` | the text | contains / starts with / ends with / equals, or a pattern with `%` (`kop%`) |
| `number` (`.decimals(n)`), `money` | right-aligned, with the locale's separators; `money` is an amount in the smallest unit, shown with the `number` filter (no currency symbol, unlike the template filter `money`) | from–to |
| `date`, `datetime` | `2026-03-05`; `datetime` adds the time, shown in `APP_TIMEZONE` | a date range: two date fields and a calendar ([Cally](https://wicky.nillia.ms/cally/), bundled); a `datetime` range takes whole days of `APP_TIMEZONE` |
| `bool` | Yes / No (`.icons()`: ✓ / ✗) | yes, no |
| `select(key, label, options)` | the option's label | pick some |
| `tags(key, label, options)` | a JSON array (`Json<Vec<String>>`) as tags | rows with any of the picked |
| `image` | the image at the URL in the value (`.round()` for avatars) | none |
| `color` | a swatch of the CSS color, with its code | like text |
| `custom` | whatever the call block draws | none |

`options` are `(value, label)` pairs. Text filters ignore case. Every column kind except
`tags`, `image` and `custom` can be sorted by clicking its heading.

What every column takes:

- `.sortable(false)` and `.filterable(false)` turn off the heading's sort or filter. `custom`
  columns never have either.
- `.mobile()` shows the column on phones (under 768 px) until the user picks otherwise. When no
  column has it, the first three visible columns show. `.hidden()` hides the column on wide
  screens too, until the user turns it on.
- `.frozen()` and `.frozen_right()` keep the column at the left or right edge while the grid
  scrolls sideways (a column of actions on the right, for example).
- `.under(["Amounts"])` puts the column under a grouped heading. Headings can nest
  (`.under(["Sales", "Q1"])`). Neighbouring columns under the same headings share them, and a
  grouped heading never spans a frozen edge.
- `.width("14rem")` gives a minimum width in CSS units. `.decimals(n)` shows a number with
  exactly `n` decimals.
- `.badges(&[("paid", "success"), ("cancelled", "danger")])` shows `text`, `select` and `tags`
  values as badges. The tones are `success`, `warning`, `danger`, `info` and `neutral`. Values
  not listed get a neutral badge, and `.badges(&[])` makes every value a neutral badge.
- `.description("email")` shows another of the row's values in small type under this one, and
  `.tooltip("email")` shows it on hover.
- `.wrap()` lets long text wrap instead of widening the column. `.limit(40)` cuts text with `…`
  (the whole text shows on hover).
- `.link("/orders/{id}")` makes the value a link (`{id}` is the row's id). `.copyable()` adds a
  copy button.
- `.searchable()`, `.editable()`, `.merge()` and `.summary(…)` are covered below.

## Filters, search and chips

Each filterable heading opens a filter that suits its kind (the table above). All filters must
hold at once. On top of them:

- **Search.** Columns marked `.searchable()` put a search box in the toolbar. Every word typed
  must appear in one of those columns (as text, in any case). The grid searches as you type.
- **Chips.** The active filters, the advanced filter's rules and the search show as chips under
  the toolbar. × clears one, and "Clear filters" clears them all.
- **Only the grid's columns count.** The server reads only names that belong to the grid's
  filterable columns and their kind (and the query builder checks column names again).
  Anything else in the query string is ignored, not an error.

### The advanced filter

`.advanced_filter()` adds a button to the toolbar that opens a list of rules. Each rule is a
column, a condition that fits the column's kind, and a value. The rows must match all of the
rules, or any of them.

| Column kind | Conditions (`r.N.o`) |
|---|---|
| text, color, tags | `contains`, `not_contains`, `equals`, `not_equals`, `starts`, `ends`, `empty`, `not_empty` |
| number, money | `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `empty`, `not_empty` |
| date, datetime | `on`, `before`, `after`, `empty`, `not_empty` (whole days; of `APP_TIMEZONE` for datetime) |
| bool | `is_true`, `is_false` |
| select | `is`, `is_not`, `empty`, `not_empty` |

The rules live in the query string, as `match=any&r.0.c=total&r.0.o=gt&r.0.v=1000`, and apply
together with the headings' filters. There are at most 20 rules. A rule on an unknown column,
with a condition its kind doesn't have, or with a value that doesn't fit (a number that isn't
one) is dropped.

## Sorting and pages

A click on a sortable heading sorts by it ascending, a second click descending, and a third
returns to the grid's own order. `sort=-total`
in the query string sorts descending. Any sortable column is accepted, and so is `id` (`sort=id`
or `sort=-id`). Empty values sort last in either direction, on SQLite and PostgreSQL alike.
The pagination at the bottom has the page-size menu.

## The user's choices

The column menu (top right) shows and hides columns for the current screen size, moves them,
and freezes them at the left or right edge. With a mouse, a heading can also be dragged to move
its column. Dragging a heading's edge resizes the column, a double-click on the edge restores
the automatic width, and the arrow keys resize it once the edge has focus. "Reset columns"
returns to the grid's defaults.

- A logged-in user's choices are kept in `grid_preferences` (every app has the table), and a
  guest's in the session.
- The page saves them with `POST /_renox/grid/{grid}/prefs` (a JSON `GridPrefs`).
  `DELETE /_renox/grid/{grid}/prefs` forgets them (what "Reset columns" does).
- `GridPrefs` has `order`, `left`, `right` (frozen columns), `compact` and `wide` (the columns
  shown under and over 768 px) and `widths` (CSS pixels, 40 to 2000). `None` means the grid's
  default. `grid::save_prefs` writes them for a user, for example as a seeder's default:

```rust
# use renox::prelude::*;
use renox::grid::{self, GridPrefs};
# async fn demo(db: Db, user_id: i64) -> Result {
let mut prefs = GridPrefs::default();
prefs.wide = Some(vec!["number".into(), "customer".into(), "total".into()]);
grid::save_prefs(&db, user_id, "orders", &prefs).await?;
# Ok(())
# }
```

### Remembering filters

`.remember()` keeps the filters, the search, the sort, the grouping and the page size in the
session, so coming back to the page shows the grid as the user left it (and a cleared filter
stays cleared). The grid's form then sends `state=1`. A request with `state` saves its values,
and a request without it uses the saved ones. A link into a remembered grid that carries its
own filters should therefore include `state=1`.

## Row details

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid};
fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("number", "Order"))
        .row_url("/orders/{id}")
        .audit()   // created/updated by and at
        .details() // ...and the page's own `_details`
}
```

- `.row_url("/orders/{id}")` makes a click on a row (outside its buttons and links) open the
  URL. Ctrl/Cmd-click opens it in a new tab, and Enter opens a focused row.
- `.audit()` makes a click on a row (or the chevron in the row tools) open a row under it with
  the audit values the row has: `created_by`, `created_at`, `updated_by` and `updated_at`.
- `.details()` adds what the call block draws for `column.key == "_details"`, after the audit
  values when there are any.
- With details, a click on the row opens them, and `row_url` becomes a link in the row tools.

## Editing in place

```rust
# use renox::prelude::*;
use renox::Toast;
use renox::grid::{Column, Grid};
# #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, customer: String, items: i64 }

fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("customer", "Name").editable())
        .column(Column::number("items", "Items").editable())
        .edit_url("/orders/{id}") // PATCH with the changed fields
}

/// Only the edited fields arrive, so every field is an `Option`.
#[derive(serde::Deserialize, Validate)]
struct OrderEdit {
    #[validate(max = 100)]
    customer: Option<String>,
    #[validate(between(1, 999))]
    items: Option<i64>,
}

async fn update(State(db): State<Db>, Path(id): Path<i64>, Valid(edit): Valid<OrderEdit>) -> Result<Toast> {
    let mut order = Order::find_or_404(&db, id).await?;
    if let Some(customer) = edit.customer {
        order.customer = customer;
    }
    if let Some(items) = edit.items {
        order.items = items;
    }
    order.save(&db).await?;
    Ok(Toast::success("Order saved."))
}
```

- An editable cell opens an editor for its kind on a double-click, Enter or F2. Enter, or
  leaving the cell, saves; Escape cancels.
- The pencil in the row tools edits all of the row's editable cells at once (✓ saves, ✕
  cancels).
- Saving sends `PATCH {edit_url}` with only the edited fields, as a form (`customer=…`,
  `tags=a&tags=b`, `paid=true`). A 2xx answer (a `Toast` shows) reloads the grid's page. A 422
  from `Valid<T>` shows the errors in the cells.
- Cells are editable only when the grid has an `edit_url`. `custom` and merged columns can't be
  edited.

## Row order

`.reorder("position", "/orders/reorder")` lets rows be put in order while the grid is sorted
by that column ascending (`sort=position`). The handle in the row tools drags a row, with a
mouse or by touch, and the arrow keys move it. The page's new order is posted as
`ids=4,2,9&offset=25`, which `RowOrder::save` writes in one transaction (`updated_at` is left
alone). When the grid is sorted any other way, or grouped, the handle is off.

```rust
# use renox::prelude::*;
use renox::grid::RowOrder;
# #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, position: i64 }
async fn reorder(State(db): State<Db>, Form(order): Form<RowOrder>) -> Result<StatusCode> {
    order.save::<Order>(&db, "position").await?;
    Ok(StatusCode::NO_CONTENT)
}
```

## Merged cells

`.merge()` on a column draws one cell for neighbouring rows with the same value. Merged columns
nest from left to right: a column merges only within the groups of the merged columns before
it (Region, then City). Sort by them, as in `sort_by("region,city")`. Merging is off while rows
can be dragged and while rows are grouped, and merged columns aren't editable.

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid};
fn regions_grid() -> Grid {
    Grid::new("regions")
        .column(Column::text("region", "Region").merge()) // equal neighbours share a cell...
        .column(Column::text("city", "City").merge())     // ...nested in their region
        .column(Column::text("number", "Order"))
        .sort_by("region,city")
}
```

## Actions and selection

```rust
# use renox::prelude::*;
use renox::Toast;
use renox::grid::{Action, Column, Grid, GridRequest, Selection};
# #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, status: String }

fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("status", "Status"))
        .bulk_action(Action::new("Mark paid", "/orders/paid"))
        .bulk_action(Action::new("Delete", "/orders/bulk-delete").confirm("Delete the selected orders?").danger())
        .row_action(Action::link("Open", "/orders/{id}"))
        .row_action(Action::new("Delete", "/orders/{id}").method("DELETE").confirm("Delete this order?").danger())
}

async fn mark_paid(State(db): State<Db>, request: GridRequest, Form(selection): Form<Selection>) -> Result<Toast> {
    let changed = orders_grid()
        .selected(Order::query(), &request, &selection)?
        .update(&db, &[("status", &"paid")])
        .await?;
    Ok(Toast::success(format!("{changed} orders paid.")))
}
```

- `.bulk_action(…)` puts a checkbox at the start of each row and one for the whole page in the
  heading. Selecting rows shows the actions over the grid. Once the whole page is selected,
  "Select all N matching" selects every row the filters match.
- A bulk action sends `ids=4,7` and `all=true|false` to its URL, with the grid's query string.
  Read them with `Form<Selection>`. `grid.selected(query, &request, &selection)?` is the query
  for exactly those rows, or for every row the filters match when `all` is set.
- `.row_action(…)` puts actions in each row's ⋯ menu. `{id}` in the URL is the row's id.
- `Action::new(label, url)` sends a request (`POST` unless `.method("PATCH" | "PUT" |
  "DELETE")` says otherwise). `Action::link(label, url)` is a plain link. `.confirm("…")` asks
  first in a dialog, and `.danger()` shows the action in red.
- After a 2xx answer (a `Toast` shows), the grid reloads its page.

## Summaries and groups

- `Column::summary(Summary::Sum)` puts a figure in a footer that stays at the bottom of the
  grid. The figure covers every row the filters match, not just the page. `Sum`, `Average` and
  `Range` (smallest–largest) are for `number` and `money` columns. `Count` (rows with a value)
  works on any column. A column can have several (`.summary(Summary::Sum).summary(Summary::Average)`);
  a summary that doesn't fit its column is ignored.
- `.groups(&["region", "status"])` adds a "Group" choice to the toolbar, and
  `.group_by("region")` starts grouped. Rows come group by group, each with a heading that
  shows the value and the row count (a click folds the group), and a subtotal row with the
  group's own summaries. Group by the model's own columns.
- Merged cells and dragging rows are off while the grid is grouped.

## Empty grids

`.empty_state("No orders yet", Some("Orders show up here as customers buy."))` says what goes
in a grid that has no rows yet. The call block can add buttons for `column.key == "_empty"`. A
filtered grid with no rows says nothing matches and offers to clear the filters.

## Cards on phones

`.cards_on_mobile()` turns each row into a card under 768 px. A card shows the columns picked
for small screens as label and value, with the row tools underneath. Sorting and the column
filters move into the toolbar, since the headings are hidden there. Wider screens keep the
table.

## Columns from other tables

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid};
fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("number", "Order"))
        // customers.tier where customers.id = orders.customer_id
        .column(Column::related("tier", "Tier", "customers", "customer_id", "tier"))
        // how many order_notes have order_id = orders.id
        .column(Column::count_of("notes", "Notes", "order_notes", "order_id"))
        // the sum of order_lines.weight over the order's lines
        .column(Column::sum_of("weight", "Weight", "order_lines", "order_id", "weight"))
}
```

- `Column::related` shows a value of the row this one belongs to. It behaves like a text
  column; `.numeric()` makes it a number (filtered with a range).
- `Column::count_of` counts the rows that point at this one, and `Column::sum_of` adds up one
  of their columns. Both are number columns.
- They are SQL subqueries, so they sort, filter, search and take advanced-filter rules like
  other columns. A page fetches each one in a single query. Table and column names may only
  have letters, digits and `_`.
- `.summary(…)` does nothing on these columns (summaries cover only the model's own columns),
  and they can't be grouped by. `tags` filters also need a column of the model itself.

## Exports

`.exports()` adds an export menu to the toolbar. The handler answers its links before it draws
the page:

```rust
# use renox::prelude::*;
# use renox::grid::{Column, Grid, GridRequest};
# #[derive(Model, serde::Serialize, Default)] struct Order { id: i64, number: String }
# fn orders_grid() -> Grid { Grid::new("orders").column(Column::text("number", "Order")).exports() }
async fn index(request: GridRequest) -> Result<Response> {
    if let Some(file) = orders_grid().export(Order::query(), &request).await? {
        return Ok(file); // ?export=csv|xlsx|print
    }
    let page = orders_grid().page(Order::query(), &request).await?;
    Ok(view("orders/index.html", context! { orders => page }).into_response())
}
```

Each export holds every row the filters match, up to `grid::MAX_EXPORT_ROWS` (100,000), sorted
as on screen. It has the columns the user shows on wide screens; `custom` columns are left out.

- **CSV**: UTF-8 with a BOM (so Excel reads it), headings like `Amounts / Total`, labels for
  choices and tags, and numbers without separators. Text that starts with `=`, `+`, `-` or `@`
  gets a leading `'`, so a spreadsheet doesn't run it as a formula.
- **Excel** (the `xlsx` feature: `renox = { …, features = ["xlsx"] }`, through
  `rust_xlsxwriter`): grouped headings merged as on screen, numbers and dates stored as numbers
  and dates, and the headings and frozen-left columns frozen. Without the feature the menu has
  no Excel link, and `export=xlsx` is a 400.
- **Print**: a plain page (headings repeated on every printed page, landscape) with a button to
  print or save as PDF.

## Polling

`.poll(30)` reloads the grid's page every 30 seconds (at least 5) while the tab is visible and
nobody is editing, selecting or filtering in it.

## Several grids on a page

`.prefix("orders")` puts a prefix on the grid's query string names: `orders.page`,
`orders.q.number`, and so on. Each grid keeps the page's other query string values (another
grid's, a tab's) in its own links and forms, so the grids page and filter independently. Give
each grid its own id as well.

## The query string

Every value below belongs to one grid. With `.prefix("orders")`, each name starts with
`orders.`.

| Name | What it does |
|---|---|
| `page=2` | the page |
| `per_page=50` | rows per page; only the sizes the grid offers (10, 25, 50, 100 and its `.per_page(n)`) |
| `sort=-total` | sort by a sortable column or `id`; `-` for descending |
| `search=kopi susu` | the toolbar search; every word must appear in a searchable column |
| `q.number=A%` | a text filter (`%` makes it a pattern) |
| `m.number=starts` | how `q.` matches: `contains` (default), `starts`, `ends`, `equals` |
| `min.total=1000`, `max.total=…` | a number or money range |
| `from.ordered_on=2026-01-01`, `to.ordered_on=…` | a date range (`to` includes its day) |
| `in.status=paid` (repeated) | select and tags choices; bool uses `in.paid=1` / `in.paid=0` |
| `group=region` | group by one of `.groups`; empty for no groups |
| `match=all\|any`, `r.0.c=total`, `r.0.o=gt`, `r.0.v=1000` | the advanced filter's rules: column, condition, value |
| `export=csv\|xlsx\|print` | an export ([Exports](#exports)) |
| `state=1` | with `.remember()`: these values replace the remembered ones |

Only the grid's columns filter or sort. Anything else is ignored.

## Translations

The grid's texts (the toolbar, the filters, the column menu, the advanced filter's conditions,
the pagination) come in English and Indonesian. An app changes them in `lang/*.json` under
`ui.grid.*`, for example `ui.grid.columns`, `ui.grid.filter`, `ui.grid.search`,
`ui.grid.clear_all`, `ui.grid.empty`, `ui.grid.row_word` (`"row|rows"`) or `ui.grid.op.contains`.
The full list is in `crates/renox-core/src/i18n.rs`.

## Coming from Laravel and Filament

| Filament tables | Renox |
|---|---|
| `Table::make()->columns([...])` | `Grid::new("orders").column(…)` |
| `TextColumn::make('number')` | `Column::text("number", "Order")` (also `number`, `money`, `date`, `datetime`, `bool`, `select`, `tags`) |
| `ImageColumn`, `ColorColumn`, `IconColumn::boolean()` | `Column::image` (`.round()`), `Column::color`, `.icons()` |
| `->badge()->color(fn …)` | `.badges(&[("paid", "success")])` |
| `->description()`, `->tooltip()`, `->wrap()`, `->limit(40)`, `->url()`, `->copyable()` | `.description("email")`, `.tooltip("email")`, `.wrap()`, `.limit(40)`, `.link("/orders/{id}")`, `.copyable()` |
| `->searchable()`, `->sortable()`, `->toggleable()` | `.searchable()`, sortable by default (`.sortable(false)`), the column menu |
| `->defaultSort('ordered_on', 'desc')`, `->paginated([10, 25])` | `.sort_by("-ordered_on")`, `.per_page(n)` |
| `Filter` / `SelectFilter` / `TernaryFilter` | a filter in every heading, by kind |
| `QueryBuilder` filter | `.advanced_filter()` |
| `->persistFiltersInSession()`, `->poll('30s')` | `.remember()`, `.poll(30)` |
| `->recordUrl(…)`, `->emptyStateHeading(…)` | `.row_url("/orders/{id}")`, `.empty_state(…)` |
| `Action`, `BulkAction`, `->requiresConfirmation()` | `.row_action(Action::…)`, `.bulk_action(Action::…)`, `.confirm("…")` |
| `TextInputColumn`, `SelectColumn` | `.editable()` + `.edit_url(…)` |
| `->reorderable('position')` | `.reorder("position", url)` + `RowOrder::save` |
| `->summarize(Sum::make())`, `->groups([...])` | `.summary(Summary::Sum)`, `.groups(&[…])` / `.group_by(…)` |
| `->contentGrid([...])` (cards) | `.cards_on_mobile()` |
| `TextColumn::make('customer.tier')`, `->counts('notes')`, `->sum('lines', 'weight')` | `Column::related`, `Column::count_of`, `Column::sum_of` |
| `ExportAction` | `.exports()` + `grid.export(…)` |
| Column groups (`ColumnGroup`) | `.under(["Amounts"])` |

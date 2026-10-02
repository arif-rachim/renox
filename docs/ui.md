# Views, components and the UI kit

Renox pages are MiniJinja templates sent as HTML, updated in place with htmx. This guide
covers:

- components (macros that see the request);
- the UI kit that ships with Renox (`renox/ui.html`);
- toasts;
- fragments and out-of-band swaps;
- htmx response headers;
- live validation;
- stacks (`push` / `stack`);
- Tailwind CSS.

## Components see the request

A component is a MiniJinja macro in a file of its own, imported where it's used. Inside it,
the same request helpers work as in the page itself:

- `old()`, `error()`, `errors`;
- `t()`, `can()`, `auth`, `request`;
- `csrf_field()`, `flash`;
- `once()`.

```html
{# resources/views/components/price_field.html (rnx make:component price_field) #}
{% macro price_field(name, label) -%}
<div class="rx-field">
  <label class="rx-label" for="rx-{{ name }}">{{ label }}</label>
  <input class="rx-input" id="rx-{{ name }}" name="{{ name }}" inputmode="numeric" value="{{ old(name) }}"
    {%- if error(name) %} aria-invalid="true"{% endif %}>
  <p class="rx-error" data-error-for="{{ name }}">{{ error(name) }}</p>
</div>
{%- endmacro %}
```

```html
{% from "components/price_field.html" import price_field %}
{{ price_field("price", "Price") }}
```

`{% if once('datepicker') %}<script …>{% endif %}` is true only the first time a key is asked for
on a page. Use it for a component's script or style when the component appears several times.

## The UI kit

`renox/ui.html` is a set of components styled after Apple's Human Interface Guidelines. Add
its stylesheet and script to the layout, and the toast region to the body:

```html
<head>
  {{ renox_head() }}
  {{ renox_ui() }}
</head>
<body class="rx-page">
  <main class="rx-container">{% block content %}{% endblock %}</main>
  {{ toasts() }}
</body>
```

Then import what a page needs:

```html
{% from "renox/ui.html" import card, input, select, checkbox, button, link_button, form_errors %}
<form method="post" action="{{ route('products.store') }}" data-live-validate novalidate>
  {{ csrf_field() }}
  {% call card(title="New product") %}
    {{ form_errors() }}
    {{ input("name", "Name", required=true, hint="Shown on the product list.") }}
    {{ select("size", "Size", [["s", "Small"], ["m", "Medium"]], selected="m") }}
    {{ checkbox("featured", "Feature it on the home page", switch=true) }}
    <div class="rx-card__footer">
      {{ link_button(route('products.index'), "Cancel", variant="plain") }}
      {{ button("Create product") }}
    </div>
  {% endcall %}
</form>
```

| Component | What it is |
|---|---|
| `input(name, label, type=…, value=…, hint=…, required=…, autocomplete=…, placeholder=…, attrs={…}, id=…)` | A labelled text field with its hint and error. It is refilled after a failed submit, except for passwords. `id` tells apart two fields of the same name on one page. |
| `textarea`, `select(name, label, options, selected=…, placeholder=…)` | The same for longer text and for a choice. `options` are values or `[value, label]` pairs. |
| `checkbox(name, label, checked=…, switch=…)` | A checkbox, or an iOS-style switch for settings. The whole row is the target. |
| `button(label, variant=…, size=…, block=…)` | Variants: `primary`, `secondary`, `plain`, `danger` and `plain-danger`. The button shows a spinner while its form or htmx request is being sent. |
| `link_button(href, label, …)` | A link that looks like a button. |
| `card(title=…, subtitle=…)`, `group(title=…, footer=…)` | A surface, and an inset grouped list like Settings. |
| `alert(message, kind=…, title=…)`, `badge(text, kind=…)` | Kinds: `info`, `success`, `warning`, `error`. Each kind has its own icon, so color is never the only signal. |
| `form_errors(title=…)` | Every error of the last submit, above the form, each linking to its field. |
| `sheet(id, title)` + `open_button(id, label)` | A modal dialog. It closes on Esc and on a click on the backdrop, and focus returns to the button that opened it. On phones it rises from the bottom edge. |
| `confirm(id, label, action, title, message, confirm_label=…, method="DELETE")` | A destructive action behind a confirmation sheet, with Cancel focused first. |
| `menu(label)` + `menu_link`, `menu_action`, `menu_separator` | A menu of actions: arrow keys move, Esc closes. |
| `tabs(id, items, selected=…)` + `tab_panel(id, key, selected=…)` | A segmented control; the arrow keys, Home and End move between tabs. |
| `table(head, caption=…)` | A table in a card. A heading `["Total", "num"]` right-aligns its column, and `["Slug", "hide-narrow"]` hides it on phones. |
| `empty(title, message, action_href, action_label)` | What an empty list says, with the way to add the first item. |

Renox's own pages use the kit too: the sign-in pages (`renox/auth/*`: login, registration,
password reset, email verification, password confirmation, the account page) and the error
page. They have `stack('head')` and `stack('scripts')`; to change one, put a file with the same
name under `resources/views/renox/auth/`.

The kit's own texts ("optional", "Cancel", the error summary's title) come in English and
Indonesian. An app can change them in `lang/*.json`, under the keys `ui.optional`,
`ui.cancel`, `ui.close`, `ui.dismiss`, `ui.more` and `ui.errors_title`.

To change the markup or the styles, copy the kit into the app:

```sh
rnx make:component --ui   # my-app ui:publish: components/ui.html and public/css/renox-ui.css
```

Every class starts with `rx-`, and nothing in the kit styles bare elements, so it sits next
to an app's own CSS. To rebrand it, override the tokens on `:root`, e.g.
`--rx-accent: #0a7d5a;`.

### Design principles

The kit follows the Human Interface Guidelines. These are the rules its components enforce,
and worth keeping in an app's own pages:

- **Hierarchy.**
  - One primary button per form or page: the action people came to take.
  - Destructive actions in lists are red text (`plain-danger`); the filled red button waits in
    the confirmation sheet.
  - Secondary text is lighter and smaller, never lighter than WCAG AA allows.
- **Clarity.**
  - Labels are always visible; placeholders are examples, not labels.
  - A hint says what's expected before anyone gets it wrong.
  - Errors are plain sentences next to the field and in a summary above the form.
  - Optional fields are marked, since required ones are the norm.
- **Feedback.**
  - A button shows it's working (a spinner, `aria-busy`) and can't be pressed twice.
  - A toast confirms what happened and names the thing ("“Espresso” moved to the trash").
  - Success and info toasts leave on their own, but wait while hovered or focused; error
    toasts stay until dismissed.
- **Forgiveness.**
  - Destructive actions ask first, with Cancel focused.
  - Live validation checks a field when it's left, then as it's corrected. It never complains
    about a field someone is still typing in.
- **Deference.**
  - Content stays plain.
  - Translucent materials are only for what floats over it: the navigation bar, menus,
    toasts, the sheet's backdrop.
- **Accessibility.**
  - Contrast is at least 4.5:1 for text in both appearances. Apple's system blue (#007AFF)
    falls short under white text, so the accent is #0071E3.
  - Every control has a 44 × 44 pt target and a visible focus ring for keyboards.
  - Hints and errors are tied to their fields with `aria-describedby`, and errors are
    announced.
  - Menus, tabs and sheets work with the keyboard.
- **Respect for settings.**
  - Dark mode follows the system; `data-theme="light|dark"` on `<html>` overrides it.
  - Reduce Motion stops the animations, Reduce Transparency makes materials solid, and
    Increase Contrast darkens separators and secondary text.
  - Text sizes are in `rem`, so they follow the reader's settings.
  - Sheet and toast placement respect the notch and the home indicator (safe areas).

### Error pages

`rnx new` writes `resources/views/errors/default.html`: the layout with the kit's `empty`
component and a way home, so a 404 or a 403 keeps the navigation bar. `errors/<status>.html`
replaces it for one status; the page gets `status`, `reason` and `detail` (with `APP_DEBUG`
also `request_line` and `template`) besides the usual
globals. Renox's own error page (used when an app has none) is built on the kit too.

## Toasts

Return a `Toast` with the response:

```rust
use renox::prelude::*;
use renox::Toast;

async fn save() -> (Toast, Redirect) {
    (Toast::success("Product saved."), Redirect::to("/products"))
}
```

After a redirect, the toast waits in the session and `{{ toasts() }}` shows it on the next
page, once. For an htmx request it rides in `HX-Trigger` (event `renox:toast`) and appears at
once. Kinds: `success`, `info`, `warning` and `error`; `error` is announced as an alert and
stays until dismissed.

## Fragments and out-of-band swaps

`view(…).fragment("rows")` renders only that block for htmx requests. `.also("count")` adds
more blocks after it. Give each extra block's root element an `id` and `hx-swap-oob="true"`, and
htmx swaps it into the element with that id:

```html
{% block rows %}<tr id="order-{{ order.id }}">…</tr>{% endblock %}
{% block count %}<span id="order-count" hx-swap-oob="true">{{ count }}</span>{% endblock %}
```

```rust
# use renox::prelude::*;
# fn demo(count: i64) -> View {
view("orders/index.html", context! { count }).fragment("rows").also("count")
# }
```

## htmx response headers

The following types are response parts; return them in a tuple with the response:

- `HxRetarget("#errors")`: swap into another element;
- `HxReswap("outerHTML")`: swap another way;
- `HxPushUrl("/orders?status=open")`: update the address bar;
- `HxRedirect`, `HxRefresh` and `HxTrigger`.

## Live validation

Add `data-live-validate` to a form that `Valid<T>` handles. The kit's script then sends a
field's value (with the rest of the form) when the field is left, and again as it's corrected,
with the header `X-Renox-Validate: field`. The `Valid` extractor answers with that field's
errors as JSON and **the handler doesn't run**, so nothing is saved until the form is
submitted. The rules are the form's own, database checks such as `unique` included.

## The current route, conditional classes, loops

A navigation marks the current section with `route_is` (the route's name; `*` stands for
anything), and `class_names` builds a `class` attribute from the classes whose condition holds:

```html
<a href="{{ route('admin.products.index') }}"
   class="{{ class_names('tab', {'tab-active': route_is('admin.products.*')}) }}"
   {% if route_is('admin.products.*') %}aria-current="page"{% endif %}>Products</a>
{# request.route is the name itself, e.g. "admin.products.edit" #}
```

`route_is` also takes several patterns (`route_is('orders.*', 'checkout')`). In Rust, the
`CurrentRoute` extractor gives the same (`route.is("admin.*")`, `route.name()`). Loops may stop
early or skip items with `{% break %}` and `{% continue %}`:

```html
{% for product in recently_viewed %}{% if loop.index > 4 %}{% break %}{% endif %}…{% endfor %}
```

A text that depends on a count uses Laravel's plural ranges in the lang file, `{n}` for exactly
n, `[a,b]` for a range and `*` for no end: `"{0} Sold out|{1} Only one left|[2,5] Only :count
left|[6,*] :count in stock"`, printed with `{{ t('products.in_stock', count=product.stock) }}`
(examples/shop).

## Data grids

`renox::grid` is a table for dashboards and back offices: it fills its container (only the rows
scroll, the toolbar and the pagination stay put, on a phone too), each heading has a filter
that suits the column, and pages, sorting and filters come from the server and stay in the URL.
A grid is defined in Rust:

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

and drawn with the `grid` macro; a call block draws the `custom` columns:

```html
{% from "renox/grid.html" import grid %}
<main class="rx-grid-fill">
  {% call(row, column) grid(orders) %}
    {% if column.key == "trend" %}<span class="{{ 'rx-up' if row.up else 'rx-down' }}">{{ sparkline(row.trend) }}</span>
    {% elif column.key == "actions" %}{{ link_button(route('orders.show', {'id': row.id}), "Open", size="small") }}{% endif %}
  {% endcall %}
</main>
```

| Column | Shows | Filter |
|---|---|---|
| `text` | the text | contains / starts with / ends with / equals, or a pattern with `%` (`kop%`) |
| `number` (`.decimals(n)`), `money` | right-aligned, with the locale's separators | from–to |
| `date`, `datetime` | `2026-03-05` (a moment in `APP_TIMEZONE`) | a date range: two date fields and a calendar ([Cally](https://wicky.nillia.ms/cally/), bundled) |
| `bool` | Yes / No | yes, no |
| `select(key, label, options)` | the option's label | pick some |
| `tags(key, label, options)` | a JSON array (`Json<Vec<String>>`) as badges | rows with any of the picked |
| `custom` | whatever the page draws | none |

- **Columns per screen.** On a phone (under 768 px) the columns marked `.mobile()` show (the first
  three when none are); on wider screens all but `.hidden()` ones. The column menu (top right)
  shows and hides columns for the current screen size, moves them and freezes them left or
  right; a logged-in user's choices are kept in `grid_preferences` (every app has the table),
  a guest's in the session.
- **Search and active filters.** Columns marked `.searchable()` put a search box in the toolbar:
  every word typed must appear in one of them (as text, any case), and it asks as you type. The
  active filters and the search show as chips under the toolbar; × clears one.
- **Rows that open something.** `.row_url("/orders/{id}")` makes a click on a row open it
  (Ctrl/Cmd-click: a new tab; Enter on a focused row). With row details the click opens those
  and the row tools get a link instead.
- **Empty grids.** `.empty_state("No orders yet", Some("…"))` says what goes here; the call block
  can add buttons for `column.key == "_empty"`. A filtered grid with no rows offers to clear the
  filters.
- **Several grids on a page.** `.prefix("orders")` names the grid's values `orders.page`,
  `orders.q.number`, …; each grid keeps the page's other query string values (another grid's,
  a tab) in its own links.
- **Selecting rows and acting on them.** `.bulk_action(Action::new("Mark paid", "/orders/paid"))`
  puts a checkbox at the start of each row (and one for the page in the heading); selecting
  rows shows the actions over the grid, with "Select all N matching" once the whole page is
  picked. The action `POST`s `ids=4,7` and `all=true|false` (read with `Form<Selection>`) to its
  URL with the grid's query string, and `grid.selected(query, &request, &selection)?` is the
  query for exactly those rows, or for every row the filters match. `.row_action(…)` puts
  actions in each row's ⋯ menu (`{id}` in the URL; `Action::link` for a plain link).
  `.confirm("…")` asks first in a dialog, `.danger()` shows the action in red, `.method("DELETE")`
  picks the method. After a 2xx (a `Toast` shows) the grid reloads its page.
- **Summaries and groups.** `Column::summary(Summary::Sum)` (also `Average`, `Range`, and `Count`
  for any column) puts the figure in a footer that stays at the bottom of the grid, over every
  row the filters match. `.groups(&["region", "status"])` adds a "Group" choice to the toolbar
  (`.group_by("region")` starts grouped): rows come group by group, each with a heading (its
  value and row count, a click folds it) and a subtotal row with the group's own summaries.
  Merged cells and dragging rows are off while grouped.
- **Cards on phones.** `.cards_on_mobile()` turns each row into a card under 768 px: the
  columns picked for small screens as label and value, the row tools underneath, and sorting
  and the column filters in the toolbar (the headings are hidden there).
- **Kinds of cells.** `Column::image` (a URL; `.round()` for avatars), `Column::color` (a swatch),
  and on any column: `.badges(&[("paid", "success"), ("cancelled", "danger")])` (tones `success`,
  `warning`, `danger`, `info`, `neutral`), `.icons()` (yes/no as ✓/✗), `.description("email")`
  (another value under this one), `.tooltip("email")`, `.wrap()`, `.limit(40)` (cut with `…`,
  the whole text on hover), `.link("/orders/{id}")` and `.copyable()`.
- **Moving and resizing columns.** Drag a heading (with a mouse) to move its column, or move it
  in the column menu (on touch screens too); drag the edge of a heading to resize the column,
  double-click the edge for the automatic width, or focus it and use the arrow keys. Widths
  are kept with the other choices (`GridPrefs::widths`).
- **Grouped headings.** `.under(["Amounts"])`, or deeper (`.under(["Sales", "Q1"])`); neighbours
  under the same headings share them, and a heading never spans a frozen edge.
- **The query string.** `q.number=A%`, `m.number=starts`, `min.total=1000`, `max.total=…`,
  `from.ordered_on=2026-01-01`, `to.…`, `in.status=paid` (repeated), `sort=-total`, `page=2`,
  `per_page=50` (only the sizes the grid offers: 10, 25, 50, 100 and its `.per_page(n)`). Only
  columns of the grid filter or sort; anything else is ignored.
- `grid.filter(query, &request)` is the same filters and sort as a `Query`, for totals or an
  export over every filtered row. `GridRequest::new(&db, path, &params)` builds a request in
  tests and commands.
- `{{ sparkline(values) }}` draws a small line (or `kind="bars"`) chart as inline SVG; it takes
  the text color, so `rx-up` / `rx-down` around it color it.
- The grid needs only the page around it to have a height: `rx-grid-fill` is a flex child taking
  the rest of a column-flex `body` (`height: 100dvh`), as examples/grid does.

### Details, editing, row order and merged cells

```rust
# use renox::prelude::*;
use renox::grid::{Column, Grid, RowOrder};
# #[derive(Model, serde::Serialize, Default)]
# struct Order { id: i64, region: String, city: String, customer: String, position: i64 }

fn orders_grid() -> Grid {
    Grid::new("orders")
        .column(Column::text("region", "Region").merge()) // equal neighbours share a cell…
        .column(Column::text("city", "City").merge())     // …nested in their region
        .column(Column::text("customer", "Name").editable())
        .column(Column::number("position", "#"))
        .sort_by("region,city")
        .audit()                       // a click on a row: created/updated by and at
        .details()                     // …and the page's own `_details`
        .edit_url("/orders/{id}")      // PATCH with the changed fields
        .reorder("position", "/orders/reorder")
}

async fn reorder(State(db): State<Db>, Form(order): Form<RowOrder>) -> Result<StatusCode> {
    order.save::<Order>(&db, "position").await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- **Details.** A click on a row (or the chevron in the row tools) opens a row under it with the
  audit fields the row has (`created_by`, `created_at`, `updated_by`, `updated_at`) and, with
  `.details()`, what the page's call block draws for `column.key == "_details"`.
- **Editing.** An editable cell opens an editor for its kind on a double-click, Enter or F2;
  Enter or leaving the cell saves, Escape cancels. The pencil in the row tools edits all of the
  row's editable cells at once (✓ saves, ✕ cancels). Saving sends `PATCH {edit_url}` with only
  the edited fields as a form, so the handler reads them as `Valid<T>` with `Option` fields;
  a 2xx (a `Toast` shows) reloads the grid's page, a 422 shows the errors in the cells.
- **Row order.** While the grid is sorted by the order column ascending (`sort=position`), the
  handle in the row tools drags a row (mouse or touch), and the arrow keys move it; the page's
  new order is posted as `ids=4,2,9&offset=25`, which `RowOrder::save` writes in one transaction
  (`updated_at` is left alone). Sorted any other way, the handle is off.
- **Merged cells.** A merged column draws one cell for neighbouring rows with the same value,
  within the groups of the merged columns before it. Sort by them (`sort_by("region,city")`).
  Merging is off while rows can be dragged, and merged columns aren't editable.

### Exports

`.exports()` adds an export menu to the toolbar, and the handler answers its links first:

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

Each export holds every row the filters match (up to `grid::MAX_EXPORT_ROWS`), sorted as on
screen, in the columns the user shows on wide screens (custom columns left out):

- **CSV**: UTF-8 with a BOM (Excel reads it), headings like `Amounts / Total`, labels for
  choices and tags, numbers without separators; text starting with `=`, `+`, `-` or `@` gets a
  `'` so a spreadsheet doesn't run it.
- **Excel** (the `xlsx` feature, `renox = { …, features = ["xlsx"] }`, through
  `rust_xlsxwriter`): grouped headings merged as on screen, numbers and dates as numbers and
  dates, the headings and frozen-left columns frozen. Without the feature the menu has no Excel
  link and `export=xlsx` is a 400.
- **Print**: a plain page (headings repeated on every printed page, landscape) with a button to
  print or save as PDF.

## Stacks

A page or a component often needs something in another part of the layout: a script at the end
of `<body>`, a style or a `<meta>` in `<head>`. The layout names the places with `stack`, and
anything rendered for the page adds to them with `push`:

```html
{# layouts/app.html (rnx new's layout has both) #}
<head>… {{ stack('head') }}</head>
<body>… {{ stack('scripts') }}</body>
```

```html
{# components/chart.html #}
{% macro chart(id, data) -%}
{% call push('scripts', once='chart') %}
  <script src="{{ asset('js/chart.js') }}" nonce="{{ csp_nonce() }}"></script>
{% endcall %}
<canvas id="{{ id }}" data-points="{{ data | tojson }}"></canvas>
{%- endmacro %}
```

- `push(name)` adds to the end of the stack, `prepend(name)` to the front.
- `once='key'` adds it only the first time that key is pushed to that stack on the page, however
  many charts there are.
- Pushes work from the page's blocks, from included templates and from imported components,
  and they reach a stack that was rendered earlier (the head): the layout's head is written
  before the page's blocks run, so `stack` leaves a marker that's filled in once the page is
  done.
- htmx fragments (`.fragment("rows")`) have no layout, so what they push goes nowhere. Put a
  fragment's script in the fragment itself.
- Error pages (`errors/*.html`) have stacks too. Mails don't.

## Tailwind CSS

`rnx new shop --tailwind` sets it up; for an existing app, create `resources/css/app.css`:

```css
@import "tailwindcss";
@source "../views";
```

and link the output in the layout: `<link rel="stylesheet" href="{{ asset('css/app.css') }}">`.

- `rnx serve` runs Tailwind in watch mode next to the app: it rebuilds `public/css/app.css` when
  a view changes, and the page reloads.
- `rnx build` builds it minified before compiling, so an embedded binary carries it. `rnx tailwind`
  builds it once (`--minify`, `--watch`).
- There's no Node: `rnx` downloads Tailwind's standalone CLI (v4, the version `rnx` pins) once into
  your cache and checks its SHA-256. `rnx tailwind:install` does it ahead of time;
  `TAILWIND_BIN=/path/to/tailwindcss` uses another binary, `RNX_CACHE_DIR` moves the cache.
- Commit `public/css/app.css`: the Dockerfile from `make:deploy` builds the app without
  Tailwind.
- The kit's `rx-*` rules sit outside Tailwind's cascade layers, so the components keep their
  look, and utilities (`mt-4`, `text-sm`, `md:grid-cols-2`) lay out around them. Tailwind's reset
  changes bare elements (headings, lists, links without a class) in your own markup; style those
  with utilities or the kit's classes (`rx-title`, `rx-link`).

## Coming from Laravel

| Laravel | Renox |
|---|---|
| Blade components (`<x-input>`) | Macros in `resources/views/components/*.html` (`rnx make:component`) that see `old`, `error`, `t`… |
| Breeze's components | `renox/ui.html` (`rnx make:component --ui` to copy it) |
| `session()->flash('status')` + a toast library | `Toast::success(…)` and `{{ toasts() }}` |
| `@once` | `{% if once('key') %}` |
| `@push('scripts')` / `@stack('scripts')`, `@pushOnce`, `@prepend` | `{% call push('scripts') %}…{% endcall %}` / `{{ stack('scripts') }}`, `push(…, once='key')`, `prepend` |
| Vite + Tailwind | `rnx new --tailwind`: Tailwind's standalone CLI in `rnx serve` / `rnx build` |
| `@fragment` / `fragments([...])` | `.fragment("rows").also("count")` |
| Precognition (live validation) | `data-live-validate` and `Valid<T>` |
| `@class(['tab', 'active' => $on])` | `class_names('tab', {'active': on})` |
| `request()->routeIs('admin.*')` | `route_is('admin.*')` (and `request.route`, the route's name) |
| `@break` / `@continue` in `@foreach` | `{% break %}` / `{% continue %}` |

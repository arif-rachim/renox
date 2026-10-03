# Data grid example

A sales dashboard on one data grid (`renox::grid` and the `grid` macro of
`renox/grid.html`), to see what the grid does on a desktop and on a phone.

```text
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run -- db:seed    # 480 orders, and demo@example.com / password
cargo run               # http://127.0.0.1:3000
```

Anyone can browse, filter and export. Changing orders (editing in place,
dragging rows, the bulk actions and Delete) needs a login: log in as
demo@example.com / password and the tools appear (`orders_grid(can_edit)`;
the routes that change data sit behind `require_auth`).

## What to try

- **Phone and desktop.** Narrow the window under 768 px: the grid shows three
  columns (`Column::mobile()`); wider, all of them. The column menu (top right)
  shows and hides columns for the screen you're on, so a phone and a desktop
  keep different sets.
- **Only the rows scroll.** The toolbar and the pagination stay where they are;
  the headings stick to the top, the frozen columns (Order on the left,
  Actions on the right) to the sides.
- **Move and resize columns.** Drag a heading to move its column; drag the
  edge of a heading to make it wider or narrower (double-click the edge for
  the automatic width).
- **Freeze and reorder.** In the column menu, pick Left or Right for a column,
  or move it up and down. Logged in, your choices are saved for you
  (`grid_preferences`); as a guest, in the session.
- **Search.** Type in the box at the top: every word must be in the order
  number, name, email or city. Active filters and the search show as chips;
  × removes one.
- **Select and act.** Tick rows (or the heading's box for the page, then
  "Select all … matching") and mark them paid or shipped, or delete them
  after a confirmation; each row's ⋯ menu opens or deletes that order.
- **Totals and groups.** The footer adds up Items and Total and shows the
  discount range over every filtered order; "Group" in the toolbar groups the
  orders by region, status or paid, each group with its own subtotal.
- **Cards on a phone.** Under 768 px each order is a card; sort and filters
  move to the toolbar. Status shows as colored badges, Paid as ✓/✗, the
  customer with an avatar and the email under the name, and the order number
  with a copy button.
- **Other tables, advanced rules.** Tier comes from `customers` and Notes counts
  `order_notes` rows; both sort and filter. The sliders button builds rules
  ("Total > 30,000,000", "Tier is gold", "Email is empty"), all or any. Leave
  and come back: the grid remembers its filters; it also refreshes itself
  every 30 seconds when nobody is busy with it.
- **Filters by kind.** Text (contains, starts with, ends with, equals, or a
  `%` pattern), number ranges, the Ordered column's date range calendar,
  choices for Region, Status and Paid, and any-of for Tags. The URL keeps
  them, so a filtered list can be bookmarked or shared.
- **Grouped headings.** Customer, Location, Amounts and Charts span their
  columns (`Column::under`).
- **Cells of any kind.** A sparkline (`{{ sparkline(row.trend) }}`, green when
  sales went up), a progress bar and an Open button, drawn by
  `resources/views/orders/index.html` for the `custom` columns.

- **Details.** Click a row: who created it and who changed it last, and when.
- **Edit in place.** Double-click a cell (or focus it and press Enter) in Name,
  Status, Tags, Items, Total, Discount, Ordered or Paid; Enter saves, Escape
  cancels. The pencil edits the whole row. Wrong values (Items over 999) show
  their error in the cell.
- **Drag rows.** Show the # column and sort by it: the handle at the left of
  each row drags it, or moves it with the arrow keys.
- **Merged cells.** *By region* sorts by region and city: equal neighbours share
  one cell, cities nested in their region; a row's details fit between them.

- **Two grids on one page.** *Follow up* shows unpaid orders and the largest
  orders. Each grid's query string names start with its prefix
  (`unpaid.page`, `largest.max.account`), so paging or filtering one leaves
  the other where it was. The order number is a link (`.link`), the
  customer's email shows on hover (`.tooltip`), long names are cut
  (`.limit`) or wrap (`.wrap`), Email and Total can't be filtered (and Email
  not sorted) from their headings, and Account # is a related column filtered
  as a number range (`.numeric()`). Ten rows a page (`.per_page(10)`).

- **Export.** The download button (top right) exports every row the filters
  match: CSV, Excel (this example turns on renox's `xlsx` feature) or a page to
  print or save as PDF.

## Files

- `src/app/orders/mod.rs`: the grids (`orders_grid`, `regions_grid`,
  `unpaid_grid`, `largest_grid`), the page handlers (`index`, `regions`,
  `follow_up`, `show`), `update` (edits), `reorder`, `destroy`
  (the row menu's delete) and the bulk actions (`bulk_status`, `bulk_delete`).
- `src/app/orders/model.rs`: the `Order` model and its factory.
- `resources/views/orders/index.html`: the page and its custom cells.
- `resources/views/orders/regions.html`: the `/regions` grid with merged cells.
- `resources/views/orders/follow_up.html`: `/follow-up`, two prefixed grids
  (`unpaid_grid`, `largest_grid`) on one page.
- `resources/views/orders/show.html`: one order, opened from its row.
- `migrations/20260103000000_add_customers_and_notes.up.sql`: the `customers` and
  `order_notes` tables behind the Tier and Notes columns.
- `public/app.css`: the dashboard shell (a bar on top, the grid filling the rest).
- `tests/grid.rs`: the tests (below).

## Tests

```bash
cargo test -p grid
```

[tests/grid.rs](tests/grid.rs) covers pages and the custom cells, filters, rows that open their order, saved
columns, edits in place (and their errors), rows dragged into order, merged regions, exports
of every filtered row, bulk and row actions, totals and groups, and the related columns with
the advanced filter, and two prefixed grids paging apart on one page.

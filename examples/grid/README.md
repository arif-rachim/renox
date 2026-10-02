# Data grid example

A sales dashboard on one data grid (`renox::grid` and the `grid` macro of
`renox/grid.html`), to see what the grid does on a desktop and on a phone.

```text
cargo run -- migrate
cargo run -- db:seed    # 480 orders, and demo@example.com / password
cargo run               # http://127.0.0.1:3000
```

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

- **Export.** The download button (top right) exports every row the filters
  match: CSV, Excel (this example turns on renox's `xlsx` feature) or a page to
  print or save as PDF.

## Files

- `src/app/orders/mod.rs`: the grids (`orders_grid`, `regions_grid`), the page
  handlers, `update` (edits) and `reorder`.
- `src/app/orders/model.rs`: the `Order` model and its factory.
- `resources/views/orders/index.html`: the page and its custom cells.
- `public/app.css`: the dashboard shell (a bar on top, the grid filling the rest).
- `tests/grid.rs`: pages, filters, the custom cells and saved columns.

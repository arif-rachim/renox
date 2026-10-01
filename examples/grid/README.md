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
- **Freeze and reorder.** In the column menu, pick Left or Right for a column,
  or move it up and down. Logged in, your choices are saved for you
  (`grid_preferences`); as a guest, in the session.
- **Filters by kind.** Text (contains, starts with, ends with, equals, or a
  `%` pattern), number ranges, the Ordered column's date range calendar,
  choices for Region, Status and Paid, and any-of for Tags. The URL keeps
  them, so a filtered list can be bookmarked or shared.
- **Grouped headings.** Customer, Location, Amounts and Charts span their
  columns (`Column::under`).
- **Cells of any kind.** A sparkline (`{{ sparkline(row.trend) }}`, green when
  sales went up), a progress bar and an Open button, drawn by
  `resources/views/orders/index.html` for the `custom` columns.

## Files

- `src/app/orders/mod.rs`: the grid's definition (`orders_grid`) and the handler.
- `src/app/orders/model.rs`: the `Order` model and its factory.
- `resources/views/orders/index.html`: the page and its custom cells.
- `public/app.css`: the dashboard shell (a bar on top, the grid filling the rest).
- `tests/grid.rs`: pages, filters, the custom cells and saved columns.

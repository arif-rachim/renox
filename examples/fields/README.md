# examples/fields

Every common kind of form field, from the browser to the database and back into the edit form,
on SQLite and PostgreSQL. Each field of one `Product` shows one pairing of HTML input, Rust type
and column type. Read it with the table in [docs/types.md](../../docs/types.md) when you are
unsure which type to use.

```bash
cd examples/fields
cargo run -- migrate
cargo run                        # http://127.0.0.1:3000
```

For PostgreSQL, set `DATABASE_URL=postgres://...` as in [examples/postgres](../postgres); this
package already enables renox's `postgres` and `uuid` features.

## What's where

| Feature | Where |
|---|---|
| The model, the form struct with one comment per input, validation, routes | [src/app/products/mod.rs](src/app/products/mod.rs) |
| The form: text, textarea, number, checkbox, select, checkbox group, time, datetime-local, date | [resources/views/products/form.html](resources/views/products/form.html) |
| Column types on SQLite and on PostgreSQL | [.up.sql](migrations/20260101000000_create_products_table.up.sql), [.postgres.up.sql](migrations/20260101000000_create_products_table.postgres.up.sql) |

## Things worth copying

- **A `<select>` is an enum.** `#[derive(DbEnum)] enum Size` is stored as text (`small`,
  `medium`, `large`); the form lists `Size::ALL`, and an unknown value fails validation.
- **A list of checkboxes is a `Json<Vec<String>>`.** `JSONB` on PostgreSQL, `TEXT` on SQLite.
  The form field needs `#[serde(default)]`, since nothing is sent when no box is checked.
  Each item is checked with `v.each("colors", &self.colors, |c| c.one_of(COLORS))` and
  repeats are refused with `v.distinct("colors", &self.colors)`; errors are keyed `colors.1`,
  and `error('colors')` (or the `data-error-for="colors"` slot over htmx) shows the first.
- **A checkbox is a `bool`.** Checked sends `on`; unchecked sends nothing, which becomes `false`.
- **Empty inputs become `None`.** `Option<String>`, `Option<NaiveTime>`, `Option<NaiveDateTime>`
  and `Option<NaiveDate>` are `None` when the input is left empty.
- **UUIDs in URLs.** Pages use `public_id: Uuid`, not the sequential `id`
  (`/products/{public_id}/edit`).

## Tests

```bash
cargo test -p fields
```

The tests run on SQLite, or on PostgreSQL with `TEST_DATABASE_URL` set. They post every field,
check the stored values, and check the edit form shows each value in the format its input
expects. One test posts an unknown and a repeated color and checks the error lands on the
item's key (`colors.1`), and in the form's `colors` slot after a plain post.

# examples/fields

Every common kind of form field, from the browser to the database and back into the edit form,
on SQLite and PostgreSQL. Each field of one `Product` shows one pairing of HTML input, Rust type
and column type. Read it with the table in [docs/types.md](../../docs/types.md) when you are
unsure which type to use.

```bash
cd examples/fields
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run -- db:seed             # two products to open and edit
cargo run                        # http://127.0.0.1:3000
```

Try it: add a product with every field filled (a toast says it was created), edit it (a toast
says "Saved."), open it read-only from the list, and delete it from the list behind the
confirmation sheet.

For PostgreSQL, set `DATABASE_URL=postgres://...` as in [examples/postgres](../postgres); this
package already enables renox's `postgres` and `uuid` features.

## What's where

| Feature | Where |
|---|---|
| The model, the form struct with one comment per input, validation, routes, toasts after a save or delete | [src/app/products/mod.rs](src/app/products/mod.rs) |
| The list as a kit `table` (price with the `money` filter, badges for size and availability), an `empty` state, Edit and a `confirm` sheet for Delete | [resources/views/products/index.html](resources/views/products/index.html) |
| The form on the UI kit: text, a Markdown editor, number with a prefix or suffix, a switch, a radio group, a checkbox list, time, datetime-local, a date in the kit's calendar (`date_picker`), the key read-only with a copy button, a rich text editor and a JSON code editor (the `renox-editors` crate), in a `form_grid` and `fieldset`s | [resources/views/products/form.html](resources/views/products/form.html) |
| The product read-only, as an infolist: money, numbers with a suffix, Yes/No, a badge with labels, color swatches, tag badges, the specifications as a table, Markdown, rich text (`rich_text`), JSON in a `code_entry`, dates and `since`; buttons beside the key and the stock (`suffix_actions`) | [resources/views/products/show.html](resources/views/products/show.html) |
| The layout: the kit (`renox_ui()`), its `navbar`, `toasts()` | [resources/views/layouts/app.html](resources/views/layouts/app.html), [public/app.css](public/app.css) |
| Column types on SQLite and on PostgreSQL | [.up.sql](migrations/20260101000000_create_products_table.up.sql), [.postgres.up.sql](migrations/20260101000000_create_products_table.postgres.up.sql); tags and specifications, then details and settings added later: [migrations](migrations) |

## Things worth copying

- **Editors are plain fields.** The `renox-editors` module (`.module(Editors::new())` in
  [src/lib.rs](src/lib.rs)) gives the form a Markdown editor (`description`, a `String`), a
  rich text editor (`details`, read as `RichText`: the HTML is cleaned of scripts and event
  handlers as the form is read, and an emptied editor counts as empty) and a code editor
  (`settings`, a `String` checked with `.json()`). The product page shows `details` with the
  `rich_text` filter, which cleans it again, and `settings` with `code_entry`. Guide:
  [docs/editors.md](../../docs/editors.md).

- **Tags and pairs are JSON columns.** The kit's `tags_input` sends one `tags` value per tag (a
  `Vec<String>`, stored as `Json<Vec<String>>`); its `key_value` sends `specs[0][key]`,
  `specs[0][value]`… read into `renox::KeyValues` and stored as `Json<KeyValues>` (a list of
  pairs, so the order holds in PostgreSQL's `JSONB`). Such a nested name makes `Valid` read
  the whole form as a tree; every other field still parses from its text.

- **Toasts instead of flashed messages.** `store` and `update` return
  `(Toast::success(…), Redirect::route("products.edit", &[&product.id])?)`, and `destroy`
  returns `(Toast::success(…), Redirect::route("home", &[])?)`, since the product is gone. The
  toast waits in the session and the layout's `{{ toasts() }}` shows it on the next page, once.
- **A radio group (or a `<select>`) is an enum.** `#[derive(DbEnum)] enum Size` is stored as
  text (`small`, `medium`, `large`); the form lists `Size::ALL` with the kit's `radio`, and an
  unknown value fails validation.
- **A list of checkboxes is a `Json<Vec<String>>`.** `JSONB` on PostgreSQL, `TEXT` on SQLite.
  The form field needs `#[serde(default)]`, since nothing is sent when no box is checked.
  Each item is checked with `v.each("colors", &self.colors, |c| c.one_of(COLORS))` and
  repeats are refused with `v.distinct("colors", &self.colors)`; errors are keyed `colors.1`,
  and `error('colors')` (or the `data-error-for="colors"` slot over htmx) shows the first.
  The kit's `checkbox_list("colors", …, selected=product.colors)` ticks the stored colors, or
  after a failed save exactly the ones sent.
- **A checkbox is a `bool`.** Checked sends `on`; unchecked sends nothing, which becomes `false`.
- **Empty inputs become `None`.** `Option<String>`, `Option<NaiveTime>`, `Option<NaiveDateTime>`
  and `Option<NaiveDate>` are `None` when the input is left empty.
- **A UUID key.** `id: Uuid` makes the product's key a UUID v7, made on insert (the nil
  UUID of `Product::default()` means "not saved yet"). URLs show it
  (`/products/{id}/edit`) without revealing how many products there are, and
  `Product::find_or_404(&db, id)` takes it from `Path<Uuid>`. The column is `BLOB PRIMARY KEY`
  on SQLite and `UUID PRIMARY KEY` on PostgreSQL. `renox::uuid::Uuid` is renox's re-export.

## Tests

```bash
cargo test -p fields
```

The tests run on SQLite, or on PostgreSQL with `TEST_DATABASE_URL` set. They post every field,
check the stored values, and check the edit form shows each value in the format its input
expects. Others check the list (a table, or the empty state), the toasts after a create and a
save, and that Delete removes the row. One test posts an unknown and a repeated color and checks the error lands on the
item's key (`colors.1`), and in the form's `colors` slot after a plain post.

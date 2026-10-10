# Types: from the form to the database and back

A value in a web app travels a long way: someone types it into a form, your Rust code reads
it, and the database stores it. This page tells you which type to use at each step, so the
value arrives safely at every one.

### In this guide

- one big table: for each kind of form field, the Rust type and the database column to use;
- enums (a fixed list of choices, like a status);
- what browsers really send from forms, and how Renox reads it;
- advice for the tricky cases: money, time zones, keys, deleted rows and PostgreSQL.

### Words you'll meet

| Word | What it means |
|---|---|
| **form field** (or input) | A box, checkbox or menu in an HTML form, like `<input type="number">`. |
| **model** | A Rust struct that matches a database table: each field is a column, each value one row. |
| **column type** | What kind of value a database column holds: `TEXT`, `INTEGER`, `DATE`… |
| **SQLite**, **PostgreSQL** | The two databases Renox supports. SQLite is one file; PostgreSQL is a server. They name some types differently. |
| **migration** | A SQL file that creates or changes a table. It's where you write the column types. |
| **NULL**, **nullable** | NULL means "no value". A nullable column may hold NULL; in Rust that's `Option<T>`, and NULL is `None`. |
| **serde** | The Rust library that turns form data and JSON into structs and back. |
| **JSON** | A text format for structured data, like `["red", "blue"]` or `{"size": "L"}`. |
| **key** (or id) | The column that names a row, always called `id` in a Renox model. |

### The table

The form inputs are exercised by the bike shop's `/about/fields` page
([src/app/about/fields.rs](../examples/bikeshop/src/app/about/fields.rs): a form with every input, samples keyed
by `Uuid`, its tests run on SQLite and PostgreSQL). Every row is checked by Renox's own
tests, on SQLite and PostgreSQL.

How to read it: find the form field you're adding in the first column. The second column is
the type to give the field in your Rust form struct **and** in your model. The last two
columns are what to write in your migration's `CREATE TABLE`, for SQLite or PostgreSQL.

| HTML input | Rust type (form and model) | SQLite column | PostgreSQL column |
|---|---|---|---|
| `<input>`, `type=email/url/tel/search/password` | `String` | `TEXT` | `TEXT` |
| `<textarea>` | `String`, or `Option<String>` (empty → `None`) | `TEXT` | `TEXT` |
| `type=number` | `i64` (use `i64` for whole numbers; PostgreSQL has no unsigned types) | `INTEGER` | `BIGINT` |
| `type=number`, smaller ranges | `i16`, `i32` (also `i8` on SQLite: on PostgreSQL `i8` reads only the one-byte `"CHAR"` type, not `SMALLINT`; `u8`, `u16`, `u32` only in builds without the `postgres` feature, whatever database the app runs on: PostgreSQL has no unsigned columns) | `INTEGER` | `SMALLINT`, `INTEGER` |
| `type=number step=0.01` (measures) | `f64` | `REAL` | `DOUBLE PRECISION` |
| `type=number step=0.01`, single precision | `f32` | `REAL` | `REAL` |
| money | `i64` in the smallest unit (cents), never `f64` | `INTEGER` | `BIGINT` |
| `type=checkbox` (one) | `bool`: with `Valid<T>`, `on` (or `1`, `yes`, `checked`) → `true`, unchecked (nothing sent; or `off`, `0`, `no`) → `false` (see [Checkboxes](#checkboxes)) | `INTEGER` 0/1 | `BOOLEAN` |
| `<select>` | an enum with `#[derive(DbEnum)]` | `TEXT` | `TEXT` |
| `<select multiple>`, checkboxes sharing a name | `Vec<String>` with `#[serde(default)]`; stored as `Json<Vec<String>>` | `TEXT` | `JSONB` |
| `type=date` | `NaiveDate` | `TEXT` | `DATE` |
| `type=time` | `NaiveTime` | `TEXT` | `TIME` |
| `type=datetime-local` (no seconds needed with `Valid<T>`) | `NaiveDateTime` | `TEXT` | `TIMESTAMP` |
| (set by Renox) `created_at`, `updated_at` | `Option<DateTime>` (UTC), or a plain `DateTime` | `TEXT` | `TIMESTAMPTZ` |
| (set by Renox) `deleted_at`, with `#[model(soft_deletes)]` | `Option<DateTime>` (`None` = not deleted) | `TEXT` | `TIMESTAMPTZ` |
| `type=file` | `Upload`; store it and keep its key as `String` | `TEXT` | `TEXT` |
| the kit's `tags_input` | `Vec<String>` with `#[serde(default)]`; stored as `Json<Vec<String>>` | `TEXT` | `JSONB` |
| the kit's `key_value` (`meta[0][key]`, `meta[0][value]`) | `KeyValues` with `#[serde(default)]`; stored as `Json<KeyValues>` (a list of pairs, so the order holds) | `TEXT` | `JSONB` |
| the kit's `repeater` (`lines[0][name]`, `lines[0][qty]`) | `Vec<Line>` of a `Deserialize` struct, checked with `v.nested("lines", &self.lines)` | (a table of its own, or `Json<Vec<Line>>`) | |
| (structured data) | `Json<T>` for any serde type (`Json<serde_json::Value>` for free-form data) | `TEXT` | `JSONB` (or `JSON`, `TEXT`); a bare `serde_json::Value` field needs `JSONB` or `JSON` |
| (public ids) | `uuid::Uuid`, with renox's `uuid` feature (`renox::uuid::Uuid`) | `BLOB` | `UUID` |
| (sortable public ids) | `renox::db::Ulid` | `TEXT` | `TEXT` |
| (bytes) | `Vec<u8>` | `BLOB` | `BYTEA` |
| (secrets: an id number, a bank account, an API key) | `Encrypted<T>` (`renox::db::Encrypted`, any serde `T`), sealed with `APP_KEY` | `TEXT` | `TEXT` |
| any optional field | `Option<T>` | nullable | nullable |

Some words in the table, explained:

- **Rows in brackets**, like "(public ids)", aren't a form field: they're values your code or
  Renox sets.
- **"the kit's"** means a field from Renox's UI kit (ready-made form parts; see [ui.md](ui.md)).
- **`Option<T>`** is for any field that may be left empty. In the migration, leave out
  `NOT NULL` for that column.
- **`#[serde(default)]`** tells serde "if this field is missing, use an empty value". A list
  where nothing was picked is not sent at all; with `#[serde(default)]` it becomes an empty
  list.
- **`Json<T>`** stores any Rust value as JSON text in one column.
- **`JSONB`** is PostgreSQL's JSON column type.
- **`Upload`** is a file sent with the form. You store the file, then save its key (a name
  that finds it again) in a `String` column.
- **`Encrypted<T>`** is scrambled with the app's secret key (`APP_KEY`) before it's stored, so
  someone who reads the database can't read the value. In your app it's the plain value again,
  and it serializes as the plain value too: a model sent as `Json(model)` or passed to a
  template shows the secret. Only `Debug` hides it (`Encrypted(..)`). Keep such a field out of
  API answers and views (a separate struct for the output, or `#[serde(skip_serializing)]`).
- **A model field that isn't a column** gets `#[model(skip)]`: the model never writes it, and
  after a load it holds its `Default` value. Use it for values you fill in yourself, like a
  count loaded separately.
- **UTC** is the world's reference time zone, with no summer time.

## Enums

An enum is the right type for a `<select>` with a fixed list of choices, like a product's
status. Add `#[derive(DbEnum)]` and Renox can store it, read it from forms and print it:

```rust
use renox::prelude::*;

/// A product's status. Each variant is stored as text, in snake_case.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Default)]
enum Status {
    /// The status a new product starts with.
    #[default]
    Draft,          // stored as "draft"
    InStock,        // "in_stock"
    /// `rename` picks the stored text yourself.
    #[db(rename = "gone")]
    SoldOut,
}
```

What's going on:

- Each variant is stored as text: `Draft` as `"draft"`, `InStock` as `"in_stock"`.
- `#[db(rename = "gone")]` stores `SoldOut` as `"gone"` instead of `"sold_out"`.
- `#[default]` makes `Draft` the value of `Status::default()`.

The same enum is a model field, a form field (`<option value="{{ s }}">` for `s in Status::ALL`),
a JSON value and a template value, always as its text.

A value that isn't a variant is a validation error: someone who sends `status=stolen` gets an
error message, not a crash.

## Forms

Browsers don't always send what you'd expect. Here is what they send, and how Renox reads it.

### Checkboxes

Browsers send only checked checkboxes, and send them as `on`. `Valid<T>` (Renox's form checker)
reads that as `true`, and a missing one as `false`.

With `Valid<T>`, a `bool` field also accepts `1`, `yes`, `checked` and `true` (and `off`, `0`,
`no`, `false` or an empty value for `false`). So a hidden input or an API client (a program,
not a browser) can send either.

> [!WARNING]
> This is `Valid<T>`'s work. The plain `Form<T>` extractor (from the prelude) accepts only
> `true` and `false`: it refuses a checkbox's `on`, and a missing checkbox is an error unless
> the field has `#[serde(default)]`. Read forms with checkboxes through `Valid<T>`.

### Lists: multi-selects and checkbox groups

A multi-select or checkbox group repeats its name (`colors=black&colors=red`).

Declare the field as `Vec<T>` with `#[serde(default)]`, so that nothing chosen becomes an empty
list.

### Dates and times

`datetime-local` sends `2026-10-01T10:30`, without seconds. `Valid<T>` accepts it as a
`NaiveDateTime` (it adds `:00`); the plain `Form<T>` refuses it.

To show values back in an edit form, write them as they come. Dates, times and date-times
serialize in the formats their inputs expect (`2026-10-01`, `07:30:00`,
`2026-10-01T10:30:00`).

### Wrong values

A field that doesn't parse (`weight=heavy`, `size=huge`) is reported together with every other
field's errors. The person sees all their mistakes at once, not one at a time.

### Nested names: rows inside a form

Some fields have names with brackets: `lines[0][name]`, `meta[1][key]`. The kit's `repeater`
and `key_value` send them, one numbered row after another.

When a form has such names, `Valid` (Renox's form checker) reads the whole form as a tree:

- a `Vec` of structs, maps, and every type above still parsing from its text;
- errors are keyed with dots (`lines.0.name`), which is what `error('lines[0][name]')`,
  `old('lines[0][name]')` and the kit's error slots use;
- a row's field is labelled by its own name ("The name field is required."); translate it as
  `renox.validation.attributes.lines.*.name` or `.name`;
- empty values stay (an `Option` reads "" as `None`), so rows keep their numbers.

More on checking forms in [validation.md](validation.md).

## Choosing

Advice for the cases where the wrong type causes trouble later.

### Money

Use `i64` in the smallest unit, `price: i64 // cents`, and format it for display (the
`money` template filter, in `APP_CURRENCY`, which is `USD` by default). So 12.50 dollars is
stored as `1250` cents, and `{{ 1250 | money }}` shows `$12.50`. A currency without decimals
(IDR, JPY) stores the amount as it is.

Why not a float (`f64`)? Floats round: `0.1 + 0.2 != 0.3`. And why not a decimal type? sqlx
(the database library Renox uses) deliberately has no decimal type on SQLite, so a `Decimal`
would behave differently on the two databases.

### Time zones

- `DateTime` (UTC) is for moments something happened (`created_at`, `paid_at`).
- `NaiveDateTime` is for what a user typed in their local time (`launch_at`). "Naive" means it
  doesn't know its time zone. Convert it with `APP_TIMEZONE` when you need a moment.

### Keys

A model's `id` is its key, and its type picks how rows are numbered:

- `i64`: the database counts 1, 2, 3… (`INTEGER PRIMARY KEY AUTOINCREMENT` /
  `BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY`);
- `Ulid` (`TEXT PRIMARY KEY`) or `Uuid` (`BLOB` / `UUID PRIMARY KEY`): long random-looking
  ids, both made on insert and sortable by creation time;
- `String` (`TEXT PRIMARY KEY`), set by the app.

Pick a ULID or UUID when ids show up in URLs or APIs and shouldn't reveal how many rows there
are. (With counted ids, `/orders/52` tells everyone you have about 52 orders.)
The bike shop's `/about/fields` keys its samples by `Uuid`, and its rentals carry a `Ulid` code.

### Soft deletes

A soft delete hides a row instead of removing it, so it can be brought back.

Use `#[model(table = "products", soft_deletes)]` with a `deleted_at: Option<DateTime>` field
(the derive refuses `soft_deletes` without it). Then:

- `delete` sets `deleted_at` instead of removing the row;
- queries skip those rows (`with_trashed()` / `only_trashed()` to see them);
- `restore` clears `deleted_at`;
- `force_delete` really removes the row.

### PostgreSQL

> [!WARNING]
> Use `BIGINT` for `i64` columns. A plain `INTEGER` is 32-bit and won't read into an `i64`.

## Checking models against the tables

A model and its migration are written by hand, so they can drift apart: a column renamed in
one and not the other, `Option<String>` over a `NOT NULL` column, a `String` over a number.
`rnx db:check` builds the schema from your migrations on a scratch database and compares
every registered model with its table. Register the models you want checked:

```rust
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "products")]
struct Product {
    id: i64,
    name: String,
    price: i64,
    note: Option<String>,
}

fn app() -> App {
    // Registering the same model twice keeps one. In a module, use
    // `app.model::<Product>()` inside `register`.
    App::new().model::<Product>()
}
```

What is compared, for each model:

- the table exists, and every model column is in it;
- the table has no `NOT NULL` column without a default (and not the primary key) that the
  model lacks, because an insert would fail;
- the column's type fits the field's (table below);
- an `Option<T>` field sits over a nullable column, and a plain field over a `NOT NULL` one
  (`id` is exempt);
- a hand-written `impl Model` that gives no column info is reported, since there is nothing
  to compare.

| Field kind | SQLite (by affinity) | PostgreSQL |
|---|---|---|
| `i64`, `u32` / `i32`, `u16` / `i8`, `i16`, `u8` | INTEGER, NUMERIC | `bigint` / `integer` / `smallint` |
| `f64` / `f32` | REAL, NUMERIC | `double precision` / `real` |
| `String`, enums, `Ulid`, `Encrypted<T>` | TEXT | `text`, `character varying`, `character` |
| `Vec<u8>` | BLOB | `bytea` |
| `bool` | INTEGER, NUMERIC | `boolean` |
| `DateTime` | TEXT, NUMERIC | `timestamp with time zone` |
| `NaiveDateTime` | TEXT, NUMERIC | `timestamp without time zone` |
| `NaiveDate` | TEXT, NUMERIC | `date` |
| `NaiveTime` | TEXT, NUMERIC | `time without time zone` |
| `Json<T>`, `serde_json::Value` | TEXT, NUMERIC | `jsonb`, `json`, `text`, `character varying` |
| `Uuid` | BLOB, NUMERIC | `uuid` |

The command prints the problems grouped by model and exits with 1 when there are any, so it
fits a CI step. When all is well it prints `N model(s) match the schema.`

The scratch database depends on `DATABASE_URL`. With PostgreSQL, the migrations run in a fresh
schema on that server, dropped after the check. Otherwise they run on an in-memory SQLite
database. Your real data is never touched, and there is no flag to point it elsewhere.

A field whose type Renox does not know (a type of your own) skips the type check, and the
rest still runs. To have it checked too, say which kind of column it fills:

```rust
use renox::db::{ColumnKind, ColumnType};

struct Sku(String);

impl ColumnType for Sku {
    const KIND: ColumnKind = ColumnKind::Text;
}
```

`#[derive(DbEnum)]` already counts as text. To run the same check inside your tests, see
`assert_models_match_schema` in [the testing guide](testing.md).

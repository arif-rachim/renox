# examples/postgres

One small task list that runs on PostgreSQL and on SQLite with the same Rust code. Only
`DATABASE_URL` and one migration file differ. Read it when you want PostgreSQL, or want to see
what changes between the two databases.

Without any variables it runs on SQLite:

```bash
cd examples/postgres
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run                        # http://127.0.0.1:3000
```

On PostgreSQL (the package already enables renox's `postgres` feature):

```bash
createdb tasks && createdb tasks_test
DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks cargo run -- migrate
DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks cargo run
```

`DATABASE_URL` can also go in `.env`. See [docs/postgresql.md](../../docs/postgresql.md).

## What's where

| Feature | Where |
|---|---|
| Wiring | [src/lib.rs](src/lib.rs) |
| The model (`bool`, `Option<NaiveDate>`), routes, search and the overdue filter, the toggle | [src/app/tasks/mod.rs](src/app/tasks/mod.rs) |
| The SQLite migration and its PostgreSQL twin | [.up.sql](migrations/20260101000000_create_tasks_table.up.sql), [.postgres.up.sql](migrations/20260101000000_create_tasks_table.postgres.up.sql) |
| The page: search, add, toggle, pagination | [resources/views/tasks/index.html](resources/views/tasks/index.html) |
| `renox = { ..., features = ["postgres"] }` | [Cargo.toml](Cargo.toml) |

## Notes

- **One migration per database where the SQL differs.** On PostgreSQL the migrator uses
  `*.postgres.up.sql` when it exists, and the plain `*.up.sql` otherwise. Here `done` is
  `BOOLEAN` vs `INTEGER` and `due_on` is `DATE` vs `TEXT`; the model is the same.
- **Search ignores case on both.** `where_like("title", ...)` finds "Beli KOPI" for `?q=kopi`
  on SQLite and PostgreSQL alike.
- **Dates compare as dates.** `?overdue=true` uses `where_op("due_on", "<", today)` with a
  `NaiveDate`.

## Tests

```bash
cargo test -p postgres-app
TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/tasks_test cargo test -p postgres-app
```

Without `TEST_DATABASE_URL` the tests use an in-memory SQLite database. With it (in the
environment or in `.env`), each test gets a fresh schema in that PostgreSQL database. CI runs
them on PostgreSQL.

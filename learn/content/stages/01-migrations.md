---
title: Migrations
stage: 1
summary: Create the categories table with plain SQL, then let Renox write the menu items table from the model.
minutes: 20
---
The coffee app needs somewhere to keep its menu. In this stage you create two tables in the database: `categories`, which you write by hand in SQL, and `menu_items`, which Renox writes for you from a Rust struct. You also make the module that will hold the menu, and a test that proves the tables exist. You start from the app you ran in stage 0.

## What you'll learn

- What a migration is, and what the `up` and `down` files are for.
- How to write a migration by hand and run it.
- How to describe a table as a model and let `rnx make:migration --auto` write the SQL.
- How `rnx db:check` tells you when a model and its table disagree.
- How to undo the last migration and run it again.

## Make the menu module

An app is built from modules. Make one for the menu:

```bash
rnx make:module menu
```

```text
Created src/app/menu/mod.rs
Created resources/views/menu/index.html
Updated src/app/mod.rs
Updated src/lib.rs (.module(app::menu::Menu))
```

The command wrote a module with one route, `/menu`, and a page for it. It also added the module to `src/lib.rs`, so you do not have to remember to. The page is plain HTML with a few Renox tags:

```learn:include
file: resources/views/menu/index.html
```

`<rx-page>` puts the page inside the app's layout, the frame with the navigation bar. The `title` becomes the browser tab's title. Everything inside the tag is the page's content. You will fill this page in stage 3. For now it only has to exist.

## What a migration is

A database starts empty. A **migration** is a file that changes it: create a table, add a column, drop an index. You keep the files in `migrations/`, in order, so every copy of the app, yours, your teammate's and the server's, can build exactly the same database by running them.

Each migration is two files:

- `NAME.up.sql` makes the change.
- `NAME.down.sql` undoes it.

The name starts with a version, a number that sorts files into the order they ran. Renox's own tables (users, sessions and so on) have versions starting with `0001`, so they always run before yours.

## Write the categories migration

Ask `rnx` for an empty pair of files:

```bash
rnx make:migration create_categories_table
```

```text
Created migrations/20261011163238_create_categories_table.up.sql
Created migrations/20261011163238_create_categories_table.down.sql
```

The version is the time you ran the command, so yours differs from the one here. Rename both files to `20261012090000_create_categories_table`, so that your files have the same names as the ones this page shows. Then write the SQL that makes the table:

```learn:include
file: migrations/20261012090000_create_categories_table.up.sql
```

Every row of `categories` has an `id` that SQLite numbers for you, a `name` that must be filled in, and two columns, `created_at` and `updated_at`, which Renox keeps up to date. The last line is a unique index. An index makes searching a column fast, and a *unique* one also refuses a second row with the same value, so there can be only one category called Coffee.

The down file does the opposite:

```learn:include
file: migrations/20261012090000_create_categories_table.down.sql
```

Now run it:

```bash
rnx migrate
```

```text
Migrated: 20261012090000_create_categories_table
```

Only your migration ran, because the app had already run Renox's own when you started it in stage 0. Each run of `rnx migrate` is a **batch**: it remembers which migrations ran together, and `rnx migrate:status` lists them by batch.

## Describe the tables as models

A **model** is a Rust struct that stands for a table. One value of the struct is one row. You could write every query by hand, but with a model Renox knows the column names and types, and the compiler can check your code against them. You make a model with a command:

```bash
rnx make:model Category --module menu
rnx make:model MenuItem --module menu
```

Each command writes a file in `src/app/menu/` with an empty struct, and registers it. Fill the first one in:

```learn:include
region: menu.category-model
```

The `#[derive(Model, ...)]` line writes all the code that reads and saves a `Category`. The other derives, `Serialize`, `Deserialize`, `Default`, `Debug` and `Clone`, are what the generator always adds. `#[model(table = "categories", unique(name))]` names the table and says that `name` is unique, as the migration does. The two `Option<DateTime>` fields are the timestamps: `Option` because a row that was not saved yet has none.

The menu item has more to say:

```learn:include
region: menu.item-model
```

Read the fields from the top:

- `category_id` says which category the item belongs to. `references = "categories"` makes it a foreign key: the database refuses a number that is not the `id` of a category.
- `user_id` is who added the item, with the same kind of key to `users`.
- `description` and `stock` are `Option`, so they may be empty. For `stock`, `None` means "we do not count this", like a drink, and `Some(0)` means sold out.
- `price` is a whole number of cents. Money in a float goes wrong: 0.1 + 0.2 is not 0.3. 450 means $4.50.
- `default = "true"` is the value the database uses when a row does not say.
- `unique(name)` and the two `index(...)` entries ask for a unique index on the name and plain indexes on the two keys, because the app will look items up by category and by user.

Last, the module has to tell Renox about its models. The generator did that in `register`:

```learn:include
region: menu.register
```

## Let Renox write the second migration

You have the model, so do not write the SQL for `menu_items`. Ask Renox to compare the models with the database and write what is missing:

```bash
rnx make:migration --auto create_menu_items_table --yes
```

```text
Changes:
  create table `menu_items`
  create index `menu_items_name_unique`
  create index `menu_items_category_id_index`
  create index `menu_items_user_id_index`
```

`--auto` builds the database from your migrations in a scratch place, compares it with the registered models, and writes the difference as SQL files. `--yes` accepts the changes without asking. It wrote the same four files as before, plus two with `.postgres.` in their names for PostgreSQL, the database you can switch to later. Rename all of them to start with `20261012090100`, then read the main one. It is ordinary SQL, which you can edit:

```learn:include
file: migrations/20261012090100_create_menu_items_table.up.sql
```

Run it:

```bash
rnx migrate
```

```text
Migrated: 20261012090100_create_menu_items_table
```

> [!NOTE]
> `--auto` does not drop tables, and it cannot see a changed default or foreign key on a column that exists. For those, write the migration yourself, as you did for the categories.

## Keep the models and the tables in step

Two commands tell you when a model and its table disagree, for example after you add a field and forget the migration:

```bash
rnx db:check
rnx db:diff --yes
```

`db:check` lists the models that do not match their table. `db:diff` is what `--auto` runs: if you run it now it finds nothing to write.

## Undo and redo

The down files are what `rnx migrate:rollback` runs. It undoes the last batch, which is the menu items table:

```bash
rnx migrate:rollback
rnx migrate
```

```text
Rolled back: 20261012090100_create_menu_items_table
Migrated: 20261012090100_create_menu_items_table
```

A rollback is safe to try while you work. Do not run it on a database that holds data you need, because dropping a table drops its rows.

## Check it works

The stage comes with two tests in `tests/migrations.rs`. They count the rows of each table, which proves the tables exist, then roll the migrations back and run them again. Run them together with the rest:

```bash
cargo test
```

```text
running 2 tests
test migrations_create_the_menu_tables ... ok
test migrations_roll_back_and_run_again ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`rnx db:check` should also say that the models match the schema:

```text
2 models match the schema.
```

Test yourself:

```learn:quiz
id: migration-down
question: What does `rnx migrate:rollback` run?
choice: The `.up.sql` files of the last batch, again
correct: The `.down.sql` files of the last batch
choice: Every `.down.sql` file there is
explain: A rollback undoes the last batch only: it runs the down files of the migrations that batch ran.
```

## What changed

These are the files this stage added or changed in the app. Open one to see the lines.

```learn:changes
```

## Go deeper

[Types and migrations](https://docs.renox.rs/docs/types) lists every column type a model can have, the attributes that describe indexes, defaults and keys, and what `--auto` can and cannot do.

```learn:challenge
id: add-allergens
```

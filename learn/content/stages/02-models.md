---
title: Models
stage: 2
summary: Turn the tables into Rust structs and query them with columns the compiler checks.
minutes: 20
---
You have two empty tables and two structs that stand for them. In this stage you fill the tables with the coffee shop's real menu, then read it back in Rust. You write the queries with typed columns, so a misspelled column name stops the build instead of failing in front of a customer. You finish with tests that run every query against the seeded menu.

## What you'll learn

- What a seeder is, and how to make one that is safe to run twice.
- How to create rows with a model.
- How to filter and sort with typed columns such as `MenuItem::PRICE.lt(400)`.
- How to get the newest row first, and how to ask for one row by its id.
- How to save only the columns you changed.

## Seed the menu

An empty menu is hard to work with. A **seeder** is a function that fills the database with sample data, the same every time, so that you, your teammates and your tests all start from the same menu. This one adds two people, three categories and nine menu items.

```learn:include
region: seed.run
```

Read it in four parts.

First, the guard. It looks for Bea's account, and if it is there the function returns at once. That makes the seeder safe to run twice: the second run does nothing, instead of failing on the unique names or adding every item again.

Second, the people. `User::register` is the same function the registration page uses. It hashes the password and saves the row. Bea, the barista, will be the person who adds every item. Sam is there to have a second user.

Third, the menu itself, written as a plain list. Each entry has its category, name, description, price in cents, whether it is available, and its stock. Cold brew is not available, because it is back in summer, and the blueberry muffin has `Some(0)` left: it sold out. Drinks have `None`, since nobody counts espresso shots.

Fourth, the loop. The first time it sees a category, it creates it and remembers its `id`. Then it creates the item. `Category::create(db, Category { name: ..., ..Default::default() })` builds a `Category` with only a name and lets `Default` fill in the rest: an `id` of 0 means "not saved yet", and the database picks the real one. `create` saves the row and gives it back with its new `id` and timestamps.

A seeder is just a function with the app's state as its argument. Plug it into the app so that the `db:seed` command can find it:

```learn:include
region: lib.seeder
```

Then run it:

```bash
rnx db:seed
```

```text
Seeded.
```

Run it again. It prints the same word and changes nothing.

## Read rows with typed columns

Each field of a model has a matching constant on the struct, in capital letters: `MenuItem::PRICE`, `MenuItem::NAME`, `MenuItem::CATEGORY_ID`. They are **typed columns**. `PRICE` knows it holds an `i64`, so `MenuItem::PRICE.lt(400)` means "price is less than 400" and compiles only when 400 is a number.

Here is the first test of this stage. It asks for every item under $4.00, in alphabetical order:

```learn:include
region: models.typed-columns
```

Take it line by line. `MenuItem::query()` starts a query on `menu_items`. `.where_(...)` adds a condition. `.order_by(MenuItem::NAME)` sorts by name. `.get(db)` runs it and returns the rows as a `Vec<MenuItem>`. The test then collects the names and compares them with the four cheap items.

Why not write `"price < 400"`? A typo in a string is a bug you find at run time, perhaps in production. Misspell `MenuItem::PRICE` and the compiler refuses to build, and says which names exist. Rename a field and every query that used it stops compiling until you fix it. That is the point of typed columns.

> [!TIP]
> Rust's `assert_eq!` compares an array with a `Vec`, so a test can list the expected names right in the call. When it fails, the message shows both sides.

## The newest row first

A list of what was added last is common: recent orders, recent posts. `.latest()` sorts by `created_at`, newest first, and uses `id` to break a tie when two rows were saved in the same second.

```learn:include
region: models.latest
```

The test adds an item called Affogato and then reads the whole menu with `latest`. It checks that Affogato is first and that the table now has ten rows, the nine from the seeder and the new one. `MenuItem::create(db, item)` is the same function the seeder used.

## One row by its id

Pages like `/menu/7` need one row. `MenuItem::find(db, id)` returns an `Option`: `Some` when the row exists, `None` when it does not. Most of the time, a missing row should be a "not found" page, not a crash, so there is a shortcut:

```learn:include
region: models.find-or-404
```

`find_or_404` gives the row, or the error `Error::NotFound`. When a page's code returns that error, Renox shows the 404 page. The test checks both cases: item 1 is the Espresso, and item 999 is not found.

## Save what changed

A model you loaded can be changed and saved. `item.save(db)` writes the whole row. `item.save_changes(db, &original)` is more careful: you hand it the row as it was loaded, it compares the two, and it writes only the columns that differ. It returns `true` when it wrote something and `false` when nothing changed, in which case it sends no query at all.

The test `save_changes_a_row` in `tests/models.rs` loads the Espresso, clones it, changes the price of the clone to 325 and saves. It reads the row again to see the new price, then calls `save_changes` once more and checks that it answers `false`. Writing only what changed keeps two people editing different columns from overwriting each other.

## Check it works

The models test file starts every test from the seeded menu. It builds the app, then calls `app.kernel().seed()`, the same function `rnx db:seed` runs. Run the stage's tests:

```bash
cargo test --test models
```

```text
running 5 tests
test find_or_404_answers_not_found ... ok
test latest_comes_first ... ok
test save_changes_a_row ... ok
test the_seeder_runs_twice_without_duplicates ... ok
test typed_columns_filter_by_price ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The last test seeds the database twice and counts nine items, which proves the guard works.

```learn:quiz
id: typed-column-typo
question: What happens when you write `MenuItem::PRICEE.lt(400)`?
choice: The query runs and finds no rows
choice: The app starts, then fails when the query runs
correct: The code does not compile
explain: `MenuItem::PRICEE` is not a constant that exists, so the compiler stops before the app can run.
```

## What changed

This stage opened the `app` module to the tests, added the seeder and plugged it in.

```learn:changes
```

## Go deeper

[Relations](https://docs.renox.rs/docs/relations) shows how to load the category of every menu item in one query instead of one query per item, which is what the next stages need for the menu page.

```learn:challenge
id: cheap-items
```

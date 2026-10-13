---
title: Testing it all
stage: 8
summary: Test the pages, the form and the live component's actions with TestApp.
minutes: 25
---
Throughout the course you have run `cargo test` and watched tests pass. In this last stage you learn how those tests are built, so you can write your own. The app does not change: you add a factory that makes test records, five more tests for the counter, and one test that follows a person through the whole app from the sign-up page to a sale. By the end you will know the handful of calls that cover almost everything.

## What you'll learn

- What `#[renox::test]` and `TestApp::new` give every test.
- When to seed the demo data and when to use a factory.
- How `acting_as` signs a user in.
- The assertions for pages, redirects, forms and the database.
- How `TestApp::live` drives a live component.
- How to run one test, or all of them.

## A fresh app for every test

Every test in the course starts the same way:

```text
let app = TestApp::new(coffee::app()).await;
```

`#[renox::test]` replaces `#[tokio::test]` because the framework's macros need it. `TestApp::new` boots the real `coffee::app()`: the same modules, routes and migrations as production. It differs only where tests need it to. The database is in memory, so it is new for each test and gone afterwards; migrations have run; mail goes to a list you can read; there are no background workers; and no `.env` is read. Tests cannot disturb each other, and they run in parallel.

## Seeds and factories

An empty database is rarely what a test wants. There are two ways to fill it.

`app.kernel().seed().await` runs your seeder, the same demo menu you used in stage 2. It is good for tests that read the menu, because the data is realistic and every test sees the same one. The stock counter tests find Bea, Sam, the Croissant and the Blueberry muffin there.

A **factory** makes the record a test needs and nothing more. It is worth having when a test needs something the seed does not have, such as an item with exactly one portion left:

```learn:include
region: menu.factory
```

`impl Factory` has one method, `definition`, which returns a model with sensible values. `Category::factory().create_one(&db)` saves a "Coffee" category. Item names are unique, so the item's definition counts up: *Item 1*, *Item 2* and so on. A foreign key is a real constraint in the test database too, so an item cannot be saved without a category and a user. The test says which ones with `.state(...)`, a closure that changes the model before it is saved. The builder can also make several at once with `.count(3)`.

## One test, step by step

```learn:include
region: tests.live-factory
```

The test seeds the menu, so Bea exists and the "Coffee" category is taken. A state gives the new category another name. A second state attaches the item to that category and to Bea and sets its stock to 1. Then `acting_as(&bea)` signs Bea in, `app.live(StockCounter { item_id })` creates a driver for the component, and `call("sell")` presses the button. The answer is the component's HTML, so `assert_see("0 left")` can read it. Note that no browser was involved.

`TestApp::live` has four methods you have already met. `call(action)` runs an action with no arguments, `call_with(action, json!([...]))` passes arguments, `set(field, value)` adds a form field to every later call, and `component()` reads the state back. The stock counter's other tests use them to check the errors too: selling from an empty tray is a 400, a restock of 25 is a 400, and a drink that was not counted starts counting at 6.

Look again at the two tests from the last stage:

```learn:include
region: tests.live-sell
```

```learn:include
region: tests.live-forbidden
```

The first is the happy path, the second the refusals. For every action of a live component you want both: what it does for a person who may, and what it does for those who may not. A guest gets a 401 and Sam a 403, and the stock stays what it was.

## The assertions

You have used most of these in earlier stages:

| Call | Checks |
|---|---|
| `assert_ok()`, `assert_status(n)` | the status |
| `assert_not_found()`, `assert_forbidden()`, `assert_unauthorized()` | the common errors |
| `assert_see(text)`, `assert_dont_see(text)` | the body contains a text, or does not |
| `assert_view(name)` | which template answered |
| `assert_redirect(path)` | a redirect, and where |
| `assert_invalid(field)` | a validation error on the field |
| `app.assert_database_has(table, &[...])` | a row exists |

Two assertions work on the whole app and were in `tests/home.rs` since stage 0. `assert_models_match_schema` compares each model with the real table, so a column you forgot in a migration fails here. `assert_views_compile` loads every template and fails on a typo. They cost almost nothing and catch whole classes of mistakes.

## A journey

The tests above each look at one thing. A last test follows a person through all of it:

```learn:include
region: tests.journey
```

It registers Ana through `/register`, which also signs her in. She posts the form from stage 5 and the item is saved with her as the author. Her visit to the page shows *6 left*, and the live driver sells one with her session. Then the test looks at two places: the toast, which travels in the `HX-Trigger` header, and the database, where the stock is now 5. The author check from stage 7 lets her through because she is the author. If any of the stages broke, this test would fail, and it would name the step.

Journeys are slower than the others and show less when they break, so write a few. Most of your tests should be small and aimed at one rule.

## Running them

```bash
cargo test
```

```text
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

That is one file's summary: cargo prints one for each file in `tests/`, plus the app's own. To run a single file, name it with `--test`, for example `cargo test --test stock_counter`. To run a single test, add part of its name after `--`:

```bash
cargo test --test stock_counter -- guests_are_unauthorized
```

Add `--nocapture` to see what a test prints. When an assertion fails, the message shows the page it read, so you can see what the app really answered.

The checks that `learn/course/run.sh` makes, `cargo fmt --check`, clippy, `view:check` and `db:check`, are worth running before every commit of your own app, too.

```learn:quiz
id: acting-as
question: A test calls `app.acting_as(&bea)` and then `app.live(...)`. Whose session do the live calls use?
choice: A guest's, because live components are separate
correct: Bea's, because the driver sends the same session as the other requests
choice: A new user made for each call
explain: The live driver posts to the real route with the app's current session, so authorization runs exactly as in a browser.
```

## What changed

```learn:changes
```

## Go deeper

[Testing](https://docs.renox.rs/docs/testing) shows the fakes for mail, queues, events and time, and how browser tests fit in.

```learn:challenge
id: test-the-menu
```

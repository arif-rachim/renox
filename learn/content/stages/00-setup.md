---
title: Set up and run an app
stage: 0
summary: Install rnx, create the coffee app and see it answer in the browser.
minutes: 10
---
In this course you build one web app, a coffee shop's menu, from an empty folder to tested live components. This first stage is the shortest. You install Renox's command-line tool, make the app and see it answer in your browser. You need Rust 1.94 or later, and a little Rust: what a `struct` and a `fn` are, and what `async` and `?` do.

## What you'll learn

- What `rnx` is and how to install it.
- What `rnx new coffee` writes, and where each part of an app lives.
- How to start the app, open it in a browser and stop it.
- How to run the app's tests.

## Install rnx

`rnx` is Renox's command-line tool. It makes new apps, writes code for you and runs them while you work. Install it once, like any Rust program:

```bash
cargo install renox-cli
```

If something doesn't work later, `rnx doctor` says what this machine or the app is missing: a linker, a database, a setting. Run it now to see that your machine is ready.

```bash
rnx doctor
```

## Create the app

Make the coffee app, go into its folder and start it:

```bash
rnx new coffee
cd coffee
rnx serve
```

What these three commands do:

- `rnx new coffee` makes a folder called `coffee` with a working app inside.
- `cd coffee` goes into that folder. Run every command of this course from there.
- `rnx serve` builds the app and starts it.

The first build downloads and compiles everything Renox needs, so it takes a few minutes. After that, builds take seconds.

> [!TIP]
> Leave `rnx serve` running while you work. When you change a `.rs` file it rebuilds and restarts the app, when you change a template the browser reloads, and it runs the migrations before each start. A migration is a file that changes the database; you write the first two in the next stage.

## What rnx new made

Open the folder in your editor. These are the parts you will meet in this course:

- `src/lib.rs` builds the app. You will add to it in every stage.
- `src/main.rs` runs it. You will not change it.
- `src/app/` holds the app's modules. A module is a part of the app, like the home page or, soon, the menu.
- `resources/views/` holds the pages, written as HTML with a few extra tags.
- `migrations/` is empty for now. Your tables will go here.
- `tests/` holds tests that start the whole app and use it like a browser would.
- `.env.example` lists the settings the app reads. `rnx new` also copies it to `.env`, which is yours to change and is never committed.

Here is `src/lib.rs`, the heart of it:

```learn:include
file: src/lib.rs
```

Read it from the top. `renox::App::new()` starts an empty app. `.embed(...)` bakes the views and public files into the program, so one file is all you deploy. `.migrations(...)` adds the files in `migrations/`. Then each `.module(...)` plugs in a part of the app. The first is Renox's own login and registration. The second is the home page, in `src/app/home`.

The function is called `app()` and it only builds the app; it does not start it. That is why `main.rs` can run it, and why tests can start the same app without a network.

## Change a setting

An app's settings live in environment variables, and `.env` is the file that sets them while you work. Open it. You will see `APP_NAME`, `APP_PORT`, `DATABASE_URL` and a few more, each with a comment above it. `APP_KEY` is a long random secret that Renox uses to sign and encrypt cookies, and `rnx new` made one for you. Never share it or commit `.env`: the `.gitignore` the app came with keeps it out of Git.

Try it. Change `APP_PORT` to `3001`, stop the server with <kbd>Ctrl</kbd> + <kbd>C</kbd> if it is running, and start it again with `rnx serve`. It now answers on port 3001. Change it back to `3000` before you go on, because the rest of this course says port 3000. The point is that you change how an app runs without touching its code. Production servers use the same settings, set by the server instead of a file.

## See it in the browser

Open <http://127.0.0.1:3000>. You see a home page with a navigation bar and **Log in** and **Register** buttons, and they already work. Register an account. You come back to the home page, logged in, with your name in a menu on the right.

```learn:screenshot
file: home.webp
page: /
alt: The new app's home page: a navigation bar with Log in and Register buttons above a welcome message.
caption: Your new app's home page, the first time you open it.
```

The data lives in a SQLite database, one file at `storage/app.db`, made the first time it's needed. SQLite is a database that is a plain file, so there is nothing to install or start. Press <kbd>Ctrl</kbd> + <kbd>C</kbd> in the terminal to stop the server.

## Run the tests

The app came with tests. Open `tests/home.rs`: the first one starts the app, asks for the home page and checks the answer. Another registers a user, another checks that the models match the database, and one checks that every template compiles. You will add tests to every stage, so run them now, in a second terminal or after you stopped the server:

```bash
cargo test
```

## Check it works

The test run ends with five passing tests:

```text
running 5 tests
test every_view_compiles ... ok
test guests_can_register ... ok
test sessions_end_after_their_lifetime ... ok
test the_home_page_works ... ok
test the_models_match_the_schema ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The order of the lines can differ on your machine. If a test fails, run `rnx doctor` and read the first error: it usually names the missing piece.

Answer this before you go on:

```learn:quiz
id: where-it-runs
question: Where does `rnx serve` answer by default?
choice: http://localhost:8000
correct: http://127.0.0.1:3000
choice: http://0.0.0.0:80
explain: `APP_HOST` defaults to `127.0.0.1` and `APP_PORT` to `3000`; `.env` changes both.
```

## Go deeper

The [tutorial](https://docs.renox.rs/docs/tutorial) builds a different app from `rnx new` to deployment, with the same tools. Read it after this course, or alongside, to see them used another way.

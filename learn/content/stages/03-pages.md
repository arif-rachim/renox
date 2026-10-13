---
title: Routes and pages
stage: 3
summary: Add a route, read the menu in a handler and show them in a page made of rx- tags.
minutes: 25
---
Until now the coffee app has a database and a few Rust structs, but nothing a customer can open. In this stage you give the menu two pages: `/menu` lists what the shop serves today, and `/menu/7` shows one item. Each page has the same three parts. A route says which URL goes to which function. The function, a handler, reads the rows. A template turns the rows into HTML.

## What you'll learn

- How to add routes to a module and give each one a name.
- How a handler reads the database and hands its data to a page.
- What a typed page context is, and why `view:check` likes it.
- How to build a table with `<rx-table>` and the kit's other tags.
- How a missing item becomes a 404 page without extra code.

## Routes and their names

A **route** pairs an HTTP method and a path with a handler. Every module lists its routes in `fn routes`. Stage 2 left you with one, which only printed a heading; now it gets a real list page, and a second route for a single item.

```learn:include
region: menu.routes
```

`.get("/menu", index)` means "when a browser asks for `/menu`, run the function `index`". The `{id}` in the second path is a **parameter**: `/menu/7` runs `show` with the number 7.

Each route is followed by `.name(...)`. A name is how the rest of the app points at a route without writing its URL. A template says `route('menu.show', item.id)`, and Renox builds `/menu/7`. If you move the page to `/dishes/7` one day, you change one line here and every link follows. Names like `menu.index` also appear in the output of `rnx route:list`.

## A page context with a type

A handler does not build HTML. It gathers the values the page needs and gives them to a template. Instead of an anonymous bag of values, you can describe them with a struct:

```learn:include
region: menu.view-data
```

`MenuPage` says "the list page has `items`, a list of menu items". `ItemPage` says "the item page has an `item` and its `category`". `Serialize` turns the struct into the values a template reads, and `ViewData` ties each struct to its template with `#[view(template = "...")]`.

The tie is more useful than it looks. When the app starts, the module registers both structs:

```learn:include
region: menu.register
```

Now Renox knows which variables each template may use. The command `rnx view:check` compiles every template and fails when one reads a name its context does not have, so a typo such as `{{ item.nmae }}` stops the build instead of printing an empty string to a customer. It runs on its own in the stage's checks, and `cargo test` runs a similar check through `assert_views_compile`.

## The list handler

The first handler reads the menu.

```learn:include
region: menu.index
```

Read the query from the top. `MenuItem::query()` starts a query, as in stage 2. `.where_(MenuItem::AVAILABLE.eq(true))` keeps only what is on the menu today, so the Cold brew, which is out until summer, is not listed. The two `order_by` calls sort by category, then by name. `.get(&db)` runs the query.

The handler takes `State(db): State<Db>`. That is an **extractor**: you ask for what you need in the function's arguments, and Renox provides it. The function returns `Result<View>`. The `?` after `.await` turns a database error into an error page, and `View::typed(MenuPage { items })` renders `menu/index.html` with the struct's values.

## The item handler

```learn:include
region: menu.show
```

Here the arguments ask for two things: the database and `Path(id)`, the number from the URL. The handler finds the item, then its category. Both calls end in `_or_404`. When no row has that id, the call returns `Error::NotFound`, and the user sees the 404 page. The same happens for `/menu/abc`: `renox::Path` answers 404 when the segment cannot be a number, instead of an unhelpful 400.

## The page, made of tags

Open `resources/views/menu/index.html`. The page is HTML with tags that start with `rx-`. They belong to the UI kit that comes with Renox. Renox rewrites them into plain HTML when it loads the file, so there is nothing to install.

```learn:include
region: menu.table
```

`<rx-table>` draws a table. `:rows="items"` is the list to show. The colon means "this is an expression, not text": without it the table would receive the word `items`. `as="item"` names the variable each row is known by, and `:key="item.id"` tells the table what identifies a row.

Inside, each `<rx-column>` is one column, and what you write between its tags is the cell. The first cell links to the item's page with `route('menu.show', item.id)`. The second applies `money`, a filter that turns 450 into `$4.50`. Prices are stored in cents, and the filter divides them using the currency in `APP_CURRENCY`, which is `USD` by default. The column has `align="num"`, which lines numbers up on the right.

The `empty` slot is what shows when `items` is empty: a friendly message instead of a blank box. The whole table sits in `<rx-page>`, which wraps the page in the layout, and under `<rx-page-header>`, which draws the title and subtitle.

The item's page is shorter. Open `resources/views/menu/show.html`:

```learn:include
file: resources/views/menu/show.html
```

It shows the name, the category, the description and the price, using the same tags. Finally the layout got a link, so people can reach the menu: a `<rx-nav-link>` in `resources/views/layouts/app.html`, shown under *What changed* below.

```learn:screenshot
file: menu-list.webp
page: /menu
wait: table
alt: The menu page: a table listing each drink and pastry with its price.
caption: The menu, rendered from the MenuItem rows.
```

```learn:screenshot
file: menu-phone.webp
page: /menu
viewport: phone
wait: table
alt: The same menu page on a phone: the table fits the narrow screen.
caption: The same page on a phone.
```

> [!TIP]
> Run `rnx serve`, open `http://127.0.0.1:3000/menu`, and look at the page. Change a word in the template and save: the browser reloads by itself.

## Check it works

The page tests use the seeded menu from stage 2. One of them looks up the Croissant, then opens its page.

```learn:include
region: menu.item-page-test
```

Run them:

```bash
cargo test --test menu
```

```text
running 4 tests
test a_missing_item_is_a_404 ... ok
test an_item_has_its_own_page ... ok
test an_unavailable_item_is_not_listed ... ok
test the_menu_lists_available_items ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`app.get("/menu")` sends a request to your app without a network and a browser. `assert_view` checks which template answered, and `assert_see` checks the page contains a text. The other tests confirm that the Cold brew is missing and that `/menu/999` is a 404.

```learn:quiz
id: route-names
question: Why does the menu table link to `route('menu.show', item.id)` and not to `/menu/7`?
choice: The route function is faster than a plain link
correct: The link follows if the path changes, because only the route's definition holds it
choice: Plain paths do not work inside a table cell
explain: A name is defined once, next to the route. Templates and redirects ask for the name, so changing the path touches one line.
```

## What changed

This stage replaced the placeholder page with two real ones.

```learn:changes
```

## Go deeper

[Routing](https://docs.renox.rs/docs/routing) covers route groups, other methods, guards such as `require_auth`, and the extractors a handler can ask for.

```learn:challenge
id: category-page
```

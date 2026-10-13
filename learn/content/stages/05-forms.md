---
title: Forms and validation
stage: 5
summary: Add menu items through a form whose errors appear next to the fields, without a reload.
minutes: 25
---
So far the menu only comes from the seeder. Now Bea needs to add an item herself. A form is where an app meets the unexpected: an empty name, a price of minus ten, a category that does not exist. In this stage you build the form, check what it sends, and show each mistake next to the field that caused it. It works with JavaScript, with the page updating in place, and without it.

## What you'll learn

- How `#[model(form)]` makes a validated form struct from a model.
- What `Valid<T>` does before your handler runs.
- What an htmx form gets back when the input is wrong, and what a plain form gets.
- How to put the form behind login.
- What `live` adds to `<rx-form>`.

## Rules on the model

You already describe the menu item once, as a struct. The same struct can describe its form, too. Add `form` to the model's attributes, and write the rules on the fields:

```learn:include
region: menu.item-model
```

`#[model(..., form)]` makes Renox generate a second struct, `MenuItemForm`, with the fields people may send, and each `#[form(validate(...))]` lists that field's rules.

Read the rules as sentences. The category is required and must exist in `categories`. The name is required, at most 80 characters, and `unique("menu_items", "name")` makes sure no other item has it. The description may be up to 300 characters. The price is required, at least 1 and at most 100,000, in cents. The stock may be empty, but if it is there it must be between 0 and 999.

Two fields have no rules. `available` is a checkbox: a browser sends nothing when it is unchecked, and the generated form treats that as false. And `user_id` has `#[form(skip)]`: it is not part of the form at all. Nobody chooses who adds an item. The server does, from the logged-in user. This is also a safety measure. Someone who adds `user_id=1` to the request gets nothing, because the field is not in `MenuItemForm`.

Rules live in the model, the same place as the columns, so the page and the database cannot drift apart without you seeing it. They are the same rules as `#[derive(Validate)]` takes on any other struct. The form struct also has a `fill` method, which copies the checked values into a `MenuItem`.

## Routes behind login

Only staff add items, so the new routes need a login:

```learn:include
region: menu.staff-routes
```

`require_auth` is a guard: guests are sent to the login page. It wraps only the routes added **before** it, which is why the two staff routes are collected in their own `Routes` and `merge`d into the public ones. The public routes stay open. `GET /menu/new` shows the form and `POST /menu` receives it.

## The two handlers

```learn:include
region: menu.create
```

The first handler loads the categories as pairs of id and name, sorted by name. The select in the form needs them as its options.

```learn:include
region: menu.store
```

The second does the work, and look what it does not do: it never checks the input. `Valid(form): Valid<MenuItemForm>` is an extractor that reads the request, builds the form and runs its rules before the handler starts. If any rule fails, the handler is not called, and the user gets the response described below. When it is called, the data is good.

The handler starts from an empty `MenuItem::default()`, lets `form.fill` copy the fields in, and sets `user_id` from `AuthUser`, the logged-in user. `MenuItem::create` saves it. The return value is a pair: a success toast and a redirect to the new item's page. A toast is the small message that appears after an action, and the redirect means a reload of the page does not post the form again.

## Failing in two ways

What `Valid` returns on failure depends on who asked.

A form sent by htmx, which is how `<rx-form>` sends it, gets a **422** status with a JSON body that lists the errors by field name. The small script that ships with Renox reads it and writes each message in a slot under its input, marks the input as invalid and moves focus to the first one. The page does not reload, so everything the person typed stays where it was.

A plain form, sent by a browser with no JavaScript, gets a **303** redirect back to the form. The errors and the old input travel in the session for one request, and the kit's fields fill themselves in again. Passwords are never kept. You write one handler and get both behaviours.

## The form page

```learn:include
file: resources/views/menu/create.html
```

`<rx-form route="menu.store">` draws a form that posts to a route by name, and adds the hidden CSRF token that protects it. `novalidate` turns off the browser's own pop-ups, so that Renox's messages are the ones people see. `live` asks the page to check each field as soon as the person leaves it: the server runs the same rules and answers with that field's errors. `<rx-form-errors/>` is a summary for problems that belong to no single field.

The fields are the kit's: `rx-select` with `:options="categories"`, `rx-input` for the name, `rx-textarea` for the description, number inputs for the price and stock, and a switch for `available`. Each takes a label, a hint and, when the form fails, its own error. Nothing here handles errors by hand.

The index page also gained an *Add an item* button, shown with `rx-if="auth.check"` only to people who are logged in. See it under *What changed*.

```learn:video
id: s05-add-item
title: Adding a menu item: errors appear next to the fields
caption: The form posts with htmx. A 422 puts each error under its field, and nothing the person typed is lost.
```

```learn:screenshot
file: new-item.webp
page: /menu/new
login: bea@example.com
wait: form
alt: The Add an item form: a category select, name, description, price and stock fields, and a Save button.
caption: The form, empty, as Bea sees it.
```

```learn:screenshot
file: new-item-errors.webp
page: /menu/new
login: bea@example.com
wait: form
click: main form button[type=submit]
alt: The same form after Save with nothing filled in: a summary at the top and a red message under the Name and Price fields.
caption: Saved empty: the errors appear next to the fields, and nothing is lost.
```

## Check it works

Two tests cover the main paths. The first posts a valid item as Bea:

```learn:include
region: menu.store-test
```

The second sends bad input the way the browser does with htmx, and expects errors for both fields:

```learn:include
region: menu.errors-test
```

```bash
cargo test --test forms
```

```text
running 5 tests
test a_name_is_used_once ... ok
test a_valid_item_is_saved_with_its_author ... ok
test an_unknown_category_is_refused ... ok
test errors_come_back_next_to_the_fields ... ok
test guests_are_sent_to_login ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`assert_invalid("name")` checks that the response blames that field. The other tests confirm that guests are sent to the login page, that a name is used once, and that a category that does not exist is refused.

```learn:quiz
id: skip-user-id
question: Why is `user_id` marked `#[form(skip)]`?
choice: Because it is optional
correct: So a request cannot set it: the handler sets it from the logged-in user
choice: Because the database fills it in
explain: Fields marked `skip` are not in the generated form, so extra values in a request are ignored.
```

## What changed

```learn:changes
```

## Go deeper

[Validation](https://docs.renox.rs/docs/validation) lists every rule, shows how to write your own and how to translate the messages.

```learn:challenge
id: name-min-length
```

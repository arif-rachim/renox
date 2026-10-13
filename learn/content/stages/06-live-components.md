---
title: How live components work
stage: 6
summary: A part of the page whose state lives in a Rust struct: snapshot, action, render, morph.
minutes: 25
---
Some parts of a page have a memory. A drink builder remembers the size you picked, the number of shots and the name you typed, and every click changes the price. You could build it with a route for each button and a fragment for each answer, or you could write the state as a Rust struct and let Renox do the plumbing. That second way is a **live component**. In this stage you build one on the home page, and it never touches the database, so you can see how the mechanism works with nothing else in the way.

## What you'll learn

- The four steps of a live component: mount, snapshot, action, morph.
- How `rx-click` and `rx-submit` call a Rust method, with and without arguments.
- The three modes of `rx-model`, and which one to use when.
- The difference between `state` and `data`, and why it matters.
- What a live component costs you, so you can choose it with open eyes.

## The four steps

A live component is a struct and a template. Four things happen between them.

First, a page handler **mounts** the struct: it gives Renox the starting state and gets back the component rendered once. Second, the page carries a **snapshot**, which is the state, signed so it cannot be changed on the way. It sits in a `data-rx-snapshot` attribute on the component's wrapper. Third, when you click a button, the browser sends the snapshot and the name of the action to `POST /_renox/live/drink-builder/<action>`. The server checks the signature, rebuilds the struct, runs your method and renders the template again. Fourth, the answer is the new HTML of the wrapper, and the browser **morphs** the old one into it with a library called idiomorph. It touches only what changed, so the field you are typing in keeps its cursor.

You write the struct, the methods and the template. You write no route, no fragment and no JavaScript.

## The state

```learn:include
region: home.drink-builder
```

The three fields are the whole state: the size, the number of shots and the name on the cup. `Default` gives the starting point, a small drink with one shot and no name. The state derives `Serialize` and `Deserialize` because it travels in the snapshot, and it derives `Validate`, so the name has rules like any form: required, at most 30 characters, labelled "Name on the cup".

`#[renox::live_component(view = "live/drink_builder.html", name = "drink-builder")]` goes on the `impl` block, not on the struct, because a derive on the struct could not see the methods. It names the template and the component, and the name is the one in the URL.

Every method marked `#[live(action)]` is an action. It takes `&mut self`, the `LiveContext`, and then its own arguments, which are read by position from the call. A wrong number of arguments, or a value of the wrong type, becomes a 400 before your code runs. An action that is not in the list becomes a 404.

`size` refuses any word that is not small, medium or large with `Error::BadRequest`. The browser never sends one by itself, but anybody can send a request by hand, so the server checks. `add_shot` and `remove_shot` keep the number between 1 and 4 without complaint. `order` calls `ctx.validate(&*self)`, which runs the rules above and, on failure, answers with a 422 and the errors, which the page places next to the field. If the name is fine, `ctx.toast` sends a success message together with the answer.

## State and data

The price is not a field. Look at the first method in the component above: `data` returns extra values for the template, here the price in cents, worked out from the size (300, 375 or 450) plus 75 for every shot after the first. The template sees the struct as `state` and the result of `data` as `data`. The difference is important: **only the state is in the snapshot**. The price comes from the size and the shots, so keeping it in the state would only create a second place that could disagree with the first. Anything you can recompute, or read from the database, belongs in `data`.

## Mounting it on a page

```learn:include
region: home.mount
```

A handler takes a `LiveContext` like any other extractor, and `ctx.mount` turns the starting state into a value the template can print. `DrinkBuilder` must also be registered, once, where the module sets itself up:

```learn:include
region: home.register
```

Registering twice with one name stops the app at boot, so a typo cannot hide.

## The template

```learn:include
file: resources/views/live/drink_builder.html
```

The size buttons use `rx-click='size("small")'`. The text inside the parentheses is read as a JSON list, so a string needs double quotes, and that is why the attribute itself is in single quotes. `rx-click="add_shot"` has no parentheses and sends no arguments. The form has `rx-submit="order"`, so pressing Enter in the field calls the same action as pressing the button.

The home page shows the component with one tag, `<live-drink-builder :component="builder"/>`, placed inside its card:

```learn:include
file: resources/views/home/index.html
```

The tag takes no content. The part after `live-` is for the reader, and the component comes from `:component`.

```learn:screenshot
file: drink-builder.webp
page: /
wait: [data-rx-live="drink-builder"]
alt: The home page with the drink builder: size buttons, a shots counter and a price.
caption: The drink builder as the page first loads it.
```

```learn:screenshot
file: drink-builder-large.webp
page: /
wait: [data-rx-live="drink-builder"]
click: #size-large
alt: The drink builder after pressing Large: the large size is selected and the price is higher.
caption: After one click on Large. The page did not reload.
```

```learn:video
id: s06-drink-builder
title: The drink builder: every click sends the state and gets HTML back
caption: Each click posts the signed snapshot to the component's route and swaps in the HTML that comes back.
```

## Three ways to follow an input

The cup name has `rx-model.live="name"`. The `rx-model` attribute keeps a state field in step with an input, and it has three modes:

| Attribute | When the field is sent | Use it for |
|---|---|---|
| `rx-model` | with the next click or submit | forms, where the state changes when the person presses Save |
| `rx-model.live` | 300 ms after each keystroke | search boxes and previews |
| `rx-model.blur` | when the field changes and loses focus | a value checked when the person leaves it |

The two live modes call the built-in `_refresh`, which applies the fields and renders again without running an action. The builder uses `.live` because it shows *Ready for Ana* as the person types. With plain `rx-model` the line would appear only after the first click. Posted text is converted to the type the field holds: a number field becomes a number, a boolean accepts `true`, `1` and `on`, and a string stays a string.

## What the snapshot is, and is not

The snapshot is signed, not encrypted. It is `base64url` of the JSON, then a dot and an HMAC keyed from `APP_KEY`. The server trusts only what it signed, so nobody can change the size to "huge" and have it believed. But anyone who has the page can **read** the state. Keep secrets, tokens and other people's data out of the fields. The next stage uses this rule: the state will hold a number, the id of a menu item, and nothing else.

## What it costs

Live components are a good fit for a part of a page with several states. They are not free:

- There is no fallback without JavaScript. Without the script, `rx-click` does nothing.
- The whole component renders again on every call. Keep it small and keep `data` cheap.
- A component cannot contain another one. Use sibling components.
- The route has no rate limit, so check what an action does before you let it run expensive work.

For a form that must work everywhere, such as the one in stage 5, a normal form is still right.

## Check it works

Tests drive a component without a browser. `app.live(DrinkBuilder::default())` signs a snapshot, posts to the real route and keeps the new snapshot between calls:

```learn:include
region: home.builder-tests
```

`call_with` takes the action's arguments as a JSON list. `set` fills a form field that every later call sends, and `component()` reads the struct back from the last answer.

```bash
cargo test --test drink_builder
```

```text
running 8 tests
test an_unknown_size_is_a_400 ... ok
test choosing_a_size_changes_the_price ... ok
test ordering_needs_a_name ... ok
test ordering_with_a_name_shows_a_toast ... ok
test shots_stay_between_one_and_four ... ok
test the_home_page_mounts_the_builder ... ok
test the_name_follows_the_input ... ok
test unknown_actions_are_404 ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```learn:quiz
id: state-or-data
question: The drink's price depends on its size and shots. Where does the builder keep it?
choice: In the state, so it is in the snapshot
correct: In `data`, computed each time from the state
choice: In a cookie
explain: Only the state travels in the snapshot. A value you can recompute is returned by `data`, so it can never disagree with the state.
```

## What changed

```learn:changes
```

## Go deeper

[Live components](https://docs.renox.rs/docs/live) covers the context, events, the snapshot size limit and everything the browser tests prove.

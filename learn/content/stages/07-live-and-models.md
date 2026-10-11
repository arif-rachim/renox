---
title: A live component on a model
stage: 7
summary: A stock counter that keeps only the menu item's id, reads the model and saves it after checking the user may.
minutes: 25
---
The drink builder lived on its own. Most components are about a record: a stock counter for a menu item, a comment box for a post, a status switch for an order. They read the record, change it and save it, and now two questions come up that the builder never had. What should the component remember? And who is allowed to press the buttons? In this stage you add a stock counter to the item page and answer both, with a rule you can apply to every live component you write.

## What you'll learn

- Why the state keeps an id and not the record.
- How `data` reads the model each time the component renders.
- Why every action checks the user again, and how a `Policy` does it.
- What the browser sees for a 401, a 403 and a 400.
- How to mount a component on a page that already loads a model.

## Keep the id, read the rest

```learn:include
region: menu.stock-counter
```

Compare the state with the builder's. `StockCounter` has one field, `item_id`. It does not hold the name, the price, the author or the stock. There are two reasons.

The first is correctness. Another barista may sell a croissant in a second tab while this page is open. If the page carried the stock in its state, the component would work from a number that is already wrong. Reading the row on every call means the answer is always based on what the database says now.

The second is security, and it comes from the last stage: the snapshot is signed, not encrypted. Anyone with the page can read it, and prices or other people's data do not belong there. An id is public anyway.

`data` runs on every render. It finds the item with `find_or_404`, and returns it together with `can_update`, which asks whether the current user may change it. The template cannot do that itself, so the Rust side answers and the template only shows or hides. Note `ctx.user().is_ok_and(...)`: for a guest `ctx.user()` is an error, and the answer is simply "no".

## Authorize in every action

`sell` and `restock` begin the same way: they load the item and then call `ctx.user()?.authorize("update", &item)?`. This is the most important line in the stage.

It is tempting to think the template already took care of it, because the buttons are shown only when `can_update` is true. But the template is a courtesy, not a lock. A request does not have to come from your page: anybody can post to `/_renox/live/stock-counter/sell` with a snapshot they copied from a page they are allowed to see. The signature proves the snapshot came from the server. It does not prove that **this user** may do what the action does. So every action that changes something asks again.

If nobody is signed in, `ctx.user()?` returns `Error::Unauthorized`, a 401. If somebody is signed in but is not allowed, `authorize` returns `Error::Forbidden`, a 403. After the checks come the rules of the action itself. `sell` refuses a stock of nothing, or a stock that is not counted, with a 400 and the message *Nothing left to sell*. `restock` takes the number of portions as an argument and accepts only 1 to 24, so a tampered request cannot add a million croissants. An item with no stock, such as a drink, starts being counted when it is restocked: `stock.unwrap_or(0) + amount`.

On success both actions save the model and call `ctx.toast`. The toast says *Sold one Croissant* or, when the last portion goes, *Croissant is sold out*, and it travels with the answer to appear on the page.

## The policy

The question "may this user change this item?" is written once, next to the model:

```learn:include
region: menu.policy
```

`impl Policy for MenuItem` has one method, `allows`, which gets the user and the ability's name. Anyone can `view` an item. Only the person who added it can `update` it, which is a comparison of `user_id` with the user's id. Everything else, `delete` included, is refused, because the last arm says `false`. A refusal by default is the safe direction: an ability you forgot about is denied, not granted.

You can make this file with `rnx make:policy MenuItem --module menu`. It writes the skeleton with `"view" => true` and a comment about `update`, and adds `pub mod policy;` to the module. The same policy now serves two callers: `user.can(...)` answers a yes or no for the template, and `user.authorize(...)` turns a no into a 403 for the action.

## The template

```learn:include
file: resources/views/live/stock_counter.html
```

It shows *12 left*, or *Not counted* when the stock is none. The three buttons are inside a block with `rx-if="data.can_update"`, so a guest or another barista sees the number and nothing to press. The buttons call `sell`, `restock(6)` and `restock(12)`. Each has an `id`, so you and the tests can point at them.

```learn:screenshot
file: stock-counter.webp
page: /menu/7
login: bea@example.com
wait: [data-rx-live="stock-counter"]
alt: The Croissant's page signed in as Bea: 12 left, with Sell one and Restock buttons.
caption: The Croissant's page as Bea sees it.
```

```learn:screenshot
file: stock-counter-sold.webp
page: /menu/7
login: bea@example.com
wait: [data-rx-live="stock-counter"]
click: "#sell"
alt: The same page after pressing Sell one: the counter reads 11 left and a toast says Sold one Croissant.
caption: After one sale: 11 left, and the page did not reload.
```

```learn:video
id: s07-stock-counter
title: The stock counter: selling a portion and restocking
caption: Bea sells two portions and restocks six. Each action is authorized on the server before it changes a row.
```

## Mounting it with the item

The item page already loads the item and its category. It also mounts the counter:

```learn:include
region: menu.show
```

The handler takes a `LiveContext` next to the database and the path, and `ctx.mount(StockCounter { item_id: item.id })` makes the component. `ItemPage` gained a `counter` field of type `Mounted`, and the template prints it with `<live-stock-counter :component="counter"/>`. The component has to be registered once, with the module:

```learn:include
region: menu.register
```

## What the browser sees

When a live action fails, the answer is a normal HTTP error and htmx shows it as it would for any request. A 401 means the session has ended: the person should sign in again. A 403 means they are signed in but the policy refuses. A 400 is a bad request, such as selling an empty tray. In every case the database is unchanged, and the tests below prove it.

## Check it works

The first test signs in as Bea, the author of the seeded menu, and sells a croissant. The next ones prove that Sam, who added nothing, is refused, and that a guest is not even recognised:

```learn:include
region: tests.live-sell
```

```learn:include
region: tests.live-forbidden
```

`app.acting_as(&user)` signs the next requests in, including the ones `app.live` makes. A test that looks at the database afterwards checks the effect, not just the status.

```bash
cargo test --test stock_counter
```

```text
running 5 tests
test guests_are_unauthorized ... ok
test other_users_are_forbidden ... ok
test the_author_restocks ... ok
test the_author_sells_one ... ok
test the_item_page_mounts_the_counter ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```learn:quiz
id: authorize-in-actions
question: The sell button is only shown to the item's author. Is it safe to skip the check in `sell`?
choice: Yes, nobody else can press it
correct: No, anybody can post to the action's URL, so every action checks the policy
choice: Yes, because the snapshot is signed
explain: The signature proves the state came from the server. It says nothing about who is calling.
```

## What changed

```learn:changes
```

## Go deeper

[Live components](https://docs.renox.rs/docs/live#the-snapshot-and-its-security) explains the snapshot's limits, and [Authorization](https://docs.renox.rs/docs/authorization) covers policies, gates, roles and permissions.

```learn:challenge
id: sell-many
```

---
title: Your own components
stage: 4
summary: Write an <app-price-tag> once and use it on every page, with props the compiler checks.
minutes: 20
---
The menu page prints prices in a table cell, and the item page will print them again. Soon a third page will, and each copy will need the same styling and the same rules. In this stage you write the price once, as your own tag, `<app-price-tag>`. You also write a second tag that warns when an item is nearly sold out. Both tags are checked when the app builds.

## What you'll learn

- What a component is and where its file goes.
- How `<rx-props>` declares what a component accepts.
- The difference between `size="large"` and `:amount="item.price"`.
- How a slot passes content into a component.
- What the compiler tells you when you misspell an attribute.
- When to write a plain component, and when you need a live one.

Why bother? A price shown the same way in five places is five places to change when the shop adds a currency symbol, a discount or a screen-reader label. With a component there is one file to change, and every page follows. It is the same idea as a function: you give it a name, its inputs are its props, and the compiler checks every call.

## Make a component

A **component** is a template you can use as a tag. Its file lives in `resources/views/components/`, and the file name decides the tag: `price_tag.html` becomes `<app-price-tag>`. The prefix `app-` marks your own tags, as `rx-` marks the kit's. Let the generator create the files:

```bash
rnx make:component price_tag
rnx make:component stock_badge
```

```text
Created resources/views/components/price_tag.html
Use it with: <app-price-tag name="…" label="…">
Created resources/views/components/stock_badge.html
Use it with: <app-stock-badge name="…" label="…">
```

The generator writes a form field as an example. You replace it. Here is the price tag:

```learn:include
file: resources/views/components/price_tag.html
```

The first line is `<rx-props>`. It lists the component's **props**, the values it accepts. `amount` has no default, so it is required: leave it out and the build fails. `size="normal"` has a default, the text `normal`. The second line is the component. It prints a `span` whose class includes the size, then the amount through the `money` filter, then `<rx-slot />`.

The `rx-slot` is the place for the content a page writes between the tag's opening and closing parts. Without content the slot prints nothing.

A slot is how a component stays general. The price tag does not know whether a page wants "a cup", "each" or "per kilo", so it leaves a hole, and the page fills it. A component with more than one hole names them with `<rx-slot name="...">`, as `rx-table` does for its empty message.

## Text or expression

There are two ways to give a prop a value, and the colon tells them apart.

`size="large"` is text. The prop receives the characters `large`. `:amount="item.price"` is an expression. The prop receives whatever `item.price` evaluates to: a number, a list, an object. The same rule applies to the defaults inside `<rx-props>`, which is why the second component writes `:stock="none"`. The colon makes `none` the empty value, instead of the word.

```learn:include
file: resources/views/components/stock_badge.html
```

This one shows two badges, one for each case. `rx-if` prints its tag only when the condition is true, and `rx-elif` is the "else if". An item with `stock == 0` is *Sold out*. An item whose stock is a number up to 3 is *Only 3 left*. Drinks have no stock, so `stock is not none` is false for them, and nothing is printed. Items with plenty left print nothing either.

## Use the components

On the list page, the Price cell and a new Status column use the tags:

```learn:include
region: menu.table
```

The item page uses them as well, with a twist: for a coffee or a tea, the page says the price is for "a cup". That text is the slot's content, written between the opening and closing tags in `resources/views/menu/show.html`:

```learn:include
file: resources/views/menu/show.html
```

The page has two uses of the tag. `rx-if` and `rx-else` choose between them, so pastries get the plain version with no slot content.

The look comes from the app's own stylesheet. The kit has no price tag, so this is where it belongs:

```learn:include
region: menu.css
```

Use tabular figures so that prices line up, and make the large version bigger.

```learn:screenshot
file: menu-price-tags.webp
page: /menu
wait: table
alt: The menu with a Status column: a Sold out badge on the Blueberry muffin and an Only 3 left badge on the Cinnamon bun.
caption: The menu with prices in tabular figures and a status badge where one applies.
```

## Errors you see before a customer does

Open the index and misspell the attribute: `:amout="item.price"`. Run `rnx view:check`:

```text
menu/index.html:6: <app-price-tag> has no attribute "amout"; did you mean "amount"? It takes: amount, size
Checked 33 templates: 1 problems.
Error: 1 template problem(s)
```

The compiler read `<rx-props>` and knows what the component takes. It also names the closest match. The same check covers a missing required prop, and it runs on every template when the app builds in tests. Your own tags get the same safety as the kit's.

Try the other mistakes too, one at a time, and read what the compiler says. Remove `:amount` and it says `<app-price-tag>` needs the attribute "amount". Each message names the file and the line, so you fix it in seconds.

Your editor can help before you even run the check. `rnx view:data` writes `.vscode/renox-components.json`, a list of every tag and its props, and the editor settings in the project use it to complete `<app-` and its attributes.

## Plain or live

These components are **plain**. A plain component has no state and no actions: it turns the values it receives into HTML, and that is all. It cannot remember a count or react to a click.

When a part of the page must change after a click, without reloading, you need a **live component**, which keeps its state on the server. Chapter 6 introduces them. For a price or a badge, a plain component is the right size.

## Check it works

Tests for the components read the page and look for the HTML they produce:

```learn:include
region: menu.price-tag-test
```

```bash
cargo test --test components
```

```text
running 5 tests
test a_counted_item_with_plenty_shows_no_badge ... ok
test a_drink_is_priced_by_the_cup ... ok
test a_sold_out_item_says_so ... ok
test few_left_shows_the_count ... ok
test the_price_tag_shows_dollars ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```learn:quiz
id: text-or-expression
question: What does `<app-price-tag amount="item.price"/>` pass as the amount?
choice: The price of the item
correct: The text `item.price`, because there is no colon
choice: Nothing: the build fails
explain: Without a colon the value is text. `:amount="item.price"` is the expression that reads the item's price.
```

## What changed

```learn:changes
```

## Go deeper

[Your own components](https://docs.renox.rs/docs/ui) in the UI guide lists every prop form, named slots and how to move an older macro to a tag.

```learn:challenge
id: category-chip
```

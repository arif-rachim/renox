---
title: A chip for the category
stage: 4
files: resources/views/menu/show.html, resources/views/components/category_chip.html
test: challenge_category_chip
minutes: 20
summary: Write an app-category-chip component, a badge with a kind for each category, and use it on the item page.
---
The item page prints the category's name in its header, in plain text. Make it a small coloured chip. Write the component `<app-category-chip :name="..."/>`, in the file `resources/views/components/category_chip.html`. It takes a `name` prop and draws an `<rx-badge>` with the name inside, with a kind per category:

| Category | Kind |
|---|---|
| Coffee | `info` |
| Tea | `success` |
| Pastry | `warning` |
| anything else | the badge's default |

Then use it on the item page, `menu/show.html`, inside the card, passing the category's name. Look at `stock_badge.html` in the same folder for how a component with `rx-if` and `rx-elif` looks.

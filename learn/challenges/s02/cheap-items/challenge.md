---
title: Only the cheap items
stage: 2
files: src/app/menu/model.rs
test: challenge_cheap_items
minutes: 15
summary: Write MenuItem::cheap, the items under $3.00, cheapest first.
---
The menu board needs a short list of what a customer can get for under three dollars. Write `MenuItem::cheap(db)`. It returns the menu items that are **available** and cost **less than $3.00** (prices are in cents, so less than `300`), the **cheapest first**.

Use the typed columns from this stage: `MenuItem::AVAILABLE` and `MenuItem::PRICE` have methods like `eq` and `lt`, and `order_by` takes a column. The function already exists in the file below. It compiles, but it returns nothing yet.

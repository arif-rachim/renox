---
title: Sell several at once
stage: 7
files: src/app/menu/stock_counter.rs
test: challenge_sell_many
minutes: 25
summary: Add a sell_many(n) action to StockCounter, with the same authorization as sell.
---
A customer buys a whole tray. Add the action `sell_many(n)` to `StockCounter`: it takes away `n` from the item's stock, where `sell` takes away one.

- It authorizes exactly like `sell`: only the person who added the item may change its stock (a guest gets 401, another user 403). Authorize first, before you look at `n`.
- It answers **400** when `n` is below 1, or more than what is left. An item whose stock is not counted (`None`) has nothing left. The stock stays as it was.
- Otherwise it saves the new stock and shows a toast.

An action is a method marked `#[live(action)]`, and the arguments after the context come from the call. Look at `restock(amount)`, which has the same shape.

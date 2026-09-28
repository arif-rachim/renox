# examples/crud

The reference CRUD module: products listed with pagination, created by logged-in users, edited
and deleted only by their owner. Deletes are soft, so deleted products wait in a trash and can
be restored. Read it before writing your first resource module.

```bash
cd examples/crud
rnx key:generate                 # optional: keeps logins across restarts
cargo run -- migrate
cargo run -- db:seed             # demo@example.com / password123 and 25 fake products
cargo run                        # http://127.0.0.1:3000
```

Log in at `/login` (or register at `/register`), then add products at `/products/new`.

## What's where

| Feature | Where |
|---|---|
| Wiring: the `Auth` module, the products module, the seeder | [src/lib.rs](src/lib.rs) |
| Routes (public list, members-only create/edit/update/delete/trash/restore), form validation, handlers | [src/app/products/mod.rs](src/app/products/mod.rs) |
| The model with `soft_deletes` and `hooks` (`impl ModelHooks`: a slug, a check, a cache key forgotten), a factory, `for_owner` for seeders and tests | [src/app/products/model.rs](src/app/products/model.rs) |
| The policy: only the owner may update, delete or restore | [src/app/products/policy.rs](src/app/products/policy.rs) |
| List, form and trash pages | [resources/views/products](resources/views/products) |
| The table, with `deleted_at`; `slug` added in a second migration | [migrations](migrations) |

## Things worth copying

- **Public and members-only routes in one module.** Two `Routes` groups, the second ending in
  `.require_auth()`, joined with `merge`. Guests who open `/products/new` go to `/login`.
- **Every write checks the policy.** `edit`, `update`, `destroy` and `restore` call
  `user.authorize(...)` after loading the product; anyone else gets 403.
- **The view asks the policy too.** The list wraps each product in
  `Can::new(product, user, &["update", "delete"])`, so edit and delete buttons show only to the
  owner.
- **Soft deletes.** `product.delete()` sets `deleted_at`; `only_trashed()` lists the trash and
  `restore()` brings a product back.
- **Model hooks.** `#[model(hooks)]` + `impl ModelHooks for Product`: `saving` fills `slug`
  from the name before every insert and update, and refuses a name with no letters or digits
  (a `ValidationError` on `name`, so the form shows it); `saved` (on create) and `deleted`
  forget the cached product count shown on the list, through `renox::context::app()`. Hooks
  run for `save`, `create`, `save_only`, `save_changes`, `delete` and `force_delete` only:
  bulk writes (`Product::where_eq(..).update(..)`, `Query::delete`, `insert_many`) and
  `restore` skip them, so `restore` forgets the count itself.
- **Save what changed.** `update` keeps the product as loaded (`original`) and calls
  `product.save_changes(&db, &original)`: only the columns that differ (plus the hook's slug
  and `updated_at`) are written, so a concurrent change to another column isn't overwritten,
  and nothing at all is written when nothing changed.
- **PUT and DELETE from plain forms.** Forms post with `method_field('PUT')` or a
  `_method=DELETE` field.

## Tests

```bash
cargo test -p crud
```

[tests/products.rs](tests/products.rs) covers guests, validation, ownership, the trash, what
the list shows to whom, the hooks (slug on create and update, the rejected name, the cached
count, bulk updates skipping them) and `save_changes` keeping a concurrent edit.

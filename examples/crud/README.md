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
| List, form and trash pages, built with the UI kit (`renox/ui.html`): a table with a confirmation sheet for deletes, a form with live validation, an inset grouped list | [resources/views/products](resources/views/products) |
| The layout: a translucent navigation bar with an account menu, the kit's styles, the toast region | [resources/views/layouts/app.html](resources/views/layouts/app.html) |
| Error pages (404, 403, 500…) in the layout | [resources/views/errors/default.html](resources/views/errors/default.html) |
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
- **The UI kit.** The pages import `input`, `button`, `card`, `table`, `confirm`, `menu`… from
  `renox/ui.html`. Fields refill themselves and show their errors; `form_errors()` sums them up
  above the form.
  - The form has `data-live-validate`: a field is checked when it's left and again as it's
    fixed, without saving.
  - Delete is red text in the row and a sheet asks first, with Cancel focused.
  - On phones the slug column hides (`hide-narrow`) and a row's actions stack.
  - See [docs/ui.md](../../docs/ui.md) for the design rules the kit follows.
- **`push` / `stack`.** The layout has `{{ stack('head') }}` and `{{ stack('scripts') }}`; the
  form page pushes `<meta name="robots" content="noindex">` into the head from its content
  block with `{% call push('head') %}…{% endcall %}`. A component that needs a script pushes
  it with `once='key'`, so it's added once however often the component appears.
- **Error pages in the layout.** `errors/default.html` extends the layout, so a 404 or a 403
  keeps the navigation bar and the account menu, with a way back to the list. It gets
  `status`, `reason` and `detail` besides the usual globals.
- **Toasts.** `store`, `update`, `destroy` and `restore` return `(Toast::success(…), Redirect)`;
  `{{ toasts() }}` in the layout shows the toast on the next page, once.

## Tests

```bash
cargo test -p crud
```

[tests/products.rs](tests/products.rs) covers guests, validation, ownership, the trash, what
the list shows to whom, the hooks (slug on create and update, the rejected name, the cached
count, bulk updates skipping them) and `save_changes` keeping a concurrent edit.

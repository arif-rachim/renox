# examples/htmx-recipes

The interactions people usually reach for a JavaScript framework for, each done with a few htmx
attributes, a little Alpine, and a handler that answers with a fragment. Read it when you want a
page to feel like an app without writing JavaScript.

```bash
cd examples/htmx-recipes
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run -- db:seed             # 40 tasks, to see infinite scroll
cargo run                        # http://127.0.0.1:3000
```

## The recipes

| Recipe | HTML | Handler answers |
|---|---|---|
| Modal form (Alpine) that adds a row | `hx-post` + `hx-swap="afterbegin"` in [index.html](resources/views/tasks/index.html); the modal closes on the `task-added` event | the new row and `HX-Trigger: task-added`; 422 errors appear in the modal |
| Two places change at once (out-of-band swap) | the count's `<small id="open-count">` in [index.html](resources/views/tasks/index.html) | `view("tasks/answer.html", …).fragment("row").also("count")`: the row, then the count with `hx-swap-oob="true"` ([answer.html](resources/views/tasks/answer.html)); add, toggle, edit and delete all keep "N open" right |
| The server picks where the answer goes | add a task that's already on the list | `HxRetarget("#task-1")` + `HxReswap("outerHTML")`: the existing row is refreshed instead of a copy added, with a toast saying why |
| Toasts over htmx | delete a task | `(Toast::success("“Task 1” deleted."), answer)`: the toast rides `HX-Trigger` and shows at once; after "Clear done" (`HX-Refresh`) or "Archive" (`HX-Redirect`) it waits for the next page |
| Inline edit | double-click the title (`hx-trigger="dblclick"`) in [_row.html](resources/views/tasks/_row.html); Save is `hx-patch`, Escape (`keyup[key=='Escape']`) asks for the row again, in [_edit.html](resources/views/tasks/_edit.html) | the form, then the updated row |
| Toggle in place | a checkbox with `hx-patch` | the row |
| Dropdown menu (Alpine) with delete | `x-data="{ menu: false }"`, `@click.outside`, `hx-delete` + `hx-confirm` | an empty row part, so the row is swapped for nothing, with the open count out of band and a toast |
| Infinite scroll | the last item has `hx-trigger="revealed"` and `hx-swap="outerHTML"`, in [_rows.html](resources/views/tasks/_rows.html); its URL is `route('tasks.index', before=…)`, the oldest row shown (by id, so a task added meanwhile doesn't repeat a row) | the next rows (and the next loader) |
| Tabs (Alpine) | `x-data="{ tab: 'all' }"` and an `x-show` on each row; no request | — |
| Reload after a bulk change | "Clear done" | `HX-Refresh: true` (`HxRefresh`) |
| Go elsewhere after an action | "Archive done" | `HX-Redirect: /summary` (`htmx.redirect`, a 303 for plain forms) |

All handlers are in [src/app/tasks/mod.rs](src/app/tasks/mod.rs).

## Things worth copying

- **Hand-made here, ready-made in the kit.** The modal, the dropdown and the tabs are written
  out with Alpine to show how they work. In an app, the UI kit has them done, with keyboard
  support and focus handling: `sheet`/`open_button`, `menu`, `tabs`, and `confirm` for a delete
  (docs/ui.md; examples/crud uses them). This page loads `renox_ui()` only for its toasts.

- **One handler, two answers.** `Htmx` tells a handler whether htmx asked; it returns a fragment
  for htmx and a redirect for a plain form, so every action works without JavaScript too.
- **Fragments are templates.** `_row.html` is included by the list and rendered alone by the
  handlers, so there's one place that draws a row.
- **The server decides, Alpine reacts.** The modal closes because the server sent
  `HX-Trigger: task-added`, not because the button was clicked, so it stays open on errors.
- **No extra wiring for CSRF or errors.** `renox_head()` sends the CSRF token with every htmx
  request and shows 422 errors in the `data-error-for` slots.

## Tests

```bash
cargo test -p htmx-recipes
```

[tests/recipes.rs](tests/recipes.rs) checks each recipe's htmx answer (fragments, empty body,
`HX-Trigger`, `HX-Refresh`, `HX-Redirect`, the out-of-band count, `HX-Retarget`/`HX-Reswap`, toasts,
422) and the plain-form fallback.

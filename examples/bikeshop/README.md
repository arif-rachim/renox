# examples/bikeshop

Renox's flagship example (epic #231): a bike shop with three stores where customers **buy**
bikes and gear, **rent** a bike by the hour or the day, and have their bikes **serviced**,
once or on a plan. It keeps the shape of Sakila/Pagila (stock per store, rented for a period,
returned, maybe late, paid) and grows into a whole business, story by story (#232–#243, #245).

**Every page explains itself.** An "About this page" button opens a panel with what the page
is for, who uses it, which Renox features it uses and why, what happens under the hood, and
links to the guide and to the source files. `/about/pages` lists every page, filtered by
feature ("which pages use `renox::grid`?") or by role.

```bash
cd examples/bikeshop
cp .env.example .env    # then: rnx key:generate
cargo run -- migrate
cargo run               # http://127.0.0.1:3000
```

## What's here so far

The skeleton the stories build on (#232, part 1):

| What | Where |
|---|---|
| The app (`rnx new bikeshop`, lib + bin), its modules in one alphabetical list | [src/lib.rs](src/lib.rs) |
| One folder per area: `about`, `access`, `accounts`, `api`, `catalog`, `home`, `multistore`, `plans`, `rentals`, `reports`, `sales`, `staff`, `stock`, `workshop` (most are empty for now) | [src/app/](src/app/) |
| The "About this page" mechanism: `Explanation`, the registry, the panel's template function, `BIKESHOP_EXPLAIN` | [src/explain.rs](src/explain.rs), each area's `explain.rs` |
| The panel (the kit's `open_button` + `sheet(slide_over=true)` + `infolist`) | [resources/views/about/_panel.html](resources/views/about/_panel.html) |
| `/about/pages`: every page, filtered by feature and by role | [src/app/about/mod.rs](src/app/about/mod.rs), [resources/views/about/pages.html](resources/views/about/pages.html) |
| Public layout: the kit's `navbar` (links, search and cart slots, "About this page", language and account menus) | [resources/views/layouts/app.html](resources/views/layouts/app.html) |
| Staff layout: the kit's `sidebar` in an `rx-shell` (a slot for the store switcher) | [resources/views/layouts/staff.html](resources/views/layouts/staff.html) |
| Renox's sign-in pages in the shop's look, with the panel | [resources/views/renox/auth/layout.html](resources/views/renox/auth/layout.html) |
| Brand colour and the page grids (only the kit's type and spacing tokens) | [public/app.css](public/app.css) |
| Motion ([motion.dev](https://motion.dev), MIT), vendored, no CDN; `prefers-reduced-motion` respected | [public/vendor/motion/](public/vendor/motion/) (see its `NOTICE`), [public/app.js](public/app.js) |
| English and Spanish | [resources/lang/](resources/lang/) |
| The walker test (every GET route has an explanation whose guide anchors and source files exist), the panel, the index, the language menu | [tests/about.rs](tests/about.rs) |

## Blocks

The UI pieces Renox's kit doesn't have are built here, as **blocks**, written like a small
library so they can move to a crate later: one macro file each in
[resources/views/blocks/](resources/views/blocks/) (documented at the top, keyword arguments
like the kit's), `bs-` classes on the kit's `--rx-*` tokens in
[public/blocks/blocks.css](public/blocks/blocks.css), and the behaviour in
[public/blocks/blocks.js](public/blocks/blocks.js) (set up from `data-bs-*` attributes, on the
page and after htmx swaps; no inline handlers, so `CSP=strict` works; Motion on `transform`,
nothing moves under `prefers-reduced-motion`). Both layouts load them. Form blocks send plain
fields, so `Valid<T>` reads them; the server still checks every value.
[`/about/blocks`](src/app/about/blocks.rs) shows each one working, with its signature.

| Macro | What it is |
|---|---|
| `gallery(photos, id, label, enlarge)` | Product photos: arrows, thumbnails, swipe, arrow keys, enlarged in the kit's `sheet` |
| `range_slider(name_min, name_max, min, max, step, value_min, value_max, label, format)` | Two handles on one track (two native range inputs), sent as two fields |
| `quantity(name, value, min, max, step, label)` | − / number / + stepper |
| `keypad(target, label, decimal, zeros, enter_label)` | A counter's number pad typing into the input `target`; arrow keys between keys, digits typed on it |
| `kanban(id, columns, url, label, values)` + `kanban_card(card)` | Columns of cards; drag (mouse, or the handle on touch) or Space/arrows/Space; each move an htmx POST of `card`, `column`, `position`, announced in a live region, put back if refused |
| `month_calendar(month, events, url, param, today, first_day, label, heading)` | A month grid with events per day, previous/next months, today marked; a list on phones |
| `availability(columns, rows, label, corner)` | Resources × hours/days, booked and free slots; a free slot is a link or a small form |
| `datetime_range(name_start, name_end, label, …, min, max, step, opens, closes)` | Two kit date pickers + time selects, sent as two `YYYY-MM-DDTHH:MM` fields, with the duration shown |
| `date_picker_blocked(name, label, disabled_dates, closed_weekdays, …)` | The kit's `date_picker` with days that can't be picked (Cally's `isDateDisallowed`) |
| `swatches(name, label, options, selected, kind, attrs)` | Size or colour chips as radios; `attrs` takes htmx to update price and stock |
| `history(items, label, date_format)` | A vertical timeline: time, title, body (Markdown), status icon |
| `compare_plans(plans, features, highlight, …)` | Pricing cards and a comparison table, one plan highlighted |

Tests: [tests/blocks.rs](tests/blocks.rs) (the page, the calendar's grid, the server-side
checks) and `tests/browser/bikeshop-blocks.test.mjs` (every block's keyboard and pointer paths
at 1280 and 390 px, light and dark, both CSPs, reduced motion).

## Adding a page

1. Add the route to the area's `routes()` in `src/app/<area>/mod.rs`, with a `.name(…)`.
2. Add its `Explanation` to `src/app/<area>/explain.rs`: the route's name and path, purpose,
   who uses it, the Renox features and why, what happens under the hood, `docs/*.md#anchor`
   links and the source files. Use the same `api` text as other pages for the same feature
   (`/about/pages` groups by it).
3. A GET route that isn't a page (JSON, a file, a stream) goes in `not_pages()` with the reason.
4. `cargo test -p bikeshop --test about` tells you what is missing.

Spanish texts for an explanation go in `resources/lang/es.json` under
`about_page.<route name>` (`title`, `purpose`, `who`, `under_hood`, `features.<n>`); whatever
is missing there is shown in English.

## Settings

`BIKESHOP_EXPLAIN=false` hides the panels (the index stays). The rest is Renox's, documented in
[.env.example](.env.example).

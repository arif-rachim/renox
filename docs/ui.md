# Views, components and the UI kit

Renox pages are MiniJinja templates sent as HTML, updated in place with htmx. This guide
covers:

- components (macros that see the request);
- the UI kit that ships with Renox (`renox/ui.html`);
- toasts;
- fragments and out-of-band swaps;
- htmx response headers;
- data grids (see [docs/grid.md](grid.md));
- live validation;
- stacks (`push` / `stack`);
- Tailwind CSS.

## Components see the request

A component is a MiniJinja macro in a file of its own, imported where it's used. Inside it,
the same request helpers work as in the page itself:

- `old()`, `has_old()`, `error()`, `errors`;
- `t()`, `can()`, `auth`, `request`;
- `csrf_field()`, `flash`;
- `once()`.

```html
{# resources/views/components/price_field.html (rnx make:component price_field) #}
{% macro price_field(name, label) -%}
<div class="rx-field">
  <label class="rx-label" for="rx-{{ name }}">{{ label }}</label>
  <input class="rx-input" id="rx-{{ name }}" name="{{ name }}" inputmode="numeric" value="{{ old(name) }}"
    {%- if error(name) %} aria-invalid="true"{% endif %}>
  <p class="rx-error" data-error-for="{{ name }}">{{ error(name) }}</p>
</div>
{%- endmacro %}
```

```html
{% from "components/price_field.html" import price_field %}
{{ price_field("price", "Price") }}
```

`{% if once('datepicker') %}<script …>{% endif %}` is true only the first time a key is asked for
on a page. Use it for a component's script or style when the component appears several times.

## The UI kit

`renox/ui.html` is a set of components styled after Apple's Human Interface Guidelines. Add
its stylesheet and script to the layout, and the toast region to the body:

```html
<head>
  {{ renox_head() }}
  {{ renox_ui() }}
</head>
<body class="rx-page">
  <main class="rx-container">{% block content %}{% endblock %}</main>
  {{ toasts() }}
</body>
```

Then import what a page needs:

```html
{% from "renox/ui.html" import card, input, select, checkbox, button, link_button, form_errors %}
<form method="post" action="{{ route('products.store') }}" data-live-validate novalidate>
  {{ csrf_field() }}
  {% call card(title="New product") %}
    {{ form_errors() }}
    {{ input("name", "Name", required=true, hint="Shown on the product list.") }}
    {{ select("size", "Size", [["s", "Small"], ["m", "Medium"]], selected="m") }}
    {{ checkbox("featured", "Feature it on the home page", switch=true) }}
    <div class="rx-card__footer">
      {{ link_button(route('products.index'), "Cancel", variant="plain") }}
      {{ button("Create product") }}
    </div>
  {% endcall %}
</form>
```

| Component | What it is |
|---|---|
| `input(name, label, type=…, value=…, hint=…, required=…, autocomplete=…, placeholder=…, attrs={…}, id=…, prefix=…, suffix=…, datalist=[…], disabled=…, readonly=…, span=…)` | A labelled text field with its hint and error. It is refilled after a failed submit, except for passwords. `id` tells apart two fields of the same name on one page. `prefix`/`suffix` join a text to the field ("Rp", "kg"); `datalist` suggests values while accepting any. |
| `textarea(name, label, value=…, rows=4, hint=…, required=…, placeholder=…, attrs={…})` | The same for longer text. |
| `select(name, label, options, selected=…, hint=…, required=…, placeholder=…, attrs={…})` | The same for a choice. `options` are values or `[value, label]` pairs. |
| `checkbox(name, label, checked=…, hint=…, value="on", switch=…, attrs={…})` | A checkbox, or an iOS-style switch for settings. The whole row is the target. After a failed submit it shows what was sent, so an unticked box stays unticked. |
| `radio(name, label, options, selected=…, inline=…, columns=…)` | One choice out of a few, all visible, in a `fieldset` with the label as its legend. An option is a value, `[value, label]` or `[value, label, description]`. |
| `checkbox_list(name, label, options, selected=[…], inline=…, columns=…)` | Several choices: each ticked option sends `name` once, so the form field is a `Vec` with `#[serde(default)]` (nothing ticked sends nothing). |
| `toggle_buttons(name, label, options, selected=…, multiple=…)` | The options as a row of buttons, the pressed ones filled and checked: one choice (a radio group underneath) or, with `multiple`, several (checkboxes). |
| `file(name, label, accept=…, multiple=…, preview=…, current=…)` | A drop zone that is also the button; the chosen files are listed under it, images with a thumbnail when `preview`. `current` is the URL of the file stored now. The form needs `enctype="multipart/form-data"`, the field is an `Upload` (`Vec<Upload>` with `multiple`). |
| `date_picker(name, label, value=…, min=…, max=…)` | A date typed as `2026-10-02` or picked in a calendar (Cally, in a popover under the field, its month named in the page's language). Sent as `YYYY-MM-DD`, like `<input type="date">`: a `NaiveDate`. Without JavaScript it is a text field. |
| `show_when(field, values)` + `hide_when(field, values)` | Fields shown (or hidden) while another field has one of `values`. Hidden fields are disabled, so the form doesn't send them; check them on the server with `required_if`. Without JavaScript they stay visible. |
| `select(…, multiple=true, searchable=true)` | Several values (a `Vec`), and a box to type in that filters the options, with the chosen ones as chips. The native select stays underneath: the form sends the same thing, and it works without JavaScript. |
| `select(…, options_url=…, editable=true)` | The options come from the server as you type (`renox::select`), for lists too long for the page. `editable`: what was typed can be added ("Add “…”") and is chosen at once, and the chosen option can be renamed (the pencil). See "Options from the server" below. |
| `tags_input(name, label, value=[…], suggestions=[…])` | Free text as chips: Enter or a comma adds one, Backspace in the empty box removes the last. A `Vec<String>`. |
| `repeater(name, label, rows=[…], min=…, max=…)` with `{% call(row, prefix) %}` | Rows added, removed and moved by the user, each with the fields of the call block, named `name[0][field]` and renumbered as rows move. A `Vec` of a struct, checked with `v.nested`. Adding a row needs JavaScript. |
| `key_value(name, label, value=…)` | Pairs of text (a repeater with a key and a value per row): `KeyValues`. |
| `wizard(id, steps, submit_label)` + `wizard_step(id, key)` | A form in steps. Next checks the step (the browser's rules, then the server's with `data-live-validate`); after a failed submit the first step with an error opens. Without JavaScript all steps show. |
| `form_grid(columns=2)`, `fieldset(legend, hint=…, columns=…)` | Fields side by side from tablet width up (one column on phones), and a titled group of fields in a long form. A field's `span=2` or `span="full"` makes it wider. |

Every field also takes `id`, `disabled` (the field isn't sent) and `span`; `input` and
`textarea` take `readonly` (shaded, but sent and readable). `input` takes `revealable=true` (a
button that shows the password; Renox's own sign-in pages use it) and `copyable=true` (a button
that copies the value, for keys and links). The error summary links to each
field by its name, so a field with its own `id` and a radio group are found too.

```html
{% from "renox/ui.html" import input, radio, checkbox_list, form_grid, fieldset %}
{% call form_grid(2) %}
  {{ input("price", "Price", type="number", prefix="Rp", required=true) }}
  {{ input("weight", "Weight", type="number", suffix="kg") }}
  {{ input("city", "City", datalist=["Jakarta", "Bandung", "Surabaya"], span="full") }}
{% endcall %}
{% call fieldset("Delivery") %}
  {{ radio("speed", "Speed", [["regular", "Regular", "2–3 days"], ["express", "Express", "Tomorrow"]],
           selected="regular", required=true) }}
  {{ checkbox_list("extras", "Extras", [["gift", "Gift wrap"], ["note", "Card"]], inline=true) }}
{% endcall %}
```

Rows of fields, as in examples/teams' "New team" wizard: each row's inputs are named
`invites[0][email]`, `invites[1][email]`…, and `Valid` reads them into a `Vec`.

```html
{% from "renox/ui.html" import wizard, wizard_step, repeater, input %}
{% call wizard("new-team", [["name", "Name"], ["members", "Members"]], submit_label="Create team") %}
  {% call wizard_step("new-team", "name") %}{{ input("name", "Name", required=true) }}{% endcall %}
  {% call wizard_step("new-team", "members") %}
    {% call(row, prefix) repeater("invites", "Members", item_label="Member", max=10) %}
      {{ input(prefix ~ "[email]", "Email", type="email", value=row.email, required=true) }}
    {% endcall %}
  {% endcall %}
{% endcall %}
```

```rust
# use renox::prelude::*;
#[derive(serde::Deserialize)]
struct NewTeam {
    name: String,
    #[serde(default)]
    invites: Vec<Invite>,
}

#[derive(serde::Deserialize)]
struct Invite {
    email: String,
}

impl Validate for Invite {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
    }
}

impl Validate for NewTeam {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required();
        // Each row's rules; errors keyed `invites.0.email`, shown in that row.
        v.nested("invites", &self.invites);
    }
}
```

### Options from the server

A select over a long list (customers, categories) asks the server as people type, through
one URL. In the template, `options` only needs the chosen option(s):

```html
{{ select("category_id", "Category", [[product.category_id, category_name]] if product.category_id else [],
          selected=product.category_id, placeholder="None",
          options_url=route('admin.categories.options'), editable=true) }}
```

| The kit sends | The handler answers |
|---|---|
| `GET ?q=te` (typed, after a short pause) | the matches: `Json(Vec<SelectOption>)` |
| `GET ?values=4` (a value sent back without its label, after a failed submit) | those options (`query.is_lookup()`, `query.values_as::<i64>()`) |
| `POST label=Juice` (`editable`: "Add “Juice”") | the new option, which is chosen at once |
| `POST _method=PUT value=4&label=Juice & Smoothies` (`editable`: renaming the chosen one) | the option as saved |

A 422 from `Valid` shows its first message under the field (a name already taken, say), and
Escape gives up a rename. Who may add or rename is the route's call (examples/shop puts the
three handlers inside its admin group):

```rust
# use renox::prelude::*;
use renox::select::{OptionQuery, SelectOption};

#[derive(Model, serde::Serialize, Default)]
struct Category { id: i64, name: String }

async fn options(State(db): State<Db>, query: OptionQuery) -> Result<Json<Vec<SelectOption>>> {
    let rows = if query.is_lookup() {
        Category::query().where_in("id", query.values_as::<i64>()).get(&db).await?
    } else {
        Category::query()
            .where_op("name", "like", format!("%{}%", query.q))
            .order_by("name")
            .limit(20)
            .get(&db)
            .await?
    };
    Ok(Json(rows.iter().map(|c| SelectOption::new(c.id, &c.name)).collect()))
}
// Routes: .get(url, options), .post(url, create), .put(url, rename); the POST and PUT take
// `Valid<…>` forms with `label` (and `value`), checked like any form (`unique`, `max`).
```

Without JavaScript the native select shows only the options in the page.

A field that depends on another, as in examples/shop's checkout: the address only for the
courier, required only then.

```html
{% from "renox/ui.html" import toggle_buttons, show_when, textarea, date_picker %}
{{ toggle_buttons("delivery", "Delivery", [["courier", "Courier"], ["pickup", "Pick up"]],
                  selected="courier", required=true) }}
{% call show_when("delivery", "courier") %}
  {{ textarea("address", "Address", required=true) }}
  {{ date_picker("deliver_on", "Deliver on", min="2026-10-03") }}
{% endcall %}
```

```rust
# use renox::prelude::*;
# struct Checkout { delivery: String, address: String }
impl Validate for Checkout {
    fn rules(&self, v: &mut Validator) {
        // A pickup sends no address at all: the hidden group is disabled.
        v.field("address", &self.address).required_if(self.delivery == "courier");
    }
}
```

| Component | What it is |
|---|---|
| `button(label, variant="primary", type="submit", name=…, value=…, size=…, block=…, attrs={…})` | Variants: `primary`, `secondary`, `plain`, `danger` and `plain-danger`. The button shows a spinner while its form or htmx request is being sent. |
| `link_button(href, label, variant="secondary", size=…, attrs={…})` | A link that looks like a button. |
| `card(title=…, subtitle=…)`, `group(title=…, footer=…)` | A surface, and an inset grouped list like Settings. |
| `alert(message, kind=…, title=…)`, `badge(text, kind=…)` | Kinds: `info`, `success`, `warning`, `error`. An alert has an icon per kind, so color is never the only signal. A badge is color only, so its text must carry the meaning ("Paid", not "●"). |
| `form_errors(title=…)` | Every error of the last submit, above the form, each linking to its field. |
| `sheet(id, title, message=…)` + `open_button(id, label)` | A modal dialog. It closes on Esc and on a click on the backdrop, and focus returns to the button that opened it. On phones it rises from the bottom edge. |
| `confirm(id, label, action, title, message, confirm_label=…, method="DELETE", size=…)` | A destructive action behind a confirmation sheet, with Cancel focused first. |
| `menu(label, id=…, variant="secondary", size="small")` + `menu_link(href, label)`, `menu_action(action, label, method="POST", danger=…)`, `menu_separator()` | A menu of actions: arrow keys move, Esc closes. `menu_action` sends a form (with `_method` for other methods). |
| `tabs(id, items, selected=…, label=…)` + `tab_panel(id, key, selected=…)` | A segmented control; the arrow keys, Home and End move between tabs. |
| `table(head, caption=…)` | A table in a card. A heading `["Total", "num"]` right-aligns its column, and `["Slug", "hide-narrow"]` hides it on phones. |
| `empty(title, message, action_href, action_label)` | What an empty list says, with the way to add the first item. |

Renox's own pages use the kit too: the sign-in pages (`renox/auth/*`: login, registration,
password reset, email verification, password confirmation, the account page) and the error
page. The sign-in pages' layout has `stack('head')` and `stack('scripts')` (Renox's own error
page has no stacks); to change one of those pages, put a file with the same name under
`resources/views/renox/auth/`.

The kit's own texts ("optional", "Cancel", the error summary's title) come in English and
Indonesian. An app can change them in `lang/*.json`, under the keys `ui.optional`,
`ui.cancel`, `ui.close`, `ui.dismiss`, `ui.more` and `ui.errors_title`; the infolist's under
`ui.yes`, `ui.no`, `ui.show_more` and `ui.since.*` (`now`, `past`, `future`, `minutes`,
`hours`, `days`, `months`, `years`). The data grid's texts
are under `ui.grid.*` ([docs/grid.md](grid.md#translations)).

To change the markup or the styles, copy the kit into the app:

```sh
rnx make:component --ui   # my-app ui:publish: components/ui.html and public/css/renox-ui.css
```

Then import from `"components/ui.html"`, and in the layout load the copied stylesheet and only
the kit's script, so the styles aren't loaded twice:

```html
<link rel="stylesheet" href="{{ asset('css/renox-ui.css') }}">
{{ renox_ui(styles=false) }}
```

Every class starts with `rx-`, and nothing in the kit styles bare elements, so it sits next
to an app's own CSS. To rebrand it, override the tokens on `:root`, e.g.
`--rx-accent: #0a7d5a;`.

### Infolists: read-only details

A record's page (an order, a customer) is labels and values: an infolist, Filament's name for
it. `infolist` lays the entries out in a grid (one column on phones, `columns` from tablet
width up), and `entry` formats each value by its kind, so a page doesn't hand-roll its markup:

```html
{% from "renox/ui.html" import card, infolist, entry, repeatable %}
{% call card(title="Order #" ~ order.id) %}
  {% call infolist(columns=2) %}
    {{ entry("Status", order.status, badge={"paid": "success", "pending": "warning"},
             labels={"paid": "Paid", "pending": "Waiting for payment"}) }}
    {{ entry("Placed", order.created_at, format="since") }}
    {{ entry("Total", order.total, format="money") }}
    {{ entry("Invoice", order.invoice, copyable=true, url=route('invoices.show', order.id)) }}
    {{ entry("Tags", order.tags, badge=true, limit_list=3) }}
    {{ entry("Note", order.note, format="markdown", span="full", placeholder="No note") }}
    {% call entry("Customer") %}<a class="rx-link" href="/customers/{{ order.customer_id }}">{{ order.customer }}</a>{% endcall %}
    {% call(line) repeatable("Items", order.lines, columns=3) %}
      {{ entry("Product", line.name) }}
      {{ entry("Quantity", line.quantity, format="number") }}
      {{ entry("Subtotal", line.price * line.quantity, format="money") }}
    {% endcall %}
  {% endcall %}
{% endcall %}
```

| | |
|---|---|
| `infolist(columns=1, inline=false)` | The `<dl>` around the entries. `inline=true` puts each label beside its value (from tablet width up). |
| `entry(label, value, format=…, …)` | A label and its value. With `{% call entry(label) %}…{% endcall %}` the block is the value. `span=2` or `"full"` makes it wider, `inline=true` puts this label beside its value, `hide_label=true` keeps the label for screen readers only, `hint` adds a line under the value, `tooltip` a title. |
| `format` | `"date"`, `"datetime"` (with `date_format`, chrono's codes; in `APP_TIMEZONE`), `"since"` ("3 hours ago", the date as its tooltip), `"money"` (`APP_CURRENCY`, or `currency="USD"`), `"number"` (`decimals`), `"markdown"`, `"bool"` (a check and "Yes", or a cross and "No"), `"color"` (a swatch and the code), `"image"` (a URL; `image_size`, `circular`), `"key_value"` (pairs or a map, such as `KeyValues`, as a table). |
| `badge`, `labels` | `badge=true`, a kind (`"success"`) or kinds by value (`{"paid": "success"}`); `labels` names raw values (`{"paid": "Paid"}`), with or without a badge. |
| `url`, `new_tab`, `copyable` | A link; a copy button (it copies the raw value). |
| `prefix`, `suffix`, `limit`, `words`, `placeholder` | Text around the value; at most `limit` letters or `words` words, then "…"; what an empty value (none, `""`, an empty list) shows instead (`—`). |
| A list as `value` | Each item is formatted the same way: joined with commas, `list="lines"`, or `list="bullets"`; badges, swatches and images sit in a row. `limit_list=3` shows three and folds the rest behind "Show 2 more" (a `<details>`, no script). |
| `repeatable(label, items, columns=1)` with `{% call(item) %}` | A list of records inside the record (an order's lines): each item a small bordered infolist of the call block's entries. |

Sections and tabs are the kit's own `card`, `fieldset` and `tabs`: put an infolist in each.
examples/shop's order page and examples/fields' product page are built this way.

### Formatting values

Every template gets these filters, which the infolist uses too:

| Filter | Gives |
|---|---|
| `number`, `number(2)` | `75,000` in `en`, `75.000` in `id`: the page's locale picks the separators. |
| `money` | The amount in `APP_CURRENCY` (default `IDR`): `Rp 75.000` (id), `Rp 75,000` (en), `$1,250.50` with `USD`. Keywords: `currency="USD"` for another currency, `decimals=0`, `divide_by=100` for amounts kept in cents. `renox::format_money` does the same in Rust. |
| `date`, `date('%d/%m/%Y %H:%M')` | A date with chrono's format codes; a moment (`created_at`) in `APP_TIMEZONE`. |
| `since` | "3 hours ago", "in 2 days", "just now" ("3 jam yang lalu" in `id`), from the clock `TestApp::travel` moves. |
| `words(20)` | The first 20 words, then "…" (`end="…"`). |
| `markdown` | Markdown (CommonMark, tables, strikethrough, task lists) as HTML. HTML in the text is shown as text, and a link or image to anything but `http(s)`, `mailto`, `tel` or a relative URL points nowhere, so it is safe for what people typed. |

### Design principles

The kit follows the Human Interface Guidelines. These are the rules its components enforce,
and worth keeping in an app's own pages:

- **Hierarchy.**
  - One primary button per form or page: the action people came to take.
  - Destructive actions in lists are red text (`plain-danger`); the filled red button waits in
    the confirmation sheet.
  - Secondary text is lighter and smaller, never lighter than WCAG AA allows.
- **Clarity.**
  - Labels are always visible; placeholders are examples, not labels.
  - A hint says what's expected before anyone gets it wrong.
  - Errors are plain sentences next to the field and in a summary above the form.
  - Optional fields are marked, since required ones are the norm.
- **Feedback.**
  - A button shows it's working (a spinner, `aria-busy`) and can't be pressed twice.
  - A toast confirms what happened and names the thing ("“Espresso” moved to the trash").
  - Success and info toasts leave on their own, but wait while hovered or focused; error
    toasts stay until dismissed.
- **Forgiveness.**
  - Destructive actions ask first, with Cancel focused.
  - Live validation checks a field when it's left, then as it's corrected. It never complains
    about a field someone is still typing in.
- **Deference.**
  - Content stays plain.
  - Translucent materials are only for what floats over it: the navigation bar, menus,
    toasts, the sheet's backdrop.
- **Accessibility.**
  - Contrast is at least 4.5:1 for text in both appearances. Apple's system blue (#007AFF)
    falls short under white text, so the accent is #0071E3.
  - Every control has a 44 × 44 pt target and a visible focus ring for keyboards.
  - Hints and errors are tied to their fields with `aria-describedby`, and errors are
    announced.
  - Menus, tabs and sheets work with the keyboard.
- **Respect for settings.**
  - Dark mode follows the system; `data-theme="light|dark"` on `<html>` overrides it.
  - Reduce Motion stops the animations, Reduce Transparency makes materials solid, and
    Increase Contrast darkens separators and secondary text.
  - Text sizes are in `rem`, so they follow the reader's settings.
  - Sheet and toast placement respect the notch and the home indicator (safe areas).

### Error pages

`rnx new` writes `resources/views/errors/default.html`: the layout with the kit's `empty`
component and a way home, so a 404 or a 403 keeps the navigation bar. `errors/<status>.html`
replaces it for one status; the page gets `status`, `reason` and `detail` (with `APP_DEBUG`
also `request_line` and `template`) besides the usual
globals. Renox's own error page (used when an app has none) is built on the kit too.

## Toasts

Return a `Toast` with the response:

```rust
use renox::prelude::*;
use renox::Toast;

async fn save() -> (Toast, Redirect) {
    (Toast::success("Product saved."), Redirect::to("/products"))
}
```

After a redirect, the toast waits in the session and `{{ toasts() }}` shows it on the next
page, once. For an htmx request it rides in `HX-Trigger` (event `renox:toast`) and appears at
once. Kinds: `success`, `info`, `warning` and `error`; `error` is announced as an alert and
stays until dismissed.

A toast can say more, as Filament's notifications do:

```rust
use renox::prelude::*;
use renox::{Toast, ToastAction};

async fn place() -> (Toast, Redirect) {
    let toast = Toast::success("Order #7 placed")
        .body("We'll email you when it ships.")            // a second, lighter line
        .link("View order", "/orders/7")                    // links under the text
        .action(ToastAction::link("Help", "https://example.com/help").new_tab())
        .action(ToastAction::event("Undo", "order-undo"))   // a DOM event on `document`
        .seconds(8)                                          // or .persistent()
        .id("order-7");                                      // Renox.dismissToast("order-7")
    (toast, Redirect::to("/orders"))
}
```

- Every action closes the toast. A link goes only to http(s), mailto, tel or a relative URL;
  an event action dispatches its event on `document` (`detail.toast` is the id), so an htmx
  element can listen with `hx-trigger="order-undo from:document"`.
- `seconds(n)` sets how long it stays (errors too); `persistent()` keeps it until dismissed.
  Every toast waits while hovered or focused.
- A toast with an `id` replaces an earlier one with the same id.
- `{{ toasts(position="bottom-end") }}` moves them: `top` (the default, centred),
  `top-start`, `top-end`, `bottom`, `bottom-start` or `bottom-end`. Phones keep them centred.
- From the page's own script: `Renox.toast({kind: "success", message: "Copied", body: "…",
  actions: [{label: "Open", url: "/x"}], duration: 3000, id: "copy"})` and
  `Renox.dismissToast("copy")`.

In-app notifications that stay (a bell in the navigation bar, new ones arriving live) are in
[mail.md](mail.md#the-bell).

## Fragments and out-of-band swaps

`view(…).fragment("rows")` renders only that block for htmx requests. `.also("count")` adds
more blocks after it. Give each extra block's root element an `id` and `hx-swap-oob="true"`, and
htmx swaps it into the element with that id:

```html
{% block rows %}<tr id="order-{{ order.id }}">…</tr>{% endblock %}
{% block count %}<span id="order-count" hx-swap-oob="true">{{ count }}</span>{% endblock %}
```

```rust
# use renox::prelude::*;
# fn demo(count: i64) -> View {
view("orders/index.html", context! { count }).fragment("rows").also("count")
# }
```

## htmx response headers

The following types are response parts; return them in a tuple with the response:

- `HxRetarget("#errors")`: swap into another element;
- `HxReswap("outerHTML")`: swap another way;
- `HxPushUrl("/orders?status=open")`: update the address bar;
- `HxRedirect`, `HxRefresh` and `HxTrigger`.

`Back` (in the prelude) is both an extractor and a response: it redirects to the previous page
(the `Referer`), or to `/` when that is missing or on another site, so it can't be used as an
open redirect.

```rust
# use renox::prelude::*;
async fn store(back: Back, session: Session) -> Result<Back> {
    session.flash("status", "Saved")?;
    Ok(back)
}
```

## Live validation

Add `data-live-validate` to a form that `Valid<T>` handles. The kit's script then sends a
field's value (with the rest of the form) when the field is left, and again as it's corrected,
with the header `X-Renox-Validate: field`. The `Valid` extractor answers with that field's
errors as JSON and **the handler doesn't run**, so nothing is saved until the form is
submitted. The rules are the form's own, database checks such as `unique` included.

## The current route, conditional classes, loops

A navigation marks the current section with `route_is` (the route's name; `*` stands for
anything), and `class_names` builds a `class` attribute from the classes whose condition holds:

```html
<a href="{{ route('admin.products.index') }}"
   class="{{ class_names('tab', {'tab-active': route_is('admin.products.*')}) }}"
   {% if route_is('admin.products.*') %}aria-current="page"{% endif %}>Products</a>
{# request.route is the name itself, e.g. "admin.products.edit" #}
```

`route_is` also takes several patterns (`route_is('orders.*', 'checkout')`). In Rust, the
`CurrentRoute` extractor gives the same (`route.is("admin.*")`, `route.name()`). Loops may stop
early or skip items with `{% break %}` and `{% continue %}`:

```html
{% for product in recently_viewed %}{% if loop.index > 4 %}{% break %}{% endif %}…{% endfor %}
```

A text that depends on a count uses Laravel's plural ranges in the lang file, `{n}` for exactly
n, `[a,b]` for a range and `*` for no end: `"{0} Sold out|{1} Only one left|[2,5] Only :count
left|[6,*] :count in stock"`, printed with `{{ t('products.in_stock', count=product.stock) }}`
(examples/shop).

## Dashboards

Figures, charts and the period they cover, like Filament's widgets, drawn on the server as
plain HTML and SVG (no chart library, nothing to load):

```rust
use renox::prelude::*;
use renox::chart::{Period, Trend};

#[derive(Model, serde::Serialize, Default)]
struct Order { id: i64, total: i64, status: String, created_at: Option<renox::db::DateTime> }

// `?period=30d`: 7d, 30d, 90d (any number of days up to 366), 12m (months up
// to 36), mtd, ytd; 30 days without one.
async fn dashboard(State(state): State<AppState>, period: Period) -> Result<View> {
    let paid = || Order::where_eq("status", "paid");
    let sales = Trend::of(paid(), "created_at").over(period).sum(&state, "total").await?;
    let before = Trend::of(paid(), "created_at").over(period.previous()).sum(&state, "total").await?;
    let orders = Trend::of(paid(), "created_at").over(period).count(&state).await?;
    Ok(view("dashboard.html", context! {
        period,
        revenue => sales.total(),
        change => sales.change_from(&before), // percent; None when the period before was 0
        sales => sales.named("Sales"),
        orders,
    }))
}
```

```html
{% from "renox/ui.html" import period_filter, stats, stat, dashboard, widget %}
{{ period_filter(period) }}
{% call stats(4) %}
  {{ stat("Revenue", revenue | money, delta=change, trend=sales.values) }}
  {{ stat("Refunds", refunds, delta=refund_change, good="down") }}
{% endcall %}
{% call dashboard(3) %}
  {% call widget("Sales", description="Paid orders", span=2) %}
    {{ chart("area", sales, format="money") }}
  {% endcall %}
  {{ widget("By status", url=route('dashboard.statuses'), poll=60) }}
  {% call widget("Orders", span="full") %}{{ chart("bar", orders, name="Orders") }}{% endcall %}
{% endcall %}
```

- **`Trend::of(query, column)`** places a model's rows in time by a date-time column and
  gives a `Series` per day (up to 92 days, and `mtd`) or per month: `count`, `sum(column)`
  or `average(column)`. The query's conditions apply; days are cut in `APP_TIMEZONE` (at its
  offset at the end of the period); empty days are 0. `Period::previous()` is the period
  just before, as long, for `Series::change_from` (a percent). A `Series` is `labels`
  (`2026-10-02`, or `2026-10` per month), `values`, `total()` and `named(…)`; build one by
  hand with `Series::new(labels, values)`.
- **`chart(kind, data, …)`**: `line`, `area`, `bar` (`stacked=true`) or `pie`/`doughnut`.
  `data` is a `Series`, a list of numbers, or a list of series (`{name, values}` maps);
  or pass `labels=…` with `series=[…]` or `values=[…]`. Options: `format` (`number`,
  `money` in `APP_CURRENCY` or `currency=…`, `percent`), `decimals`, `height` (240 px),
  `title` (for screen readers), `name` (one series' name), `x_format` (chrono's codes for
  date labels; else `Oct 2` / `2 Okt`, `Oct 2026`), `legend=false`, `table=false`, `id`.
- **How they read.** One axis that starts at 0 with clean ticks (`12.5K`, `2,5 jt` in
  Indonesian); a legend for two series or more; hairline grid; 2 px lines with a dot at the
  end; bars at most 24 px wide with rounded ends; a doughnut keeps six slices and folds the
  rest into "Other". The six series colours come in a fixed order checked for colour
  blindness in both appearances (`--rx-chart-1` … `--rx-chart-6`; a seventh series is grey).
- **Hover and keyboard.** A crosshair and one tooltip with every series at the nearest date
  (line, area), or per bar and per slice; the chart takes focus and the arrow keys, Home and
  End move along it. Every chart also has a "Show the data" table, so no value is only in a
  colour or a tooltip.
- **`stat(label, value, delta=…, delta_label=…, good="up", trend=…, url=…, hint=…)`**: a
  figure (`value` as it should read: `total | money`), its change in percent with an arrow
  and its sign (green when it goes the `good` way, "up", "down" or "none"), a sparkline of
  `trend`, a link. `stats(columns)` sets them side by side (two per row on phones).
- **`dashboard(columns)` + `widget(title, description=…, span=…, url=…, poll=…)`**: cards in
  a grid (one column on phones; `span=2` or `"full"`). A widget's content is its call block,
  or what `url` answers (a small template, or a `View` fragment), loaded after the page and
  again every `poll` seconds; the old content stays, dimmed, until the new one arrives.
- **`period_filter(period, options=…)`**: one row of presets over everything it scopes
  (`?period=`, keeping the rest of the query: `query_with(period="7d")` builds such links
  in any template).

examples/shop's admin dashboard uses all of it: the period, four figures, revenue against
the period before, orders per day, and orders by status loaded on their own every minute.

## Data grids

`renox::grid` is a server-side data grid for dashboards and back offices: a table that fills
its container, with a filter in every heading, sorting, pagination, frozen columns, grouped
headings, columns each user picks per screen size, editing in place, actions on selected rows,
summaries, groups and exports. It is defined in Rust and drawn with the `grid` macro of
`renox/grid.html`. [docs/grid.md](grid.md) is its guide, and examples/grid a dashboard built on
it. `{{ sparkline(values) }}` (a small line or bar chart as inline SVG) works in any template,
not only in a grid.

## Stacks

A page or a component often needs something in another part of the layout: a script at the end
of `<body>`, a style or a `<meta>` in `<head>`. The layout names the places with `stack`, and
anything rendered for the page adds to them with `push`:

```html
{# layouts/app.html (rnx new's layout has both) #}
<head>… {{ stack('head') }}</head>
<body>… {{ stack('scripts') }}</body>
```

```html
{# components/chart.html #}
{% macro chart(id, data) -%}
{% call push('scripts', once='chart') %}
  <script src="{{ asset('js/chart.js') }}" nonce="{{ csp_nonce() }}"></script>
{% endcall %}
<canvas id="{{ id }}" data-points="{{ data | tojson }}"></canvas>
{%- endmacro %}
```

- `push(name)` adds to the end of the stack, `prepend(name)` to the front.
- `once='key'` adds it only the first time that key is pushed to that stack on the page, however
  many charts there are.
- Pushes work from the page's blocks, from included templates and from imported components,
  and they reach a stack that was rendered earlier (the head): the layout's head is written
  before the page's blocks run, so `stack` leaves a marker that's filled in once the page is
  done.
- htmx fragments (`.fragment("rows")`) have no layout, so what they push goes nowhere. Put a
  fragment's script in the fragment itself.
- Error pages (`errors/*.html`) have stacks too. Mails don't.

## Tailwind CSS

`rnx new shop --tailwind` sets it up; for an existing app, create `resources/css/app.css`:

```css
@import "tailwindcss";
@source "../views";
```

and link the output in the layout: `<link rel="stylesheet" href="{{ asset('css/app.css') }}">`.

- `rnx serve` runs Tailwind in watch mode next to the app: it rebuilds `public/css/app.css` when
  a view changes, and the page reloads.
- `rnx build` builds it minified before compiling, so an embedded binary carries it. `rnx tailwind`
  builds it once (`--minify`, `--watch`).
- There's no Node: `rnx` downloads Tailwind's standalone CLI (v4, the version `rnx` pins) once into
  your cache and checks its SHA-256. `rnx tailwind:install` does it ahead of time;
  `TAILWIND_BIN=/path/to/tailwindcss` uses another binary, `RNX_CACHE_DIR` moves the cache.
- Commit `public/css/app.css`: the Dockerfile from `make:deploy` builds the app without
  Tailwind.
- The kit's `rx-*` rules sit outside Tailwind's cascade layers, so the components keep their
  look, and utilities (`mt-4`, `text-sm`, `md:grid-cols-2`) lay out around them. Tailwind's reset
  changes bare elements (headings, lists, links without a class) in your own markup; style those
  with utilities or the kit's classes (`rx-title`, `rx-link`).

## Coming from Laravel

| Laravel | Renox |
|---|---|
| Blade components (`<x-input>`) | Macros in `resources/views/components/*.html` (`rnx make:component`) that see `old`, `error`, `t`… |
| Breeze's components | `renox/ui.html` (`rnx make:component --ui` to copy it) |
| `session()->flash('status')` + a toast library | `Toast::success(…)` and `{{ toasts() }}` |
| `@once` | `{% if once('key') %}` |
| `@push('scripts')` / `@stack('scripts')`, `@pushOnce`, `@prepend` | `{% call push('scripts') %}…{% endcall %}` / `{{ stack('scripts') }}`, `push(…, once='key')`, `prepend` |
| Vite + Tailwind | `rnx new --tailwind`: Tailwind's standalone CLI in `rnx serve` / `rnx build` |
| `@fragment` / `fragments([...])` | `.fragment("rows").also("count")` |
| Precognition (live validation) | `data-live-validate` and `Valid<T>` |
| `@class(['tab', 'active' => $on])` | `class_names('tab', {'active': on})` |
| `request()->routeIs('admin.*')` | `route_is('admin.*')` (and `request.route`, the route's name) |
| `@break` / `@continue` in `@foreach` | `{% break %}` / `{% continue %}` |
| `back()` / `redirect()->back()` | the `Back` extractor, returned as the response |
| Filament's tables | `renox::grid` ([docs/grid.md](grid.md)) |

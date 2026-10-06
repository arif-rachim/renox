# Views, components and the UI kit

This part of Renox makes the pages people see in their browser. Your Rust code picks a
template, fills in the blanks, and Renox sends the result as HTML.

Renox pages are **MiniJinja templates** (HTML files with blanks). Renox turns them into HTML
and sends them to the browser. Then **htmx** updates parts of the page in place, without a
full reload.

### Words you'll meet

| Word | What it means |
|---|---|
| **template** (or view) | An HTML file with blanks, like `{{ product.name }}`, that the app fills in. Renox uses MiniJinja for this. |
| **layout** | The frame around every page: the `<head>`, the navigation bar, the footer. Each page fills in the middle. |
| **macro** | A small piece of template you can reuse, like a function: you call it with values and it writes HTML. |
| **component** | A macro kept in a file of its own, such as a price field, and imported where it's used. |
| **the UI kit** | The set of ready-made components that comes with Renox (`renox/ui.html`): fields, buttons, menus, cards, tables and more. |
| **htmx** | A small script (it comes with Renox) that sends a request and swaps part of the page with the answer, with no full reload. |
| **fragment** | A piece of a page (one block of a template) sent alone, for htmx to swap in. |
| **toast** | A short message that pops up at the edge of the screen ("Product saved.") and goes away. |
| **Alpine.js** | Another small script that comes with Renox. It adds small bits of behaviour in HTML attributes (open a menu, show a field). |
| **CSS token** | A named setting for the look, like `--rx-accent` (the accent colour). Change it once and every component follows. |

### In this guide

- [Components](#components-see-the-request): your own reusable pieces, which can see the
  request (old input, errors, translations).
- [The UI kit](#the-ui-kit) that ships with Renox (`renox/ui.html`):
  - form fields;
  - the page's frame (navigation bar, sidebar, page headers);
  - themes and type;
  - infolists (read-only details);
  - actions (action groups, wizards, import, duplicate, export) and dashboards.
- [Toasts](#toasts): short pop-up messages.
- [Fragments and out-of-band swaps](#fragments-and-out-of-band-swaps): sending only part of a
  page.
- [htmx response headers](#htmx-response-headers): telling htmx what to do next.
- Data grids (see [docs/grid.md](grid.md)).
- [Live validation](#live-validation): checking a form while people fill it in.
- [Stacks](#stacks) (`push` / `stack`): adding scripts and styles to the layout from a page.
- [Tailwind CSS](#tailwind-css).

## Components see the request

A **component** is a MiniJinja macro in a file of its own. You import it where you use it.

Inside a component, the same request helpers work as in the page itself:

- `old()`, `has_old()`, `error()`, `errors`: what the person typed last time, and what was wrong
  with it; `errors_in('login')` lists every error of a named error bag
  ([validation.md](validation.md));
- `t()`, `can()`, `auth`, `request`: translations, permission checks, the logged-in user, and
  the request;
- `csrf_field()`, `flash`: the hidden security field every form needs, and one-time messages;
  `csrf_token` is the same token as plain text (for a `<meta>` tag or a script);
- `method_field('PUT')`: the hidden `_method` field, for a form that sends PUT, PATCH or
  DELETE;
- `seo(title=…, description=…, image=…, type=…, canonical=…)`: the page's `<title>`, its
  description, its canonical link and the tags social sites read. `canonical` is this page's
  address on `APP_URL` unless you give one;
- `csp_nonce()`: the page's nonce (a one-time code), for an inline `<script nonce="…">` that
  the Content Security Policy should allow;
- `once()`: "only the first time on this page".

And these work in any template:

- `asset('css/app.css')`: the address of a file in `public/`, with `?v=…` added, which
  changes whenever the file does (so browsers never keep an old copy);
- `storage_url(key)`: the address of a file stored with `Storage`;
- `page_url(3)`: this page's address with `page=3`, keeping the rest of the query (`?q=…`);
  `query_with(period="7d")` does the same for any keys.

Here is a small component, a price field:

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

What it does:

- It writes a label and an input.
- `old(name)` puts back what the person typed, if the form came back with errors.
- `error(name)` shows the error message for this field, if there is one. It also marks the
  input as invalid (`aria-invalid`), so screen readers know.

`rnx make:component price_field` writes the empty file for you. A page uses it like this:

```html
{% from "components/price_field.html" import price_field %}
{{ price_field("price", "Price") }}
```

The first line imports the macro. The second calls it, with the field's name and its label.

### Only once per page

`{% if once('datepicker') %}<script …>{% endif %}` is true only the first time a key is asked
for on a page. Use it for a component's script or style when the component appears several
times. That way the script is added once, not once per field.

> [!NOTE]
> **Coming from Laravel:** components are like Blade components (`<x-input>`), and `once()`
> is like `@once`. See the table at the [end of this page](#coming-from-laravel).

## The UI kit

`renox/ui.html` is a set of ready-made components. You import them into your templates, the
same way as your own.

The kit follows the principles of Apple's Human Interface Guidelines: hierarchy (the most
important thing stands out), clarity, and accessibility (it works for everyone, including
people using a keyboard or a screen reader).

Its default look is called "warm":

- Inter for text, and Poppins for titles and figures (both come with Renox);
- a paper-white page;
- white surfaces with a hairline (a very thin border);
- an indigo accent colour;
- a type scale that makes the important thing the largest (see
  [Themes and type](#themes-and-type)).

### Turning the kit on

Add the kit's stylesheet and script to the layout, and the toast region to the body:

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

- `renox_head()` adds what every Renox page needs in `<head>`.
- `renox_ui()` adds the kit's stylesheet and script.
- `{{ toasts() }}` is the place where toasts appear.

### A first form

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

What this page shows:

- a card titled "New product";
- a summary of errors at the top (only after a failed submit);
- a name field with a hint under it, a size choice, and an on/off switch;
- a Cancel link and a "Create product" button at the bottom.

Each field comes with its label, its hint and its error message. You don't write that HTML
yourself.

> [!TIP]
> `{% call card(…) %} … {% endcall %}` passes everything between the two tags into the
> component. The card puts it inside itself. Many kit components work this way.

### Form fields

Here is every form field. The arguments with `=…` are optional: leave them out and the
field uses a sensible default.

| Component | What it is |
|---|---|
| `input(name, label, type=…, value=…, hint=…, required=…, autocomplete=…, placeholder=…, attrs={…}, id=…, prefix=…, suffix=…, datalist=[…], disabled=…, readonly=…, span=…)` | A text field with its label, hint and error. After a failed submit it is filled in again with what was typed (but never for passwords). `id` tells two fields with the same name apart on one page. `prefix`/`suffix` put a short text before or after the field ("Rp", "kg"). `datalist` suggests values, but any value is accepted. |
| `textarea(name, label, value=…, rows=4, hint=…, required=…, placeholder=…, attrs={…})` | The same, for longer text. |
| `select(name, label, options, selected=…, hint=…, required=…, placeholder=…, attrs={…})` | The same, for a choice from a list. Each option is a value, or a `[value, label]` pair. |
| `checkbox(name, label, checked=…, hint=…, value="on", switch=…, attrs={…})` | A checkbox, or (with `switch`) an iOS-style on/off switch for settings. The whole row can be clicked. After a failed submit it shows what was sent, so a box that was left unticked stays unticked. |
| `radio(name, label, options, selected=…, inline=…, columns=…)` | One choice out of a few, all visible at once. They sit in a `fieldset`, with the label as its title (`legend`). An option is a value, `[value, label]` or `[value, label, description]`. |
| `checkbox_list(name, label, options, selected=[…], inline=…, columns=…)` | Several choices. Each ticked option sends `name` once. So the form's field in Rust is a `Vec`, with `#[serde(default)]` (when nothing is ticked, nothing is sent). |
| `toggle_buttons(name, label, options, selected=…, multiple=…)` | The options as a row of buttons; the pressed ones are filled and checked. Without `multiple`, one choice (a radio group underneath). With `multiple`, several (checkboxes underneath). |
| `file(name, label, accept=…, multiple=…, preview=…, current=…, current_name=…)` | An upload area: drop files on it, or click it to pick them. The chosen files are listed under it; images get a small picture when `preview` is on. `current` is the URL of the file stored now, and `current_name` the name shown for it. The form needs `enctype="multipart/form-data"`. The Rust field is an `Upload` when the file is required, an `Option<Upload>` when it may be left out (an edit form keeping the current file), and a `Vec<Upload>` with `multiple`. |
| `date_picker(name, label, value=…, min=…, max=…, placeholder="YYYY-MM-DD", readonly=…)` | A date: typed as `2026-10-02`, or picked from a calendar. The calendar is Cally, opening in a small box under the field, with the month named in the page's language. The date is sent as `YYYY-MM-DD`, like `<input type="date">`, so the Rust field is a `NaiveDate`. Without JavaScript it is a plain text field. |
| `show_when(field, values)` + `hide_when(field, values)` | Fields shown (or hidden) while another field has one of `values`. Hidden fields are disabled, so the form doesn't send them. Check them on the server with `required_if`. Without JavaScript they stay visible. |
| `select(…, multiple=true, searchable=true)` | Pick several values (a `Vec`), with a box to type in that filters the options. The chosen ones show as chips (small rounded labels). The browser's own select stays underneath: the form sends the same thing, and it works without JavaScript. |
| `select(…, options_url=…, editable=true)` | The options come from the server as you type (`renox::select`), for lists too long to put in the page. With `editable`, what was typed can be added ("Add “…”") and is chosen at once, and the chosen option can be renamed (with the pencil). See [Options from the server](#options-from-the-server) below. |
| `tags_input(name, label, value=[…], suggestions=[…])` | Free text as chips. Enter or a comma adds one; Backspace in the empty box removes the last one. The Rust field is a `Vec<String>`. |
| `repeater(name, label, rows=[…], min=…, max=…, item_label=…, add_label=…, reorderable=true)` with `{% call(row, prefix) %}` | Rows that people add, remove and move. `reorderable=false` keeps their order fixed. Each row has the fields of the call block, named `name[0][field]`, and they are renumbered when rows move. The Rust field is a `Vec` of a struct, checked with `v.nested`. Adding a row needs JavaScript. |
| `key_value(name, label, value=…, key_label=…, value_label=…, add_label=…, hint=…)` | Pairs of text: a repeater with a key and a value on each row. `key_label` and `value_label` name the two columns, `add_label` the add button. The Rust field is `KeyValues`. |
| `wizard(id, steps, submit_label, back_label=…, next_label=…)` + `wizard_step(id, key, title=…)` | A form in steps. Next checks the current step first: the browser's own rules, then the server's (with `data-live-validate`). After a failed submit, the first step with an error opens. Without JavaScript all steps show at once. |
| `form_grid(columns=2)`, `fieldset(legend, hint=…, columns=…)` | `form_grid` puts fields side by side from tablet width up (one column on phones). `fieldset` is a group of fields with a title, for long forms. A field's `span=2` or `span="full"` makes it wider. |

A few options work on many fields:

- Every field takes `id` and `span`.
- Every field but `repeater` and `key_value` takes `disabled` (the field isn't sent).
- `input`, `textarea` and `date_picker` take `readonly`: the field is shaded and can't be
  changed, but it is still sent and can be read.
- `input` takes `revealable=true`: a button that shows the password. Renox's own sign-in
  pages use it.
- `input` takes `copyable=true`: a button that copies the value, handy for keys and links.

The error summary links to each field by its name. So it finds a field with its own `id`, and
a radio group, too.

Here are fields side by side, and a group with a title:

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

Price and weight sit next to each other. City takes the whole row (`span="full"`) and
suggests three cities. Below, a "Delivery" group asks for a speed (one choice) and extras
(any number).

### Rows of fields

Some forms have a list inside them: the people to invite to a team, the lines of an order.
examples/teams' "New team" wizard does this. Each row's input is named `invites[0][email]`,
`invites[1][email]`, and so on. `Valid` reads them into a `Vec`.

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

The wizard has two steps: first the team's name, then its members. In the second step, the
repeater lets people add up to 10 rows, each with an email field. `prefix` is the row's part
of the name (`invites[0]`), so `prefix ~ "[email]"` gives `invites[0][email]`.

On the Rust side, the form is a struct with a `Vec` of rows:

```rust
# use renox::prelude::*;
/// The whole "New team" form: a name and a list of invites.
#[derive(serde::Deserialize)]
struct NewTeam {
    name: String,
    #[serde(default)]
    invites: Vec<Invite>,
}

/// One row of the repeater: one person to invite.
#[derive(serde::Deserialize)]
struct Invite {
    email: String,
}

/// The rules for one row.
impl Validate for Invite {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
    }
}

/// The rules for the whole form.
impl Validate for NewTeam {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required();
        // Each row's rules; errors keyed `invites.0.email`, shown in that row.
        v.nested("invites", &self.invites);
    }
}
```

`#[serde(default)]` means "no rows sent" becomes an empty list instead of an error.
`v.nested` checks every row with `Invite`'s rules. An error in the first row's email is
stored as `invites.0.email`, and the kit shows it in that row.

### Options from the server

Some lists are too long to put in the page: all your customers, all your categories. A
select over such a list asks the server as people type. It uses one URL for everything.

In the template, `options` only needs the option (or options) chosen now:

```html
{{ select("category_id", "Category", [[product.category_id, category_name]] if product.category_id else [],
          selected=product.category_id, placeholder="None",
          options_url=route('admin.categories.options'), editable=true) }}
```

Here is what the kit sends to that URL, and what the handler answers:

| The kit sends | The handler answers |
|---|---|
| `GET ?q=te` (typed, after a short pause) | the matches: `Json(Vec<SelectOption>)` |
| `GET ?values=4` (a value sent back without its label, after a failed submit) | those options (`query.is_lookup()`, `query.values_as::<i64>()`) |
| `POST label=Juice` (`editable`: "Add “Juice”") | the new option, which is chosen at once |
| `POST _method=PUT value=4&label=Juice & Smoothies` (`editable`: renaming the chosen one) | the option as saved |

If adding or renaming fails, a 422 answer from `Valid` (say, "that name is already taken")
shows its first message under the field. Escape gives up a rename.

Who may add or rename is up to the route. examples/shop puts the three handlers inside its
admin group, so only admins can.

Here is the handler that answers the `GET`s:

```rust
# use renox::prelude::*;
use renox::select::{OptionQuery, SelectOption};

/// A category of products.
#[derive(Model, serde::Serialize, Default)]
struct Category { id: i64, name: String }

/// Answers the select's searches and lookups with a list of options.
async fn options(State(db): State<Db>, query: OptionQuery) -> Result<Json<Vec<SelectOption>>> {
    // A lookup asks for options by their values (after a failed submit).
    let rows = if query.is_lookup() {
        Category::query().where_in("id", query.values_as::<i64>()).get(&db).await?
    } else {
        // A search: the first 20 categories whose name contains what was typed.
        Category::query()
            .where_op("name", "like", format!("%{}%", query.q))
            .order_by("name")
            .limit(20)
            .get(&db)
            .await?
    };
    // Each option is a value (the id) and the label people see (the name).
    Ok(Json(rows.iter().map(|c| SelectOption::new(c.id, &c.name)).collect()))
}
// Routes: .get(url, options), .post(url, create), .put(url, rename); the POST and PUT take
// `Valid<…>` forms with `label` (and `value`), checked like any form (`unique`, `max`).
```

Without JavaScript, the browser's own select shows only the options in the page.

### A field that depends on another

Sometimes a field matters only when another field has a certain value. examples/shop's
checkout asks for an address only for delivery by courier, and requires it only then:

```html
{% from "renox/ui.html" import toggle_buttons, show_when, textarea, date_picker %}
{{ toggle_buttons("delivery", "Delivery", [["courier", "Courier"], ["pickup", "Pick up"]],
                  selected="courier", required=true) }}
{% call show_when("delivery", "courier") %}
  {{ textarea("address", "Address", required=true) }}
  {{ date_picker("deliver_on", "Deliver on", min="2026-10-03") }}
{% endcall %}
```

The address and the date show only while "Courier" is pressed. When "Pick up" is pressed,
they are hidden and not sent. So the server must also know the rule:

```rust
# use renox::prelude::*;
# struct Checkout { delivery: String, address: String }
/// The checkout form's rules: the address is needed only for the courier.
impl Validate for Checkout {
    fn rules(&self, v: &mut Validator) {
        // A pickup sends no address at all: the hidden group is disabled.
        v.field("address", &self.address).required_if(self.delivery == "courier");
    }
}
```

`required_if(…)` makes the field required only when the condition is true.

### Buttons, surfaces and other parts

The rest of the kit's everyday components:

| Component | What it is |
|---|---|
| `button(label, variant="primary", type="submit", name=…, value=…, size=…, block=…, attrs={…}, icon=…, badge=…, key=…, disabled=…, disabled_reason=…)` | A button. Variants (styles): `primary`, `secondary`, `plain`, `danger` and `plain-danger`. It shows a spinner while its form or htmx request is being sent. For `icon`, `badge`, `key` and `disabled_reason`, see [Actions](#actions). |
| `link_button(href, label, variant="secondary", size=…, attrs={…}, icon=…, badge=…, key=…, new_tab=…)` | A link that looks like a button. |
| `icon_button(icon, label, href=…, variant="plain", type="button", size=…, attrs={…}, key=…, badge=…, disabled=…, disabled_reason=…, new_tab=…)` | A button that shows only an icon (or, with `href`, a link). `label` is the name screen readers say, and its tooltip. Variants: `plain`, `primary`, `danger`; also `size="small"`. |
| `card(title=…, subtitle=…)`, `group(title=…, footer=…)` | A card is a surface (a white box) for content. A group is an inset list of rows, like the iPhone's Settings. |
| `alert(message, kind=…, title=…)`, `badge(text, kind=…)` | Kinds: `info`, `success`, `warning`, `error`. An alert has an icon for each kind, so colour is never the only sign. A badge is colour only, so its text must carry the meaning ("Paid", not "●"). |
| `form_errors(title=…)` | Every error of the last submit, above the form. Each links to its field. |
| `sheet(id, title, message=…, slide_over=…, width=…, icon=…)` + `open_button(id, label, variant="secondary", size=…, icon=…, key=…, badge=…)` | A sheet is a modal dialog: a box over the page that you must close before going on. It closes on Esc and on a click outside it (on the backdrop), and focus goes back to the button that opened it. On phones it rises from the bottom edge. `slide_over=true` puts it at the side, full height. `width` is `sm`, `md` (default), `lg` or `xl`. `icon` (`info`, `success`, `warning`, `error`) shows above the title. Any element with `data-rx-open="<sheet id>"` opens the sheet too, not only `open_button`. A button with `data-rx-close` inside the sheet closes it. |
| `action_sheet(id, label, action, title, …)` | A button that opens a sheet with a form sent by htmx. See [Actions](#actions). |
| `confirm(id, label, action, title, message, confirm_label=…, method="DELETE", size=…, icon=…, modal_icon="warning", key=…)` | A destructive action (like delete) that asks first, in a sheet. Cancel has the focus first, so pressing Enter by mistake does nothing harmful. `icon` goes on the button, `modal_icon` above the sheet's title (`none` for no icon). |
| `menu(label, id=…, variant="secondary", size="small")` + `menu_link(href, label, icon=…, download=…, new_tab=…)`, `menu_action(action, label, method="POST", danger=…, icon=…)`, `menu_open(id, label)`, `menu_section(title)`, `menu_separator()` | A drop-down menu of actions. The arrow keys move, Esc closes. `menu_action` sends a form (with `_method` for methods other than POST); `menu_open` opens a sheet. |
| `action_group(label=none, icon=none)`, `wizard_action(…)`, `import_action(…)` | Several actions behind one button, an action with steps, a CSV import. See [Actions](#action-groups). |
| `tabs(id, items, selected=…, label=…)` + `tab_panel(id, key, selected=…)` | A segmented control: tabs that switch between panels on one page. The arrow keys, Home and End move between tabs. |
| `table(head, caption=…)` | A table in a card. A heading `["Total", "num"]` lines its column up on the right (for numbers), and `["Slug", "hide-narrow"]` hides the column on phones. |
| `empty(title, message=…, action_href=…, action_label=…)` | What an empty list says. With `action_href` (and its `action_label`), it adds a link to add the first item. |
| `notification_bell(count=none, id="rx-notifications")` | The logged-in user's notifications, in the navigation bar: a badge with the unread count, a panel, and new ones arriving live as toasts. Needs `Auth::new().notifications()`; pass `unread_notifications`. See [docs/mail.md](mail.md#the-bell). |
| `event_stream()` | Opens the same live stream on a page without the bell, so the app's events (`state.broadcast(…)`) arrive as DOM events. Nothing for guests. See [docs/mail.md](mail.md#your-own-live-events). |

### Navigation and page structure

The frame of a page comes from the kit too. So an app writes no CSS for its navigation bar,
its sidebar or its page headings.

Every example is built this way. examples/backoffice has a sidebar. examples/shop has a
navigation bar with links, a cart count and menus. [examples/bikeshop](../examples/bikeshop) has both (a public navbar, a
staff sidebar) and shows how to add what the kit lacks as "blocks" of the app's own.

A navigation bar on top looks like this:

```html
<body class="rx-page">
  {% from "renox/ui.html" import navbar, nav_links, nav_link, menu, menu_link, menu_action, link_button %}
  {% call navbar(app.name, href=route('home'), width="wide") %}
    {% call nav_links() %}
      {{ nav_link(route('products.index'), "Products", active=route_is('products.*')) }}
      {{ nav_link(route('cart.show'), "Cart", active=route_is('cart.*'), badge=cart_count) }}
    {% endcall %}
    {# rx-spacer pushes what follows to the right end of the bar #}
    <span class="rx-spacer"></span>
    {% call menu(auth.user.name, id="account-menu", variant="plain") %}
      {{ menu_link(route('account.show'), "Account") }}
      {{ menu_action(route('logout'), "Log out") }}
    {% endcall %}
  {% endcall %}
  <main class="rx-container rx-container--wide" id="main">…</main>
</body>
```

The bar shows the app's name (a link home), two links (Products, and Cart with a count), and
on the right a menu with the user's name. `active=route_is('products.*')` highlights
"Products" on every products page.

A back office puts its sections down the side instead:

```html
<body class="rx-page rx-shell">
  {% from "renox/ui.html" import sidebar, sidebar_link, sidebar_section, navbar, notification_bell %}
  {% call sidebar(company.name, href=route('home')) %}
    {{ sidebar_link(route('home'), "Dashboard", active=route_is('home')) }}
    {{ sidebar_link(route('invoices.index'), "Invoices", active=route_is('invoices.*')) }}
    {{ sidebar_section("Admin") }}
    {% if can('staff.manage') %}{{ sidebar_link(route('staff.index'), "Staff", active=route_is('staff.*')) }}{% endif %}
  {% endcall %}
  <div class="rx-shell__main">
    {% call navbar(none, width="full", skip=false) %}<span class="rx-spacer"></span>{{ notification_bell(unread_notifications) }}{% endcall %}
    <main class="rx-shell__content" id="main">…</main>
  </div>
</body>
```

The sidebar lists Dashboard and Invoices, then an "Admin" heading. The "Staff" link shows only
to people allowed to manage staff (`can('staff.manage')`). Next to the sidebar, a thin bar on
top holds the notification bell, and the page's content goes under it.

| Component | What it is |
|---|---|
| `navbar(brand, href="/", logo=…, mark=…, width="narrow", label=…, skip=true)` | The bar on top: see-through, it stays at the top while you scroll ("sticky"), with a hairline under it. `brand` links to `href` (`none` for no brand). `logo` is an image URL; `mark=true` shows the name's first letter in the accent colour. `width` matches the page: `narrow` (`rx-container`), `wide` (`rx-container--wide`) or `full`. It starts with a "Skip to content" link to `#main`, for keyboard users. |
| `nav_links()` + `nav_link(href, label, active=…, badge=…)` | The bar's sections. `active` (usually `route_is('….*')`) marks the current one, with `aria-current` for screen readers. `badge` shows a count. On phones the links get a row of their own that scrolls sideways. |
| `sidebar(brand, href="/", logo=…, mark=true, label=…, skip=true)` + `sidebar_link(href, label, active=…, badge=…)`, `sidebar_section(title)` | Sections down the side, for back offices. The page is built like this: `rx-shell` on `<body>`, the sidebar, then `rx-shell__main` holding a full-width `navbar` and `<main class="rx-shell__content">`. On phones the sidebar becomes a bar of links on top. |
| `page_header(title, subtitle=…, back=…, back_label=…, badge=…, badge_kind=…)` | A page's heading: the title (with a badge), a line under it, a link back (`back`), and the call block's buttons at the end of the row (under the title on phones). |
| `toolbar()` | Filter fields side by side. They wrap onto more lines on narrow screens, and line up with their buttons. There are no "(optional)" marks, since filters are all optional. Put it inside the `<form>`. |
| `row_actions()` | A table row's buttons, at the end of the row. On phones only the icons show (the labels stay for screen readers). |
| `list(id=…, label=…)` | Rows in a surface; each row is an `<li>` you write. Put `rx-list__main` on the part that takes the space left (`rx-list__main--done` strikes it through, for a finished task). Rows can be fragments that htmx adds and swaps. |
| `columns(count=2)` | Columns from tablet width up, one column on phones (a photo next to its details, say). |
| `card_grid()` + `media_card(href, title, image=…, image_alt="", subtitle=…, note=…, dimmed=…)` | Cards with a picture, in a grid that fills the row. Each card is at least 13rem wide; set `--rx-card-min` for another width. |
| `link_tabs(items, current=…, label=…)` | Links that look like a segmented control, for sections or filters that have their own URLs. `items` are `[href, label]` pairs. (`tabs` switches panels on one page instead.) |
| `thumbnail(src, alt="", href=…)` | A small square picture, for example in a table row. |
| `progress(value, max=100, label=…, show_value=true)` | A progress bar (the browser's `<progress>`) in the kit's colours, with the percentage next to it. |
| `menu_button(label, attrs={…}, danger=…)` | A menu item that is a plain button. `attrs` say what it does, usually with htmx (`hx-get`, `hx-delete`…). |

### More classes and options

Some things are classes or options, not macros:

- `rx-page--fill` on `<body>` makes the page as tall as the screen, with `<main>` taking the
  rest. Use it for a data grid that fills the screen (`rx-grid-fill`).
- `rx-image` is a picture as wide as its column.
- `input`, `textarea`, `select` and `checkbox` take `hide_label=true`. The label is hidden on
  screen but stays for screen readers (for example a quantity field in a table row).
- `confirm` takes `cancel_label` ("Keep order") and `fields`: hidden values sent with it, such
  as `{"status": "cancelled"}`.
- Every form field (`input`, `textarea`, `select`, `checkbox`, `radio`, `checkbox_list`,
  `toggle_buttons`, `file`, `date_picker`, `tags_input`) takes `bag="login"`. It shows the
  errors of a named error bag. That's for a page with two forms that share field names
  ([validation.md](validation.md)).

> [!TIP]
> Build pages from these and the components above, instead of writing your own. An app's
> `public/app.css` holds its brand tokens and what is truly its own (a printed invoice, say),
> not a navigation bar or a card.

### Renox's own pages

Renox's own pages use the kit too:

- the sign-in pages (`renox/auth/*`): login, registration, password reset, email
  verification, password confirmation, and the account page;
- the error page.

The sign-in pages' layout has `stack('head')` and `stack('scripts')` (see [Stacks](#stacks)).
Renox's own error page has no stacks.

To change one of those pages, put a file with the same name under
`resources/views/renox/auth/`. Your file is used instead of Renox's.

### The kit's texts

The kit's own texts ("optional", "Cancel", the error summary's title) are in English. An app
can change or translate them in `lang/<locale>.json` (for example `es.json` for Spanish):

- the kit: `ui.optional`, `ui.cancel`, `ui.close`, `ui.dismiss`, `ui.more`, `ui.skip`
  ("Skip to content"), `ui.main_navigation`, `ui.errors_title`, `ui.loading`, `ui.back` (the
  wizard and `page_header`'s back link) and `ui.next` (the wizard);
- the fields: `ui.show_password`, `ui.hide_password`, `ui.copy`, `ui.copied`,
  `ui.choose_file`, `ui.choose_files`, `ui.current_file`, `ui.choose_date`,
  `ui.previous_month`, `ui.next_month`, `ui.remove`, `ui.add_row`, `ui.move_up`,
  `ui.move_down`, `ui.key` and `ui.value`;
- the searchable select: `ui.search`, `ui.no_results`, `ui.searching`, `ui.load_failed`,
  `ui.add_option`, `ui.edit`, `ui.editing` and `ui.save_failed`;
- the infolist: `ui.yes`, `ui.no`, `ui.show_more` and `ui.since.*` (`now`, `past`, `future`,
  `minutes`, `hours`, `days`, `months`, `years`);
- dashboards: `ui.chart.show_data`, `ui.chart.other`, `ui.stat.vs_previous` and
  `ui.period.*` (`label`, `7d`, `30d`, `90d`, `12m`, `mtd`, `ytd`);
- the notification bell: `ui.notifications.*`;
- the data grid: `ui.grid.*` ([docs/grid.md](grid.md#translations)).

The full list, with the English texts, is in `crates/renox-core/src/i18n.rs` (`builtin`).

### Changing the kit itself

To change the kit's HTML or its styles, copy the kit into the app:

```sh
rnx make:component --ui   # my-app ui:publish: components/ui.html and public/css/renox-ui.css
```

Then import from `"components/ui.html"` instead of `"renox/ui.html"`. In the layout, load the
copied stylesheet, and only the kit's script, so the styles aren't loaded twice:

```html
<link rel="stylesheet" href="{{ asset('css/renox-ui.css') }}">
{{ renox_ui(styles=false) }}
```

> [!WARNING]
> Once you copy the kit, it's yours: Renox updates no longer change your copy. To only change
> colours, override tokens instead (next paragraph).

Every class starts with `rx-`, and nothing in the kit styles bare elements (a plain `<h1>` or
`<a>` with no class). So it sits happily next to an app's own CSS.

To rebrand the kit, override its tokens on `:root`, for example `--rx-accent: #0a7d5a;` for a
green accent.

### Themes and type

The look is a set of tokens on `:root` (in renox-ui.css):

- the colours;
- the fonts;
- radii (how round the corners are) and shadows;
- the hairline on surfaces;
- the button shape;
- and a type scale (the sizes of text).

The default theme is "warm". `data-rx-theme="classic"` on `<html>` brings back the kit's
first look: system fonts, Apple's web blue, cool greys, pill-shaped buttons, no hairlines and
no small capitals.

```html
<html lang="{{ app.locale }}" data-rx-theme="classic">
```

An app's own `:root` tokens, in a stylesheet loaded after `renox_ui()`, win over either
theme. So a brand colour stays when the theme changes. examples/shop keeps its brown this
way, and examples/backoffice a colour from its settings.

### The type scale

The type scale has eight roles. Each one is a whole `font` (weight, size, line height,
family) in `rem`. Because it uses `rem`, it follows the reader's own text size setting.

The kit's components use these roles, and an app can too: `font: var(--rx-type-heading)`.

| Token | For | Warm | Classic |
|---|---|---|---|
| `--rx-type-display` | the figure that matters: a stat, a total | Poppins 600, 32 px | system 600, 28 px |
| `--rx-type-title` | a page's title (`rx-title`, `page_header`) | Poppins 600, 28 px | system 700, 22 px |
| `--rx-type-heading` | a card, widget, sheet or grid title | Poppins 600, 17 px | system 600, 19 px |
| `--rx-type-lead` | the line under a page's title | Inter 400, 16 px | system 400, 15 px |
| `--rx-type-body` | text, table cells, fields | Inter 400, 15 px | system 400, 17 px |
| `--rx-type-label` | a field's label | Inter 600, 13 px | system 600, 15 px |
| `--rx-type-note` | hints, descriptions | Inter 400, 13 px | system 400, 13 px |
| `--rx-type-caption` | small capitals over figures and table columns | Inter 600, 11 px, uppercase | system 500, 13 px |

(600 and 700 are font weights: how bold the text is. 400 is normal.)

What the warm theme makes stand out:

- A stat's figure is the largest thing on its card. On a small card it shrinks rather than
  breaking in the middle of the number. Its change is shown in a tinted pill.
- Table and grid headings, and stat and infolist labels, are in small capitals.
- A table's total row and a card's price use the title font.

### Fonts

The fonts are Latin subsets under the SIL Open Font License 1.1, about 72 KB in all. Renox
serves them from `/_renox/fonts/…`, and browsers keep them in their cache for good.
`renox_ui()` asks the browser to load the text font early (it "preloads" it).

Other writing systems fall back to the system font. An app that wants other fonts sets
`--rx-font` and `--rx-font-display` on `:root`, and also the `--rx-type-*` tokens, since they
name the font family.

### Infolists: read-only details

A record's page (an order, a customer) is mostly labels and values: "Status: Paid",
"Total: Rp 75,000". The kit calls this an **infolist**.

- `infolist` lays the entries out in a grid: one column on phones, `columns` from tablet
  width up.
- `entry` formats each value by its kind: a date, money, a badge, a link…

So a page doesn't need to write that HTML by hand:

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

This card shows an order in two columns:

- the status as a coloured badge, with a friendly name ("Waiting for payment");
- when it was placed, as "3 hours ago";
- the total as money;
- the invoice number, with a copy button and a link;
- up to three tags as badges;
- the note as Markdown, across the whole width, or "No note";
- the customer, as a link written by hand;
- and the order's lines, each in its own small box.

> [!NOTE]
> **Coming from Laravel:** "infolist" is Filament's name for it, and the kit's version works
> much the same way.

| | |
|---|---|
| `infolist(columns=1, inline=false)` | The `<dl>` (description list) around the entries. `inline=true` puts each label beside its value (from tablet width up). |
| `entry(label, value, format=…, …)` | A label and its value. With `{% call entry(label) %}…{% endcall %}` the block is the value. `span=2` or `"full"` makes it wider. `inline=true` puts this label beside its value. `hide_label=true` keeps the label for screen readers only. `hint` adds a line under the value, `tooltip` a title shown on hover. `id` sets the entry's `id`, for a link or a script to find it. |
| `format` | `"date"` and `"datetime"` (with `date_format`, using chrono's codes; in `APP_TIMEZONE`). `"since"`: "3 hours ago", with the date as its tooltip. `"money"`: in `APP_CURRENCY`, or `currency="USD"`. `"number"` (with `decimals`). `"markdown"`. `"bool"`: a check and "Yes", or a cross and "No". `"color"`: a colour sample and its code. `"image"`: a URL (with `image_size`, `circular`). `"key_value"`: pairs or a map, such as `KeyValues`, as a table. |
| `badge`, `labels` | `badge=true`, a kind (`"success"`), or kinds by value (`{"paid": "success"}`). `labels` gives raw values friendly names (`{"paid": "Paid"}`), with or without a badge. |
| `url`, `new_tab`, `copyable` | A link; a copy button (it copies the raw value). |
| `prefix_actions`, `suffix_actions` | Buttons before or after the value, a list of mappings: `label` (also the tooltip of an icon-only button), `icon` (a kit icon such as `"edit"`, `"external"`, `"refresh"`, `"trash"`, `"download"`; without one the label is shown), and what it does: `url` (a link, with `new_tab`), `action` (a small form posted there, with the CSRF token; `method` `"PUT"`, `"PATCH"` or `"DELETE"` for the others), or neither, with `attrs` for htmx (`{"hx-post": …, "hx-confirm": "Sure?"}`). `variant` (`"plain"` by default) and `disabled_reason` as for buttons. |
| `prefix`, `suffix`, `limit`, `words`, `placeholder` | Text before or after the value. At most `limit` letters, or `words` words, then "…". `placeholder` is what an empty value (none, `""`, an empty list) shows instead (by default `—`). |
| A list as `value` | Each item is formatted the same way. By default they're joined with commas; `list="lines"` puts one per line, `list="bullets"` makes a bullet list. Badges, colour samples and images sit in a row. `limit_list=3` shows three and folds the rest behind "Show 2 more" (a `<details>` element, no script needed). |
| `repeatable(label, items, columns=1, placeholder="—", span="full", hide_label=…)` with `{% call(item) %}` | A list of records inside the record (an order's lines). Each item is a small infolist with a border, made of the call block's entries. `placeholder` is shown when there are no items. |

An entry with buttons beside its value, such as a link to edit it and a form that acts on it:

```html
{{ entry("Email", user.email, copyable=true, suffix_actions=[
     {"label": "Write to them", "icon": "external", "url": "mailto:" ~ user.email},
     {"label": "Send the verification again", "icon": "refresh", "action": route('users.verify', user.id)}]) }}
```

Code shown coloured, read-only and copyable (`code_entry`), and editors for rich text, Markdown
and code, come from the `renox-editors` crate: see [editors.md](editors.md).

For sections and tabs, use the kit's own `card`, `fieldset` and `tabs`, and put an infolist
in each. examples/shop's order page and examples/fields' product page are built this way.

### Formatting values

Every template gets these **filters**. A filter changes a value before it's printed, written
after a `|`: `{{ order.total | money }}`. The infolist uses them too.

| Filter | Gives |
|---|---|
| `number`, `number(2)` | `75,000` in `en`, `75.000` in `es` or `de`: the page's language picks the separators. |
| `money` | The amount in `APP_CURRENCY` (default `IDR`): `Rp 75,000` (en), `Rp 75.000` (es), `$1,250.50` with `USD`. Options: `currency="USD"` for another currency, `decimals=0`, `divide_by=100` for amounts stored in cents. `renox::format_money` does the same in Rust; `renox::currency_decimals("USD")` (2) is a currency's usual decimals. |
| `date`, `date('%d/%m/%Y %H:%M')` | A date, with chrono's format codes. A moment in time (`created_at`) is shown in `APP_TIMEZONE`. |
| `since` | "3 hours ago", "in 2 days", "just now" (translated with `ui.since.*`). It uses the clock that `TestApp::travel` moves, so tests can check it. |
| `words(20)` | The first 20 words, then "…" (change it with `end="…"`). |
| `markdown` | Markdown (CommonMark, tables, strikethrough, task lists) turned into HTML. HTML inside the text is shown as text. A link or image to anything but `http(s)`, `mailto`, `tel` or a relative URL points nowhere. So it is safe for text that people typed. |

### Design principles

The kit follows the principles of the Human Interface Guidelines. These are the rules its
components follow by themselves. They're worth keeping in an app's own pages too:

- **Hierarchy** (the important thing stands out).
  - One primary button per form or page: the action people came to take.
  - Destructive actions in lists are red text (`plain-danger`). The filled red button waits
    in the confirmation sheet.
  - Secondary text is lighter and smaller, but never lighter than WCAG AA allows. (WCAG is
    the web's accessibility standard; AA is the level most sites aim for.)
- **Clarity.**
  - Labels are always visible. Placeholders are examples, not labels.
  - A hint says what's expected before anyone gets it wrong.
  - Errors are plain sentences, next to the field and in a summary above the form.
  - Optional fields are marked, since required ones are the norm.
- **Feedback** (people see that something happened).
  - A button shows it's working (a spinner, `aria-busy`) and can't be pressed twice.
  - A toast confirms what happened and names the thing ("“Espresso” moved to the trash").
  - Success and info toasts go away by themselves, but wait while the mouse is over them or
    they have focus. Error toasts stay until dismissed. Escape closes the toast that has the
    focus, else the newest one, once nothing else that Escape closes is open.
- **Forgiveness** (mistakes are easy to avoid and undo).
  - Destructive actions ask first, with Cancel focused.
  - Live validation checks a field when you leave it, then again as you correct it. It never
    complains about a field someone is still typing in.
- **Deference** (the design steps back so the content comes first).
  - Content stays plain.
  - See-through ("translucent") surfaces are only for what floats over the content: the
    navigation bar, menus, toasts, and the dark layer behind a sheet.
- **Accessibility.**
  - Text contrast is at least 4.5:1, in both light and dark mode. The default accent, indigo
    #4F46E5, keeps 6.3:1 under white text. (The classic theme's is #0071E3, since Apple's
    system blue falls short.)
  - Every control has a 44 × 44 pt target (big enough for a finger) and a visible focus ring
    for keyboard users.
  - Hints and errors are tied to their fields with `aria-describedby`, and errors are
    announced by screen readers.
  - Menus, tabs and sheets work with the keyboard.
- **Respect for settings** (the person's device settings win).
  - Dark mode follows the system. `data-theme="light|dark"` on `<html>` overrides it.
  - Reduce Motion stops the animations, Reduce Transparency makes see-through surfaces solid,
    and Increase Contrast darkens separators and secondary text.
  - Text sizes are in `rem`, so they follow the reader's settings.
  - Sheets and toasts stay clear of the phone's notch and home indicator (the "safe areas").

### Error pages

When a page isn't found (404), isn't allowed (403) or something breaks (500), the app shows
an error page.

`rnx new` writes `resources/views/errors/default.html` for this. It's your layout with the
kit's `empty` component and a link home, so a 404 or a 403 keeps the navigation bar.

- `errors/<status>.html` (for example `errors/404.html`) replaces it for one status.
- The page gets `status`, `reason` and `detail`, besides the usual globals. With `APP_DEBUG`
  on, it also gets `request_line` and `template`.
- Renox's own error page (used when an app has none) is built on the kit too.

## Toasts

A **toast** is a short message that pops up and then goes away, like "Product saved.". To
show one, return a `Toast` with the response:

```rust
use renox::prelude::*;

/// Saves (in a real app), then goes back to the list with a "saved" toast.
async fn save() -> (Toast, Redirect) {
    (Toast::success("Product saved."), Redirect::to("/products"))
}
```

The handler returns two things at once (a tuple): the toast and a redirect.

How it reaches the screen:

- After a redirect, the toast waits in the session. `{{ toasts() }}` shows it on the next
  page, once.
- For an htmx request, it rides in the `HX-Trigger` header (as the event `renox:toast`) and
  appears at once.

Kinds: `success`, `info`, `warning` and `error`. An `error` toast is announced as an alert
by screen readers, and stays until dismissed.

### A toast that says more

A toast can carry more: a second line, links, buttons, a duration.

```rust
use renox::prelude::*;
use renox::ToastAction;

/// Places an order, then shows a toast with links and an Undo button.
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

> [!NOTE]
> **Coming from Laravel:** this is like Filament's notifications. Instead of
> `session()->flash('status')` and a toast library, you return a `Toast`.

The details:

- Every action closes the toast.
- A link goes only to http(s), mailto, tel or a relative URL.
- An event action sends ("dispatches") its event on `document`, and `detail.toast` is the
  toast's id. So an htmx element can listen for it with
  `hx-trigger="order-undo from:document"`.
- `seconds(n)` sets how long it stays (for errors too). `persistent()` keeps it until
  dismissed. Every toast waits while the mouse is over it or it has focus.
- A toast with an `id` replaces an earlier one with the same id.
- `{{ toasts(position="bottom-end") }}` moves them: `top` (the default, centred),
  `top-start`, `top-end`, `bottom`, `bottom-start` or `bottom-end`. Phones keep them centred.
- From the page's own JavaScript: `Renox.toast({kind: "success", message: "Copied", body: "…",
  actions: [{label: "Open", url: "/x"}], duration: 3000, id: "copy"})` and
  `Renox.dismissToast("copy")`.

### A button that sends a request

A link only goes somewhere. A **request action** does something: "Undo", "Retry", "Approve".
It sends a `POST`, `PUT`, `PATCH` or `DELETE` to a path of your app, with the CSRF token, and
stays on the page.

```rust
use renox::prelude::*;
use renox::ToastAction;

/// Archives an order (htmx), with an Undo button in the toast.
async fn archive(Path(id): Path<i64>) -> Toast {
    // … archive …
    Toast::info(format!("Order #{id} archived"))
        .action(ToastAction::delete("Undo", format!("/orders/{id}/archive")))
}

/// What Undo sends: `DELETE /orders/{id}/archive`. The answer is a toast too.
async fn unarchive(Path(id): Path<i64>) -> Toast {
    // … restore …
    Toast::success(format!("Order #{id} is back"))
}
```

The details:

- `ToastAction::post(label, url)`, `put`, `patch` and `delete` make one. In JSON (for
  `Renox.toast` or a stored notification) it is `{"label": "Undo", "url": "/orders/7/archive",
  "method": "DELETE"}`.
- Only paths of your own site (`/orders/7`) are sent, never another address: the request
  carries the CSRF token, which must not leave the site. Others aren't shown.
- renox-ui.js sends it with htmx, swapping nothing. So answer it like any htmx request that
  doesn't swap: a `Toast` (a `204` carrying it), `(Toast, HxRefresh)` to reload the page, an
  `HxRedirect`, or an `HxTrigger` for an event of your own. A plain `Redirect` would be followed
  quietly, and its toast lost.
- If the request fails (a 4xx or 5xx, or no network) and the answer brought no toast of its
  own, the page shows an error toast: "That didn't work. Try again." (`ui.request_failed`).
- The same button works in the bell's list (a `DatabaseMessage` action); without JavaScript
  it is a plain form there.
- From the page's own JavaScript: `Renox.request("POST", "/orders/7/retry")`.
- A toast pushed from the server to open pages (`state.broadcast_to(user_id, "renox:toast",
  json!({ "toasts": [toast] }))`, see [mail.md](mail.md#your-own-live-events)) can carry one
  too: [examples/jobs](../examples/jobs) tells the staff about a failed charge with a
  "Reopen" button.

Toasts go away. For notifications that stay (a bell in the navigation bar, with new ones
arriving live), see [mail.md](mail.md#the-bell).

## Fragments and out-of-band swaps

When htmx asks for a page, it often needs only one part of it: the new rows of a table, say.
That part is a **fragment**.

- `view(…).fragment("rows")` sends only the block named `rows`, for htmx requests that aren't boosted (`hx-boost` requests get the whole page, since htmx swaps the body).
- `.also("count")` adds more blocks after it.
- `.status(StatusCode::CREATED)` sends the page (or the fragment) with another status than
  200.

An extra block can update a different place on the page. This is an **out-of-band swap**:
"out of band" means outside the main spot htmx was going to update. Give the extra block's
outer element an `id` and `hx-swap-oob="true"`. htmx then puts it into the element on the page
with that same id:

```html
{% block rows %}<tr id="order-{{ order.id }}">…</tr>{% endblock %}
{% block count %}<span id="order-count" hx-swap-oob="true">{{ count }}</span>{% endblock %}
```

```rust
# use renox::prelude::*;
# fn demo(count: i64) -> View {
// Send the `rows` block, plus the `count` block, which htmx swaps into #order-count.
view("orders/index.html", context! { count }).fragment("rows").also("count")
# }
```

So one answer updates the table row and the order count at the top of the page.

> [!NOTE]
> **Coming from Laravel:** this is like `@fragment` / `fragments([...])`.

## htmx response headers

htmx reads special headers in the answer to decide what to do next. In Renox these headers
are types. Return them in a tuple with the response:

- `HxRetarget("#errors".into())`: swap the answer into another element;
- `HxReswap("outerHTML".into())`: swap it in a different way;
- `HxPushUrl("/orders?status=open".into())`: change the address in the browser's address bar;
- `HxRedirect`, `HxRefresh` and `HxTrigger`: go to another page, reload this one, or send an
  event to the page.

Each of them holds a `String`, so write `.into()` after a text in quotes. `HxRetarget`,
`HxReswap` and `HxPushUrl` aren't in the prelude: import them with
`use renox::{HxPushUrl, HxReswap, HxRetarget};`.

```rust
# use renox::prelude::*;
use renox::{HxReswap, HxRetarget};

/// Puts the order list in place of the whole `#orders` element.
async fn reload() -> (HxRetarget, HxReswap, View) {
    (
        HxRetarget("#orders".into()),
        HxReswap("outerHTML".into()),
        view("orders/index.html", context! {}).fragment("orders"),
    )
}
```

### What htmx sent: the `Htmx` extractor

The other way round, `Htmx` (in the prelude) tells a handler what htmx sent with the request.
Ask for it as an argument. Its fields:

- `request`: `true` when htmx made the request (the `HX-Request` header);
- `boosted`: `true` when it came from an `hx-boost` link or form, which expects a whole page;
- `target`: the id of the element the answer goes into (`HX-Target`), if any;
- `trigger`: the id of the element that started the request (`HX-Trigger`), if any;
- `current_url`: the address the browser shows now (`HX-Current-URL`), if any.

And two helpers:

- `htmx.wants_fragment()` is `true` for an htmx request that isn't boosted: one that wants a
  part of the page, not all of it. That's the same test `.fragment(…)` makes.
- `htmx.redirect("/orders")` goes to another page after a form is sent: an `HX-Redirect`
  header for htmx (the browser loads the whole page), a `303 See Other` redirect otherwise.

```rust
# use renox::prelude::*;
/// Sends only the list to htmx, and the whole page to everyone else.
async fn index(htmx: Htmx) -> View {
    let page = view("products/index.html", context! {});
    if htmx.wants_fragment() { page.fragment("list") } else { page }
}

/// After saving, goes to the list, whether htmx sent the form or not.
async fn store(htmx: Htmx) -> Response {
    htmx.redirect("/products")
}
```

### Going back

`Back` (in the prelude) is both an extractor (an argument the handler asks for) and a
response. It redirects to the previous page (from the `Referer` header). When there's no
previous page, or it's on another site, it goes to `/` instead. So nobody can use it to send
your visitors to a stranger's site (an "open redirect").

```rust
# use renox::prelude::*;
/// Saves (in a real app), flashes "Saved", and goes back to the page the form was on.
async fn store(back: Back, session: Session) -> Result<Back> {
    // A flash message lives for one request: the next page can show it.
    session.flash("status", "Saved")?;
    Ok(back)
}
```

## Live validation

Live validation checks a form's fields while people fill it in, before they press Submit.

Add `data-live-validate` to a form that `Valid<T>` handles. Then:

1. When someone leaves a field, the kit's script sends that field's value (with the rest of
   the form), with the header `X-Renox-Validate: <field name>`. It sends it again as the field is
   corrected.
2. The `Valid` extractor answers with that field's errors as JSON, and **the handler doesn't
   run**. So nothing is saved until the form is really submitted.
3. The kit shows the errors next to the field.

The rules are the form's own, including database checks such as `unique`. You don't write
them twice.

> [!NOTE]
> **Coming from Laravel:** this is like Precognition.

## The current route, conditional classes, loops

A navigation bar marks the current section with `route_is`. It takes the route's name, and
`*` stands for anything.

`class_names` builds a `class` attribute. It always includes the plain classes, and each
class in the `{…}` map only when its condition is true:

```html
<a href="{{ route('admin.products.index') }}"
   class="{{ class_names('tab', {'tab-active': route_is('admin.products.*')}) }}"
   {% if route_is('admin.products.*') %}aria-current="page"{% endif %}>Products</a>
{# request.route is the name itself, e.g. "admin.products.edit" #}
```

On any admin products page, this link gets `class="tab tab-active"` and `aria-current`.
Elsewhere, it's just `class="tab"`.

- `route_is` also takes several patterns: `route_is('orders.*', 'checkout')`.
- In Rust, the `CurrentRoute` extractor does the same: `route.is("admin.*")`, `route.name()`.

### Loops that stop early

Loops can stop early with `{% break %}`, or skip an item with `{% continue %}`:

```html
{% for product in recently_viewed %}{% if loop.index > 4 %}{% break %}{% endif %}…{% endfor %}
```

This shows at most four products: on the fifth, the loop stops.

### Texts that depend on a number

Some texts change with a count: "Sold out", "Only one left", "12 in stock". The lang file
holds all the versions in one line, using Laravel's plural ranges:

- `{n}` for exactly n;
- `[a,b]` for a range from a to b;
- `*` for "no end".

For example: `"{0} Sold out|{1} Only one left|[2,5] Only :count
left|[6,*] :count in stock"`. Print it with
`{{ t('products.in_stock', count=product.stock) }}` (examples/shop).

## Actions

An **action** is a button that does one thing, often after asking for a few values. Think
"Adjust stock", which asks how many to add or take away.

`action_sheet` is all of it at once: the button, a sheet, and the form inside the sheet. The
fields go in its call block:

```html
{% from "renox/ui.html" import action_sheet, input %}
{% call action_sheet("stock-" ~ product.id, "Stock", route('admin.products.stock', product.id),
                     "Adjust stock of " ~ product.name, description="A negative number takes away.",
                     submit_label="Adjust stock", method="PUT", icon="box", size="small") %}
  {{ input("change", "Change", type="number", required=true, id="stock-change-" ~ product.id) }}
{% endcall %}
```

This makes a small "Stock" button with a box icon. Clicking it opens a sheet titled "Adjust
stock of …", with one number field and an "Adjust stock" button. The form is sent with
`PUT` to the product's stock route.

> [!NOTE]
> **Coming from Laravel:** these are Filament's actions, as the kit has them.

What happens when the form is sent:

- It's sent with htmx. `PUT`, `PATCH` and `DELETE` go through `_method`.
- A 422 answer (the form had errors) shows its messages under the fields, inside the sheet,
  which stays open.
- Any other success closes the sheet and resets the form. Cancel and Esc do that too.

The handler is an ordinary one. It can answer with:

- a `Toast`, to say what happened;
- `HxRefresh`, to reload the page (the toast waits in the session);
- `HxTrigger`, to tell other parts of the page;
- or, with `target` and `swap` on the action sheet, HTML that replaces part of the page.

Some checks can't be written as rules, because they need the database. The handler can
answer with a `ValidationError`, which shows up under its field the same way:

```rust
use renox::prelude::*;
use renox::HxRefresh;

/// The action's form: how much to add (or, if negative, take away).
#[derive(serde::Deserialize, serde::Serialize)]
struct StockForm { change: i64 }

/// The form's rules: a number is required, and it can't be 0.
impl Validate for StockForm {
    fn rules(&self, v: &mut Validator) {
        v.field("change", &self.change).required().rule(self.change != 0, "Type how many.");
    }
}

/// Changes a product's stock, refuses to go below zero, and reloads the page.
async fn adjust_stock(Path(id): Path<i64>, Valid(form): Valid<StockForm>) -> Result<(Toast, HxRefresh)> {
    let stock = 3; // the product's, from the database
    // Taking away more than there is: answer with an error under the "change" field.
    if stock + form.change < 0 {
        let mut errors = Errors::new();
        errors.add("change", format!("Only {stock} in stock to take away."));
        return Err(ValidationError::new(errors).with_input(&form).into());
    }
    // All good: a toast says what happened, and the page reloads.
    Ok((Toast::success(format!("Product {id}: {} in stock.", stock + form.change)), HxRefresh))
}
```

The action sheet's other options:

- for the button: `variant`, `size`, `icon` and `key`;
- for the sheet: `slide_over`, `width` and `modal_icon`;
- `danger=true` for a red submit button;
- `target`/`swap` for htmx;
- `enctype="multipart/form-data"` for a file field.

> [!IMPORTANT]
> When a page has one action sheet per row, give each its own `id`, and its fields their own
> `id=`, as in the example above. Otherwise two rows' sheets get mixed up.

### What every button can carry

These work on every button (`button`, `link_button`, `open_button`, `icon_button`), except
the disabled reason at the end of the list:

- **An icon** before the label, by name: `icon="plus"`. The kit's icons are `plus`, `edit`,
  `trash`, `check`, `close`, `copy`, `download`, `upload`, `external`, `refresh`, `search`,
  `settings`, `more`, `box`, `calendar`, `eye`, `up`, `down`, `prev`, `next`, and the
  status icons `info`, `success`, `warning`, `error`.
- **A count**: `badge=3`. A 0 is shown; an empty string or `none` isn't.
- **A keyboard shortcut**: `key="mod+s"`.
  - `mod` is ⌘ on a Mac and Ctrl elsewhere. You can also use `ctrl`, `alt`, `shift`, and keys
    like `enter`, `esc`, `backspace`.
  - The shortcut clicks the first visible element with that key (inside the open sheet, when
    there is one).
  - A key without `mod`, `ctrl` or `alt` doesn't fire while someone is typing in a field.
  - The button gets `aria-keyshortcuts`, and its tooltip (or `title`) names the shortcut.
- **A reason it's disabled**: `disabled_reason="Add a photo first."` The button can still get
  focus and shows the reason as its tooltip (on a tap too), but a click does nothing. Plain
  `disabled=true` instead takes it out of the tab order entirely. Only `button` and
  `icon_button` (without `href`) take `disabled` and `disabled_reason`; `link_button` and
  `open_button` take neither.

An `icon_button`'s `label` is read out by screen readers. It's also shown as a tooltip after
a short hover, or at once when the button gets keyboard focus. Any element can have a tooltip
with `data-rx-tip="…"`.

examples/shop's admin product list has all of these:

- an "Adjust stock" action on each row;
- an edit `icon_button`;
- a "view in the shop" button, disabled with a reason for hidden products;
- the "New product" button on the `n` key.

Its product form also saves on ⌘S / Ctrl+S.

### Action groups

When a page has more actions than it has room for, put the rarer ones behind one button.
`action_group` is a `menu` made for that (Filament's ActionGroup): with no label it's a "⋯"
icon button that screen readers call "Actions".

```html
{% from "renox/ui.html" import action_group, menu_link, menu_open, menu_action, menu_separator, confirm %}
{% call action_group() %}
  {{ menu_link(route('products.replicate', product.id), "Duplicate", icon="copy") }}
  {{ menu_link(route('products.ledger', product.id), "Export ledger", icon="download", download=true) }}
  {{ menu_action(route('products.archive', product.id), "Archive") }}
  {{ menu_separator() }}
  {{ menu_open("delete-product", "Delete", icon="trash", danger=true) }}
{% endcall %}
{{ confirm("delete-product", "Delete", route('products.destroy', product.id),
           "Delete " ~ product.name ~ "?", "This can't be undone.", button=false) }}
```

- `action_group("More")` shows a labelled button instead; `icon` changes the icon.
- Its items: `menu_link` (a link; `download=true` for a file to save, `new_tab=true`),
  `menu_action` (sends a form), `menu_button` (a button with htmx `attrs`), `menu_open`
  (opens a sheet), `menu_separator()`, and `menu_section(title)` around items that belong
  together. Each takes an `icon`; `danger=true` shows it in red.
- `menu_open(id, label)` opens a sheet made elsewhere on the page with `button=false`:
  an `action_sheet`, `confirm`, `wizard_action` or `import_action` without its own button.
  The menu closes, and when the sheet closes the focus goes back to the group's button.
- The keyboard works as in every kit menu: Enter, Space or ↓ opens it on the first item,
  ↑ on the last; the arrows, Home and End move; Esc closes it and returns to the button.

### An action with steps

`wizard_action` is an `action_sheet` whose form is a [`wizard`](#form-fields): one step at a
time, Next checks the step's fields (with the server's rules too: the form is
`data-live-validate`, `live=false` turns that off), and the last step's button sends the
form with htmx.

```html
{% from "renox/ui.html" import wizard_action, wizard_step, input %}
{% call wizard_action("new-product", "New product", route('products.store'), "New product",
                      [["details", "Details"], ["stock", "Price and stock"]],
                      submit_label="Add product", variant="primary", icon="plus") %}
  {% call wizard_step("new-product", "details") %}
    {{ input("sku", "SKU", required=true, id="new-product-sku") }}
    {{ input("name", "Name", required=true, id="new-product-name") }}
  {% endcall %}
  {% call wizard_step("new-product", "stock") %}
    {{ input("price", "Price", type="number", required=true, id="new-product-price") }}
  {% endcall %}
{% endcall %}
```

The handler is the same as for any form. A 422 after the last step opens the first step with
an error and focuses the field, so a SKU someone else took in the meantime sends the user back
to step 1. A success closes the sheet; closing it any way starts the wizard over.

### Import

`renox::import` reads a CSV file row by row, **as if each row were a form**: the columns, named
by the file's first line, fill a struct with `Deserialize` and `Validate`, and its rules (and
its `prepare` and `after` hooks) check the row. Rows that pass are written by your closure in
one transaction, a savepoint each, so a row the database refuses (a unique index, a foreign
key) undoes only its own writes. The others are reported with their row number, as a
spreadsheet counts (the line of column names is row 1).

```rust
# use renox::prelude::*;
use renox::import::{Import, ImportReport};
# #[derive(Model, serde::Serialize, Default)]
# #[model(table = "products")]
# struct Product { id: i64, sku: String, name: String, price: i64 }

/// One row: `sku,name,price`.
#[derive(serde::Deserialize, Validate)]
struct ProductRow {
    #[validate(required, max = 30, alpha_dash)]
    sku: String,
    #[validate(required, max = 100)]
    name: String,
    #[validate(required, min = 0)]
    price: i64,
}

/// The import sheet's form: one CSV file.
#[derive(serde::Deserialize, Validate)]
struct ImportForm {
    #[validate(required, mimes(&["csv", "txt"]))]
    file: Option<Upload>,
}

async fn import(
    State(state): State<AppState>,
    lang: Lang,
    Valid(form): Valid<ImportForm>,
) -> Result<ImportReport> {
    let file = form.file.ok_or(Error::NotFound)?;
    Import::csv(file.bytes())
        .lang(&lang) // the rules' messages in the user's language
        .run(&state, |tx, row: ProductRow| {
            Box::pin(async move {
                let product = Product { sku: row.sku, name: row.name, price: row.price, ..Default::default() };
                Product::create(tx, product).await?;
                Ok(())
            })
        })
        .await
}

/// The sheet's "Download a template" link: the columns, nothing else.
async fn template() -> renox::Download {
    renox::import::template("products.csv", &["sku", "name", "price"])
}
```

The page's side is `import_action`, a sheet with a file field and room for the report:

```html
{% from "renox/ui.html" import import_action %}
{{ import_action("import-products", "Import", route('products.import'), "Import products",
                 columns=["sku", "name", "price"], template_url=route('products.template')) }}
```

What the person sees:

- Every row imported: the sheet closes, the page reloads, and a toast says how many.
- Some rows refused: a table of row numbers and messages appears in the sheet, which stays
  open. Closing it reloads the page, if some rows went in.
- A file that can't be read (not UTF-8 text, no rows, too many rows) is an error under the
  file field, like any other.

`Import`'s options:

- `.all_or_nothing()`: one bad row and nothing is written;
- `.delimiter(';')`: for files from a spreadsheet set to a European language;
- `.headers(&["sku", "name"])`: for a file without a line of column names;
- `.rename("Description", "name")`: a column under another name. Headings are matched
  ignoring case, and spaces and dashes become `_` ("Unit price" fills `unit_price`);
- `.max_rows(n)`: 10,000 by default;
- `.user(&user)`: the user that the rows' `after` hooks see.

The `ImportReport` has `imported`, `failed` (each with `row` and `errors`), `is_clean()` and
`summary()`, for a command or a job that imports without a page. Rows are checked before the
transaction starts, so a `unique` rule can't see a repeat within the same file: the database's
unique index catches that one, and the report says the row repeats a unique value.

### Duplicate (replicate)

`Model::replicate()` copies a record into one that isn't saved yet: the same values, with no
id, no `deleted_at` and no timestamps. Show it in the "new" form for someone to finish:

```rust
# use renox::prelude::*;
# #[derive(Model, serde::Serialize, Default, Clone)]
# #[model(table = "products")]
# struct Product { id: i64, sku: String, name: String, price: i64 }
/// GET /products/{id}/replicate: the new-product form, filled from a copy.
async fn replicate(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let original = Product::find_or_404(&db, id).await?;
    let mut product = original.replicate();
    product.sku.clear(); // unique: the person types a new one
    product.name = format!("{} (copy)", original.name);
    Ok(view("products/replicate.html", context! { original, product }))
}
```

The form's fields take their values from it (`input("name", "Name", value=product.name)`) and
post to the ordinary "store" route. Code that saves the copy itself calls `copy.save(&db)`. In a
data grid, it's a row action: `Action::link("Duplicate", "/products/{id}/replicate")`.

### Export

Data grids export what they show ([docs/grid.md](grid.md#exports)). For a file outside a grid's
page, such as one product's ledger from its page, `Grid::export_as` takes any query and a
format. The grid only says which columns: those shown by default, in their order. See
[docs/grid.md](grid.md#exports-outside-the-grids-page).

```html
{{ menu_link(route('products.ledger', product.id), "Export ledger (CSV)", icon="download", download=true) }}
```

examples/backoffice uses all of these: "New product" is a `wizard_action`, "More" on the
product list is an `action_group` with the `import_action` and its template, and a product's
page has an action group with "Duplicate" and "Export ledger".

## Dashboards

A dashboard shows figures, charts and the period of time they cover. Renox draws them on the
server as plain HTML and SVG (a format for drawings). There's no chart library and nothing
extra for the browser to load.

> [!NOTE]
> **Coming from Laravel:** these play the role of Filament's widgets.

First, the handler collects the numbers:

```rust
use renox::prelude::*;
use renox::chart::{Period, Trend};

/// An order.
#[derive(Model, serde::Serialize, Default)]
struct Order { id: i64, total: i64, status: String, created_at: Option<renox::db::DateTime> }

// `?period=30d`: 7d, 30d, 90d (any number of days up to 366), 12m (months up
// to 36), mtd, ytd; 30 days without one.
/// The dashboard page: sales, sales in the period before, and the number of orders.
async fn dashboard(State(state): State<AppState>, period: Period) -> Result<View> {
    // A query for paid orders, made fresh each time it's needed.
    let paid = || Order::where_eq("status", "paid");
    // The sum of `total` per day (or month) in the chosen period, and in the one before.
    let sales = Trend::of(paid(), "created_at").over(period).sum(&state, "total").await?;
    let before = Trend::of(paid(), "created_at").over(period.previous()).sum(&state, "total").await?;
    // How many paid orders there were per day (or month).
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

`Period` reads the period from the address: a preset (`?period=30d`, `12w`, `12m`, `ytd`…) or
a custom range (`?period=custom&from=2026-09-01&to=2026-09-30`, what `period_filter`'s
"Custom" form sends). Then the template draws them:

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

This page shows, from top to bottom:

- a row of buttons to pick the period;
- a row of figures: the revenue (with its change and a small trend line) and the refunds
  (where going down is good);
- a grid of cards: a sales chart, the orders "by status" (loaded from another URL and
  refreshed every 60 seconds), and a bar chart of orders across the full width.

The parts, one by one:

- **`Trend::of(query, column)`** places a model's rows in time, by a date-time column.
  - It gives a `Series` per day (for up to 92 days, and for `mtd`), per week (for `12w`-style
    periods, and custom ranges up to 26 weeks) or per month. `period.per(Bucket::Week)` picks
    the step yourself: `Period::days(90).per(Bucket::Week)`.
  - It can `count`, `sum(column)` or `average(column)`.
  - The query's conditions apply.
  - Days are cut in `APP_TIMEZONE` (at its offset at the end of the period). Empty days are 0.
  - Weeks are ISO weeks: Monday to Sunday, in those same local days. Each is labelled by its
    Monday (`2026-09-28`, shown as `Sep 28`). `12w` starts on the Monday eleven weeks before
    this week's and ends today; in a custom range the first and last weeks may be cut short
    by the range.
  - `Period::previous()` is the period just before, as long, for `Series::change_from` (a
    percent). For a custom range that's as many days, just before it.
  - `Period::between(from, to)` is a custom range in code (both days included, at most
    three years; `None` otherwise). Its key is `2026-09-01..2026-09-30`, which `?period=`
    takes too. A range the extractor refuses (`from` after `to`, a date that isn't one, more
    than three years) gives the default 30 days.
  - A `Series` has `labels` (`2026-10-02`, the Monday per week, or `2026-10` per month),
    `values`, `total()` and `named(…)`. Build one by hand with `Series::new(labels, values)`.
- **`chart(kind, data, …)`** draws a chart.
  - Kinds: `line`, `area`, `bar` (`stacked=true` piles bars on top of each other),
    `pie`/`doughnut`, or `scatter`/`bubble` (below).
  - `data` is a `Series`, a list of numbers, or a list of series (`{name, values}` maps).
    Or pass `labels=…` with `series=[…]` or `values=[…]`.
  - Options: `format` (`number`, `money` in `APP_CURRENCY` or `currency=…`, `percent`),
    `decimals`, `height` (240 px), `title` (for screen readers), `name` (one series' name),
    `x_format` (chrono's codes for date labels; otherwise `Oct 2`, `Oct 2026`),
    `x_title` and `y_title` (shown along the axes), `legend=false`, `table=false`, `id`.
  - Plain numbers get as many decimals as the data needs (up to 2) in tooltips and the
    data table, unless `decimals` says; `money` uses the currency's, `percent` one.
- **Scatter and bubble charts** place points by two numbers (and a bubble's size by a third):
  ```html
  {{ chart("scatter", points=orders, x_title="Items", y_title="Total", format="money") }}
  {{ chart("bubble", points=products, x_title="Price", y_title="Units", size_title="Revenue",
           x_format="money", size_format="money") }}
  ```
  - A point is `{x, y, size, label}` (a map from the handler, e.g. `json!({…})`), or `[x, y]`
    and `[x, y, size]`. A point without both numbers is left out.
  - Several series: `series=[{"name": "Coffee", "points": […]}, …]` (or the same list as
    `data`), each in its own colour with a legend.
  - `format` is for y, `x_format` for x and `size_format` for sizes: `number`, `money` or
    `percent`. Numbers get as many decimals as the data needs (up to 2) unless `decimals` says.
  - The axes fit the data (they start at 0 only when the data comes close to it), with
    grid lines both ways.
  - A bubble's area follows its size (6 to 40 px across); bubbles are see-through with a
    solid edge, and small ones are drawn over big ones.
  - The tooltip names the point (`label`), its series, and each value with its axis title;
    the arrow keys move from point to point, left to right. The "Show the data" table lists
    every point.
- **How the charts look.**
  - One axis that starts at 0, with clean tick labels (`12.5K`, or `2,5M` where the language
    writes a decimal comma).
  - A legend for two series or more.
  - A hairline grid; 2 px lines with a dot at the end.
  - Bars at most 24 px wide, with rounded ends.
  - A pie or doughnut shows at most six slices. With more, it keeps the first five and folds
    the rest into one "Other" slice.
  - The six series colours come in a fixed order, checked for colour blindness in both
    light and dark mode (`--rx-chart-1` … `--rx-chart-6`). A seventh series is grey.
- **Hover and keyboard.**
  - On line and area charts, a crosshair and one tooltip with every series at the nearest
    date. On bars and slices, a tooltip for each.
  - The chart can take focus, and the arrow keys, Home and End move along it.
  - Every chart also has a "Show the data" table, so no value is only in a colour or a
    tooltip.
- **`stat(label, value, delta=…, delta_label=…, good="up", trend=…, url=…, hint=…,
  decimals=1)`** is one figure.
  - `value` is written as it should read: `total | money`.
  - `delta` is its change in percent, with an arrow and its sign. It's green when it goes the
    `good` way: `"up"`, `"down"` or `"none"`. `decimals` is how many decimals the percent
    shows (1 by default).
  - `trend` draws a sparkline (a tiny line chart); `url` makes it a link.
  - `stats(columns)` sets stats side by side (two per row on phones).
- **`dashboard(columns)` + `widget(title, description=…, span=…, url=…, poll=…, id=…)`**:
  cards in a grid (one column on phones; `span=2` or `"full"` for wider cards). `id` names the
  widget's body (by default it is made from the title).
  - A widget's content is its call block, or what `url` answers (a small template, or a
    `View` fragment).
  - With `url`, the content is loaded after the page, and again every `poll` seconds. The old
    content stays, dimmed, until the new one arrives.
- **`period_filter(selected, options=…, label=…, custom=true)`**: one row of preset periods,
  over everything it applies to. `selected` is the handler's `Period` (the one now shown).
  `options` are `[key, label]` pairs; by default 7 days, 30 days, 90 days, 12 months and this
  year (`7d`, `30d`, `90d`, `12m`, `ytd`; `12w`, 12 weeks, is another). `label` is the name
  screen readers say ("Period"). Its links set `?period=` and keep the rest of the address's
  query. `query_with(period="7d")` builds such links in any template.
  - Next to the presets, a "Custom" button opens a small form with two of the kit's date
    fields (typed, or picked in the calendar) and "Apply". It sends
    `?period=custom&from=…&to=…`, keeping the rest of the query (`query_fields(…)`, below).
  - The button shows the range while one is chosen (`Sep 1 – Sep 30, 2026`). A range the
    extractor refuses opens the form again with what was typed and a message.
  - It works from the keyboard: Enter on the button opens it and puts the cursor in the first
    date, Tab moves on, Escape (or a click elsewhere) closes it.
  - `custom=false` leaves the button out.
  - `query_fields("period", "from", "to")` writes the current query as hidden inputs, without
    `page` and the keys named, for any GET form that should keep the page's other filters.

examples/shop's admin dashboard uses all of it: the period (with 12 weeks and a custom
range), four figures, revenue against the period before, orders per day, orders by status
(loaded on their own every minute), products sold as bubbles and orders as a scatter chart.

## Data grids

`renox::grid` is a data grid for dashboards and back offices: a big table that works on the
server. It fills its container and has:

- a filter in every column heading, sorting and pagination;
- frozen columns (they stay put when you scroll sideways) and grouped headings;
- columns each user picks, per screen size;
- editing in place, and actions on selected rows;
- summaries, groups and exports.

You define it in Rust and draw it with the `grid` macro of `renox/grid.html`.
[docs/grid.md](grid.md) is its guide, and examples/grid is a dashboard built on it.

`{{ sparkline(values) }}` (a small line or bar chart as inline SVG) works in any template,
not only in a grid.

> [!NOTE]
> **Coming from Laravel:** this plays the role of Filament's tables.

## Stacks

A page or a component often needs to add something to another part of the layout: a script
at the end of `<body>`, or a style or a `<meta>` tag in `<head>`.

Stacks solve this. The layout names the places with `stack`. Anything rendered for the page
adds to them with `push`.

The layout:

```html
{# layouts/app.html (rnx new's layout has both) #}
<head>… {{ stack('head') }}</head>
<body>… {{ stack('scripts') }}</body>
```

A component that needs a script:

```html
{# components/chart.html #}
{% macro chart(id, data) -%}
{% call push('scripts', once='chart') %}
  <script src="{{ asset('js/chart.js') }}" nonce="{{ csp_nonce() }}"></script>
{% endcall %}
<canvas id="{{ id }}" data-points="{{ data | tojson }}"></canvas>
{%- endmacro %}
```

The `<canvas>` goes where the component is used. The `<script>` goes to the end of `<body>`,
where the layout has `stack('scripts')`.

The rules:

- `push(name)` adds to the end of the stack, `prepend(name)` to the front.
- `once='key'` adds it only the first time that key is pushed to that stack on the page. So
  the script is added once, however many charts there are.
- Pushes work from the page's blocks, from included templates and from imported components.
- They even reach a stack that was written earlier, such as the one in `<head>`. The layout's
  head is written before the page's blocks run, so `stack` leaves a marker that is filled in
  once the page is done.
- Error pages (`errors/*.html`) have stacks too. Mails don't.

> [!WARNING]
> htmx fragments (`.fragment("rows")`) have no layout, so what they push goes nowhere. Put a
> fragment's script in the fragment itself.

> [!NOTE]
> **Coming from Laravel:** this is `@push('scripts')` / `@stack('scripts')`, `@pushOnce`
> and `@prepend`.

## Tailwind CSS

Tailwind is a CSS tool: you style things with small classes like `mt-4` (margin on top) or
`text-sm` (small text), and it builds a stylesheet with just the classes you used.

`rnx new shop --tailwind` sets it up in a new app. For an existing app, create
`resources/css/app.css`:

```css
@import "tailwindcss";
@source "../views";
```

The first line brings in Tailwind. The second tells it to look for class names in your
templates.

Then link the output in the layout: `<link rel="stylesheet" href="{{ asset('css/app.css') }}">`.

How it runs:

- `rnx serve` runs Tailwind in watch mode next to the app. It rebuilds `public/css/app.css`
  when a view changes, and the page reloads.
- `rnx build` builds it minified (made as small as possible) before compiling, so an
  embedded binary carries it. `rnx tailwind` builds it once (`--minify`, `--watch`).
- There's no Node.js needed. `rnx` downloads Tailwind's standalone program (v4, the version
  `rnx` pins) once into your cache, and checks its SHA-256 (a fingerprint that proves it's
  the right file). `rnx tailwind:install` does that ahead of time.
  `TAILWIND_BIN=/path/to/tailwindcss` uses another binary, and `RNX_CACHE_DIR` moves the
  cache.

> [!IMPORTANT]
> Commit `public/css/app.css` to git. The Dockerfile from `make:deploy` builds the app
> without Tailwind, so it needs the built file.

Tailwind and the kit together:

- The kit's `rx-*` rules sit outside Tailwind's cascade layers (its system for deciding which
  rule wins). So the components keep their look, and utilities (`mt-4`, `text-sm`,
  `md:grid-cols-2`) lay things out around them.
- Tailwind's reset changes bare elements (headings, lists, links without a class) in your own
  markup. Style those with utilities, or with the kit's classes (`rx-title`, `rx-link`).

## Coming from Laravel

If you know Laravel, this table maps what you know to Renox:

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
| Filament's ActionGroup, wizard actions | `action_group`, `wizard_action` |
| Filament's ImportAction, ReplicateAction, ExportAction | `renox::import` + `import_action`, `Model::replicate`, `Grid::export_as` |

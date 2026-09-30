# Views, components and the UI kit

Renox pages are MiniJinja templates sent as HTML, updated in place with htmx. This guide
covers:

- components (macros that see the request);
- the UI kit that ships with Renox (`renox/ui.html`);
- toasts;
- fragments and out-of-band swaps;
- htmx response headers;
- live validation;
- stacks (`push` / `stack`);
- Tailwind CSS.

## Components see the request

A component is a MiniJinja macro in a file of its own, imported where it's used. Inside it,
the same request helpers work as in the page itself:

- `old()`, `error()`, `errors`;
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
| `input(name, label, type=…, value=…, hint=…, required=…, autocomplete=…, placeholder=…, attrs={…}, id=…)` | A labelled text field with its hint and error. It is refilled after a failed submit, except for passwords. `id` tells apart two fields of the same name on one page. |
| `textarea`, `select(name, label, options, selected=…, placeholder=…)` | The same for longer text and for a choice. `options` are values or `[value, label]` pairs. |
| `checkbox(name, label, checked=…, switch=…)` | A checkbox, or an iOS-style switch for settings. The whole row is the target. |
| `button(label, variant=…, size=…, block=…)` | Variants: `primary`, `secondary`, `plain`, `danger` and `plain-danger`. The button shows a spinner while its form or htmx request is being sent. |
| `link_button(href, label, …)` | A link that looks like a button. |
| `card(title=…, subtitle=…)`, `group(title=…, footer=…)` | A surface, and an inset grouped list like Settings. |
| `alert(message, kind=…, title=…)`, `badge(text, kind=…)` | Kinds: `info`, `success`, `warning`, `error`. Each kind has its own icon, so color is never the only signal. |
| `form_errors(title=…)` | Every error of the last submit, above the form, each linking to its field. |
| `sheet(id, title)` + `open_button(id, label)` | A modal dialog. It closes on Esc and on a click on the backdrop, and focus returns to the button that opened it. On phones it rises from the bottom edge. |
| `confirm(id, label, action, title, message, confirm_label=…, method="DELETE")` | A destructive action behind a confirmation sheet, with Cancel focused first. |
| `menu(label)` + `menu_link`, `menu_action`, `menu_separator` | A menu of actions: arrow keys move, Esc closes. |
| `tabs(id, items, selected=…)` + `tab_panel(id, key, selected=…)` | A segmented control; the arrow keys, Home and End move between tabs. |
| `table(head, caption=…)` | A table in a card. A heading `["Total", "num"]` right-aligns its column, and `["Slug", "hide-narrow"]` hides it on phones. |
| `empty(title, message, action_href, action_label)` | What an empty list says, with the way to add the first item. |

Renox's own pages use the kit too: the sign-in pages (`renox/auth/*`: login, registration,
password reset, email verification, password confirmation, the account page) and the error
page. They have `stack('head')` and `stack('scripts')`; to change one, put a file with the same
name under `resources/views/renox/auth/`.

The kit's own texts ("optional", "Cancel", the error summary's title) come in English and
Indonesian. An app can change them in `lang/*.json`, under the keys `ui.optional`,
`ui.cancel`, `ui.close`, `ui.dismiss`, `ui.more` and `ui.errors_title`.

To change the markup or the styles, copy the kit into the app:

```sh
rnx make:component --ui   # my-app ui:publish: components/ui.html and public/css/renox-ui.css
```

Every class starts with `rx-`, and nothing in the kit styles bare elements, so it sits next
to an app's own CSS. To rebrand it, override the tokens on `:root`, e.g.
`--rx-accent: #0a7d5a;`.

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

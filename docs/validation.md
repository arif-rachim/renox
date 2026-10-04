# Forms and validation

A form in Renox is a struct. The `Valid<T>` extractor reads the request into it, runs its
rules, and calls the handler only when every rule passes. Invalid input never reaches your code.
The short version is in the [cheat-sheet](../CHEATSHEET.md) ("Form + validation"). Complete
forms are in [examples/hello](../examples/hello) (an upload and translated labels),
[examples/crud](../examples/crud) (`#[derive(Validate)]` with a `prepare` hook) and
[examples/fields](../examples/fields) (every input type, `each`, `one_of`, `distinct`).

## A form and its handler

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct ProductForm {
    name: String,
    price: i64,
    email: Option<String>, // optional: an empty input becomes None
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(100).unique("products", "name");
        v.field("price", &self.price).label("selling price").min(1_000);
        v.field("email", &self.email).email();
    }
}

async fn store(session: Session, Valid(form): Valid<ProductForm>) -> Result<Redirect> {
    // Every rule passed.
    let _ = (form.name, form.price, form.email);
    session.flash("status", "Product saved.")?;
    Ok(Redirect::to("/products"))
}
```

`v.field(name, &value)` starts the rules for one field. `name` is the form's input name: errors
are keyed by it, and its label in messages is the name with `_` turned into spaces, unless
`.label(…)` or a translation names it (see "Messages and translations").

Rules run in order, and a field stops at its first failure. Every rule except `required`,
`accepted` and `rule` skips a missing value: `None`, or text that is blank after trimming. So
`.email()` alone accepts an empty field, and `.required().email()` doesn't.

## What `Valid<T>` reads

| Request | Read from |
|---|---|
| `GET` / `HEAD` | the query string (search filters, a report's dates) |
| `application/x-www-form-urlencoded` | the form body |
| `multipart/form-data` | the form body; file inputs become `Upload` fields |
| `application/json` | the JSON body (it must be an object) |

Any other content type gets a 415, and malformed JSON a 400. Form bodies are read with
`serde_html_form`, so a repeated name (a multi-select, a group of checkboxes) fills a `Vec<T>`.
A form with a nested name (`lines[0][qty]`, as the UI kit's `repeater` and `key_value` send)
is read as a tree instead: `lines` fills a `Vec` of a struct, and every field still parses from
its text (see "Browser values").

The steps, in order:

1. Parse the input into `T`, with every field that doesn't parse turned into an error (see
   "Browser values").
2. `prepare` tidies the data.
3. `authorize` may refuse the request: a 403, before any rule runs.
4. `rules`, then the database checks (`unique`, `exists`) in one pass.
5. `after`, only when everything so far passed.
6. The handler, if no step added an error.

## When validation fails

The same failure gets one of two answers, depending on who asked:

- **htmx and JSON requests** (`HX-Request`, `Accept: application/json` or a JSON body) get
  `422 Unprocessable Entity` with the errors:

  ```text
  {"message": "The name field is required.", "errors": {"name": ["The name field is required."], "price": ["The selling price must be at least 1000."]}}
  ```

  `message` is the first error. The bundled `renox.js` (in `renox_head()`) shows each error next
  to its input: in the form's element with `data-error-for="field"` if there is one, otherwise
  in a `<p class="error">` inserted after the input. It sets `aria-invalid="true"` on the inputs
  and focuses the first invalid one in page order. The form is not swapped or reset, so what was
  typed stays. Errors on list items (`tags.1`) go to the item's input, or to the list's slot.

- **Plain form posts** get a `303` redirect back to the form's page (the `Referer`, if it's on
  the same site; otherwise `/`), with the errors and the submitted input flashed to the session
  for that next page. Passwords are never flashed: `password`, `password_confirmation`,
  `current_password` and `_token` are dropped.

Every field's errors are reported at once, including fields that didn't parse.

### Showing errors and old input in templates

Pages get these helpers (components imported from other files see them too):

| Helper | What it gives |
|---|---|
| `error('name')` | The field's first error, or `""`. For a list, `error('tags')` also shows the first error of an item (`tags.0`, `tags.1`, …). |
| `errors` | Every error: a map of field → list of messages. Empty (falsy) when there are none. |
| `old('name', default)` | What was submitted for the field before the redirect, else `default`, else `""`. |
| `has_old()` | Whether the previous request was a failed submit. An unticked checkbox sends nothing, so `old()` alone can't tell "unticked" from "not submitted yet"; the UI kit's checkboxes and radios use it. |

```html
<form method="post" action="{{ route('products.store') }}">
  {{ csrf_field() }}
  {% if errors %}<p role="alert">Please fix the fields below.</p>{% endif %}
  <label for="name">Name</label>
  <input id="name" name="name" value="{{ old('name', product.name) }}"
    {%- if error('name') %} aria-invalid="true"{% endif %}>
  <p class="error" data-error-for="name">{{ error('name') }}</p>
  <button>Save</button>
</form>
```

The same slot works for both answers: the redirect fills it when the page renders, and
`renox.js` fills it after an htmx post. The UI kit's fields (`input`, `select`, …) do all of
this themselves, and `form_errors()` lists every error at the top of the form with links to
the fields ([ui.md](ui.md)).

#### Two forms on one page: error bags

When a page has two plain forms with the same field names (a sign-in form beside a newsletter
form, both with `email`), give one of them a named bag, as Laravel's error bags do. Its errors
are then flashed in that bag: `error('email', bag='login')` shows them, `error('email')` (the
other form) stays empty, and `errors_in('login')` lists them all. The kit's fields take
`bag="login"`. htmx and JSON answers are the usual 422; a bag only matters to redirects.

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Validate)]
#[validate(bag = "login")] // or `const ERROR_BAG: Option<&'static str> = Some("login");` in `impl Validate`
struct SignIn {
    #[validate(required, email)]
    email: String,
}
```

```html
{{ ui.input("email", "Email", bag="login") }}      {# the sign-in form #}
{{ ui.input("email", "Your email") }}              {# the newsletter form #}
```

A `ValidationError` you return yourself goes in a bag with `.in_bag("login")`.

### Errors found after validation

A check that only the handler can make (stock ran out while the user was typing) returns a
`ValidationError`. It is answered exactly like a rule's failure:

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct OrderForm {
    product_id: i64,
    quantity: i64,
}

impl Validate for OrderForm {
    fn rules(&self, v: &mut Validator) {
        v.field("quantity", &self.quantity).min(1);
    }
}

async fn order(State(db): State<Db>, Valid(form): Valid<OrderForm>) -> Result<Redirect> {
    let stock: i64 = renox::db::sql("SELECT stock FROM products WHERE id = ?")
        .bind(form.product_id)
        .scalar(&db)
        .await?;
    if stock < form.quantity {
        let mut errors = Errors::new();
        errors.add("quantity", format!("Only {stock} left."));
        return Err(ValidationError::new(errors).with_input(&form).into());
    }
    Ok(Redirect::to("/cart"))
}
```

`with_input(&form)` refills the form. Without it, the input `Valid` read for this request is
used, so a `ValidationError` from a model's `saving` hook also sends the user back to a filled
form.

## Rules with `impl Validate`

`rules` is plain Rust, so conditions, loops and helper functions all work. Each rule is a method
on the field:

```rust
use renox::chrono::NaiveDate;
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct EventForm {
    title: String,
    kind: String,
    company: Option<String>,
    starts_on: NaiveDate,
    ends_on: NaiveDate,
    phone: Option<String>,
    email: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    terms: bool,
}

impl Validate for EventForm {
    fn rules(&self, v: &mut Validator) {
        v.field("title", &self.title)
            .required()
            .between(3, 120)
            .message("Give the event a title of 3 to 120 characters.");
        v.field("kind", &self.kind).required().one_of(&["public", "private", "company"]);
        v.field("company", &self.company).required_if(self.kind == "company");
        v.field("ends_on", &self.ends_on).after_or_equal(self.starts_on);
        v.field("phone", &self.phone).digits_between(9, 13).starts_with(&["08"]);
        v.field("email", &self.email).required_without(&self.phone).email();
        v.field("tags", &self.tags).max(5);
        v.each("tags", &self.tags, |tag| tag.required().alpha_dash().max(20));
        v.distinct("tags", &self.tags);
        v.field("terms", &self.terms).accepted();
    }
}
```

`.message("…")` replaces the message of the rule just before it (here `between`), as written.

### Every rule

`min`, `max`, `between` and `size` measure what the value is: characters for text, the value for
numbers, items for a `Vec`, kilobytes for an `Upload`.

| Group | Rules |
|---|---|
| Presence | `required()`, `required_if(cond)`, `required_unless(cond)`, `required_with(&other)`, `required_without(&other)`, `required_with_all(&[&a, &b])`, `required_without_all(&[&a, &b])`, `prohibited()`, `prohibited_if(cond)`, `prohibited_unless(cond)` (must be empty), `prohibits("other", &other)` (this one or the other, not both), `accepted()` / `accepted_if(cond)` (a ticked checkbox), `declined()` / `declined_if(cond)` (unticked, or `no`/`off`/`0`/`false`) |
| Size | `min(n)`, `max(n)`, `between(min, max)`, `size(n)` |
| Text | `email()`, `url()` (`http://` or `https://`), `matches(r"^[A-Z]{2}\d{4}$")` (a regex; anchor it), `not_matches(pattern)`, `alpha()`, `alpha_num()`, `alpha_dash()`, `ascii()`, `lowercase()`, `uppercase()`, `starts_with(&[…])`, `ends_with(&[…])`, `doesnt_start_with(&[…])`, `doesnt_end_with(&[…])`, `uuid()`, `ulid()`, `ip()`, `mac_address()`, `json()`, `timezone()` (an IANA name such as `Asia/Jakarta`), `hex_color()` (`#4f46e5`) |
| Numbers | `numeric()` and `integer()` (for numbers typed into text fields), `decimal(min, max)` (decimal places: `decimal(2, 2)` for `12.50`), `multiple_of(n)`, `min_digits(n)`, `max_digits(n)` |
| Digits | `digits(n)` (exactly `n` digits, e.g. a PIN), `digits_between(min, max)` (e.g. a phone number) |
| Dates | `date()` (text such as `2026-10-01` or `2026-10-01T10:30`), `before(d)`, `before_or_equal(d)`, `after(d)`, `after_or_equal(d)`, where `d` is a `NaiveDate`, `NaiveDateTime` or `DateTime` |
| Choices | `one_of(&[…])` (Laravel's `in`), `none_of(&[…])` (`not_in`) |
| Other fields | `confirmed(&self.password_confirmation)`, `same("email", &self.email)`, `different("old_email", &self.old_email)`, `gt("min_price", &self.min_price)`, `gte(…)`, `lt(…)`, `lte(…)` |
| Database | `unique(table, column)`, `exists(table, column)`, then `ignore(id)`, `where_eq(column, value)`, `where_null(column)`, `where_not_null(column)` |
| Files | `image()`, `mimes(&["pdf", "jpg"])`, `dimensions(&Dimensions::new().min_width(1200).ratio(3, 1))` (pixels, read from the image's header), plus the size rules in kilobytes |
| Passwords | `password(&policy)` with a `Password` policy (`.uncompromised()` checks known breaches), `current_password()` (the logged-in user's) |
| Your own | `rule(valid, message)`, `apply(&MyRule)` with a `Rule` |

`gt`, `gte`, `lt` and `lte` compare a field with another one, named in the message: numbers
by value, dates by date (typed or as text), other text by length, lists by their items and
files by their size. Two texts that both read as numbers compare as numbers, so a price kept
in a `String` works. When the other field is empty the rule is skipped (add `required` to it).

```rust
# use renox::prelude::*;
use renox::validation::Dimensions;
# struct Promo { min_order: i64, max_discount: i64, starts: String, ends: String, code: Option<String>, gift_card: Option<String>, banner: Option<Upload> }
# impl Validate for Promo {
fn rules(&self, v: &mut Validator) {
    v.field("max_discount", &self.max_discount).lt("min_order", &self.min_order);
    v.field("ends", &self.ends).required().date().gt("starts", &self.starts);
    v.field("code", &self.code).prohibits("gift_card", &self.gift_card);
    let banner = Dimensions::new().min_width(1200).ratio(3, 1);
    v.field("banner", &self.banner).image().max(2048).dimensions(&banner);
}
# }
```

`decimal` counts the places as typed, so check a price as text: an `f64` field has already
lost a trailing zero (`12.50` is `12.5`).

On the validator itself:

- `v.each(name, &list, |item| item.…)`: rules for every item of a list; errors are keyed
  `name.0`, `name.1`, … and labelled "name #1", "name #2".
- `v.distinct(name, &list)`: no repeated items (text compared trimmed and lowercased); each
  repeat gets the error.
- `v.nested(name, &lines)`: each item's own `Validate` rules, for a list of structs (the lines
  of an order sent as JSON, or a form's rows named `lines[0][quantity]`); errors are keyed
  `lines.0.quantity` and labelled by the item's own field ("The quantity field is required.").
- `v.error(field, message)`: an error no rule covers.
- `Validator::new().in_lang(&lang)`: messages and field names from the app's lang file for
  that language (`Valid<T>` does it for the request's language).

Values a rule can check implement `FieldValue`: `String`, `&str`, the integer and float types,
`bool`, `NaiveDate`, `NaiveDateTime`, `DateTime<Utc>`, `Upload`, and `Option<T>` / `Vec<T>` of
those.

### Database rules: `unique` and `exists`

`unique` fails when a row already has the value; `exists` fails when no row has it. They run
after the other rules, together, and only for fields that passed them. An unknown table
or column is an error (a 500), not a rule that always passes.

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProductForm {
    #[serde(default)]
    id: i64, // a hidden input on the edit form; 0 when creating
    sku: String,
    category_id: i64,
    team_id: i64,
}

impl Validate for ProductForm {
    fn rules(&self, v: &mut Validator) {
        v.field("sku", &self.sku)
            .required()
            .unique("products", "sku")
            .ignore(self.id) // the row being edited doesn't count
            .where_eq("team_id", self.team_id) // unique per team
            .where_null("deleted_at"); // soft-deleted rows don't count
        v.field("category_id", &self.category_id)
            .exists("categories", "id")
            .where_eq("team_id", self.team_id) // not another team's category
            .message("Pick a category.");
    }
}
```

`ignore` takes any key type (`i64`, `Ulid`, `Uuid`, `String`). The checks query the table
directly, so a model's default scope (a tenant's rows, see [authorization.md](authorization.md))
doesn't apply: say it with `where_eq`.

### Passwords

```rust
use renox::prelude::*;
use renox::validation::Password;
use serde::Deserialize;

#[derive(Deserialize)]
struct PasswordForm {
    password: String,
    password_confirmation: String,
}

impl Validate for PasswordForm {
    fn rules(&self, v: &mut Validator) {
        let policy = Password::min(12).letters().mixed_case().numbers().symbols();
        v.field("password", &self.password)
            .required()
            .password(&policy)
            .confirmed(&self.password_confirmation);
    }
}
```

Renox's own register, reset and account pages use the app's policy, `Password::min(8)` unless
`Auth::new().password_rules(…)` sets another.

`Password::min(12).uncompromised()` also refuses passwords found in known data breaches, by
asking [Have I Been Pwned](https://haveibeenpwned.com/API/v3#PwnedPasswords): only the first
five characters of the password's SHA-1 leave the server (k-anonymity), through
`state.http`. It runs once the other rules pass. When the service can't be reached the
password is allowed and a warning logged, so sign-ups keep working. In tests, answer it with
`app.fake_http()` (see `parity_more.rs` in Renox's tests for an example).

`current_password()` checks a field against the logged-in user's password (Laravel's
`current_password`), e.g. before changing an email address; it fails when nobody is logged
in:

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Validate)]
struct ChangeEmail {
    #[validate(required, email)]
    email: String,
    #[validate(required, current_password)]
    current_password: String,
}
```

Both run with the database checks, after the other rules: `Valid<T>` passes them the user
and the HTTP client. Outside a request, `Validator::finish_for(&state, Some(&user))` does the
same (`finish(&db)` can't: there's no user, so `current_password` fails, and the breach check
is skipped).

### Your own rules

A one-off check is `rule(valid, message)`. Unlike the other rules it also runs on a missing
value, so it can express "required when …" too:

```rust
# use renox::prelude::*;
# struct Order { quantity: i64 }
# impl Validate for Order {
fn rules(&self, v: &mut Validator) {
    v.field("quantity", &self.quantity)
        .min(1)
        .rule(self.quantity % 6 == 0, "Eggs are sold by the half dozen.");
}
# }
```

A rule used by several forms implements `Rule`; `rnx make:rule TaxId --module invoices` writes
one. Its message may use `:attribute` (the field's label). It is skipped for missing values:

```rust
use renox::prelude::*;
use renox::validation::{Inspected, Rule};

/// A tax ID: 15 or 16 digits.
struct TaxId;

impl Rule for TaxId {
    fn check(&self, value: &Inspected) -> std::result::Result<(), String> {
        let Inspected::Text(text) = value else { return Ok(()) };
        match text.chars().filter(char::is_ascii_digit).count() {
            15 | 16 => Ok(()),
            _ => Err("The :attribute must be a valid tax ID.".into()),
        }
    }
}

# struct Form { tax_id: String }
# impl Validate for Form {
fn rules(&self, v: &mut Validator) {
    v.field("tax_id", &self.tax_id).required().apply(&TaxId);
}
# }
```

A check that needs the database or several fields at once goes in `after` (next sections).

## `#[derive(Validate)]`

For a form that only needs rules, write them as attributes. Each item is a call on the field's
rules, in order: `required` is `.required()`, `max = 100` is `.max(100)`,
`unique("users", "email")` is `.unique("users", "email")`. So every rule above works, and the
arguments are Rust expressions that may use `self`.

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize, Validate)]
struct Signup {
    #[validate(required, max = 100, label = "Full name")]
    name: String,
    #[validate(required, email, unique("users", "email"))]
    email: String,
    #[validate(required, min = 8, confirmed(&self.password_confirmation))]
    password: String,
    password_confirmation: String,
    #[serde(default)]
    #[validate(max = 5, each(required, max = 20), distinct)]
    tags: Vec<String>,
    #[serde(rename = "t-shirt")]
    #[validate(rename = "t-shirt", one_of(&["S", "M", "L"]), message = "Pick a size.")]
    size: String,
    #[validate(image, max = 2048)] // kilobytes; the content is checked, not the name
    photo: Option<Upload>,
}
```

| Attribute item | Becomes |
|---|---|
| `required`, `email`, `image`, … | `.required()`, `.email()`, … |
| `max = 100`, `label = "Full name"`, `message = "…"` | `.max(100)`, `.label("Full name")`, `.message("…")` |
| `between(3, 280)`, `unique("users", "email")`, `confirmed(&self.x)` | the same call with those arguments |
| `label = "…"` | always applied first, wherever it's written |
| `each(rule, …)` | `v.each(name, &self.field, \|item\| item.rule()…)` |
| `distinct` | `v.distinct(name, &self.field)` |
| `rename = "t-shirt"` | the name errors are keyed by, when the form's input name differs from the field's (pair it with `#[serde(rename)]`) |
| `#[validate(hooks)]` on the struct | forwards `prepare`, `authorize` and `after` to `impl ValidateHooks` |

Fields without a `#[validate]` attribute have no rules. Anything the attributes can't say
(a rule that depends on another field's value, `nested`, a loop) needs `impl Validate` by hand.

## Hooks: `prepare`, `authorize`, `after`

Three optional methods around the rules make a form what Laravel calls a form request:

```rust
use renox::prelude::*;
use renox::validation::FormContext;
use serde::Deserialize;

#[derive(Deserialize)]
struct Invite {
    email: String,
    team_id: i64,
}

impl Validate for Invite {
    // First: tidy the input. The form refilled after an error still shows what was typed.
    fn prepare(&mut self) {
        self.email = self.email.trim().to_lowercase();
    }

    // `false` answers 403 before any rule runs.
    async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {
        Ok(form.user.is_some_and(|user| user.has_role("owner")))
    }

    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
    }

    // Only when the rules passed: checks that need the database or several fields.
    async fn after(&self, form: &FormContext<'_>, errors: &mut Errors) -> Result {
        let members: i64 = renox::db::sql("SELECT COUNT(*) FROM team_user WHERE team_id = ?")
            .bind(self.team_id)
            .scalar(&form.state.db)
            .await?;
        if members >= 10 {
            errors.add("email", "This team is full (10 members).");
        }
        Ok(())
    }
}
```

`FormContext` has the app's `state`, the logged-in `user` (`Option<&User>`), and the request's
`method` and `path`. Errors added in `after` are shown like any rule's.

With `#[derive(Validate)]`, add `#[validate(hooks)]` on the struct and implement the hooks you
need on `ValidateHooks`:

```rust
use renox::prelude::*;
use renox::validation::ValidateHooks;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Validate)]
#[validate(hooks)]
struct ProductForm {
    #[validate(required, max = 100)]
    name: String,
    #[validate(min = 0)]
    price: i64,
}

impl ValidateHooks for ProductForm {
    /// "  Iced   Coffee " is checked and saved as "Iced Coffee".
    fn prepare(&mut self) {
        self.name = self.name.split_whitespace().collect::<Vec<_>>().join(" ");
    }
}
```

## Browser values

Browsers send everything as text, and send some inputs oddly. Before the rules run, `Valid`
smooths that over. Only `Valid` does: the prelude's `Form<T>` is axum's and reads the body as it
is, so a ticked checkbox's `on` in a `bool` field answers 422. A form with no rules (a toggle, a
filter, a settings switch) still uses `Valid<T>`, with `#[derive(Deserialize, Validate)]` and no
`#[validate]` attributes.

- **Empty inputs count as missing**, as in Laravel. An `Option<T>` field left empty is `None`.
  A `String` left empty is `""`, and only `required` complains about it. A number, date or
  other non-text field left empty gets "The … field is required.": make it an `Option<T>` if
  it's optional.
- **Checkboxes:** a ticked box sends `on`, an unticked one sends nothing. A `bool` field reads
  `on`, `1`, `yes`, `checked` and `true` as `true`, and `off`, `0`, `no`, `false`, empty or
  missing as `false`.
- **`datetime-local`** sends `2026-10-01T10:30`, without seconds; a `NaiveDateTime` field
  accepts it.
- **Lists:** a multi-select or checkbox group repeats its name. Declare the field as `Vec<T>`
  with `#[serde(default)]`, so that nothing chosen is an empty list. A name ending in `[]`
  (`tags[]`) is a list even when sent once, so it never fills a text field.
- **Rows:** `lines[0][name]=Coffee&lines[0][qty]=2&lines[1][name]=Tea…` reads into
  `lines: Vec<Line>`; `meta[0][key]`/`meta[0][value]` into a `renox::KeyValues` (ordered pairs,
  rows without a key skipped). Empty values stay, so rows keep their numbers and their errors
  (`lines.1.name`); an `Option` reads "" as `None`. `error()` and `old()` in templates take
  either spelling (`lines[1][name]` or `lines.1.name`). Names nested deeper than 32 levels are
  ignored.
- **Values that don't parse** (`price=abc` for an `i64`, `size=huge` for an enum) become an
  error on that field ("The price must be a number.", "The selected size is invalid.") instead
  of a 400. A stand-in value (an enum's first variant, `0`, `false`) is put in its place so the
  rest of the form still parses and every other field's rules run; rule errors on the stand-in
  itself are dropped. JSON bodies get the same treatment.

Type-by-type mappings (HTML input, Rust type, SQLite and PostgreSQL column) are in
[types.md](types.md).

## Messages and translations

Messages come in the request's language (the session's locale, `Accept-Language` with
`App::detect_locale`, else `APP_LOCALE`). Renox ships English messages; for another language
the app translates them in its own lang file (e.g. `resources/lang/es.json`), and anything it
leaves out stays English.

A message template uses `:attribute` (the field's label), `:Attribute` (the same, capitalized)
and the rule's parameters (`:min`, `:max`, `:size`, `:digits`, `:date`, `:values`, `:other`).
The app overrides any built-in message under `renox.validation.<key>` and names fields under
`renox.validation.attributes.<field>`, in its lang files (`resources/lang/<locale>.json`):

```text
{
  "renox": {
    "validation": {
      "required": "El campo :attribute es obligatorio.",
      "min.string": ":Attribute debe tener al menos :min caracteres.",
      "attributes": { "name": "nombre", "message": "mensaje", "photo": "foto" }
    }
  }
}
```

The keys:

| Keys | Rule |
|---|---|
| `required`, `accepted`, `prohibited` | presence rules (`required_if`, `required_with`… use `required`) |
| `min.string`, `min.numeric`, `min.array`, `min.file` (and the same for `max`, `between`, `size`) | size rules, by what is measured |
| `email`, `url`, `regex`, `alpha`, `alpha_num`, `alpha_dash`, `lowercase`, `uppercase`, `starts_with`, `ends_with`, `uuid`, `ip` | text rules (`regex` is `matches`) |
| `numeric`, `integer`, `decimal`, `multiple_of`, `digits`, `digits_between`, `min_digits`, `max_digits` | number and digit rules |
| `gt.numeric`, `gt.string`, `gt.array`, `gt.file`, `gt.date` (and the same for `gte`, `lt`, `lte`) | comparisons with another field (`:other` is its label) |
| `json`, `ulid`, `timezone`, `mac_address`, `ascii`, `hex_color`, `doesnt_start_with`, `doesnt_end_with`, `not_regex` | more text rules (`not_regex` is `not_matches`) |
| `prohibits`, `declined` | presence rules |
| `dimensions` | an image's size in pixels |
| `date`, `before`, `before_or_equal`, `after`, `after_or_equal` | date rules |
| `in`, `not_in` | `one_of`, `none_of` |
| `confirmed`, `same`, `different`, `distinct` | rules across fields |
| `unique`, `exists` | database rules |
| `file`, `image`, `mimes` | uploads |
| `password.letters`, `password.mixed`, `password.numbers`, `password.symbols`, `password.uncompromised` | the `Password` policy |
| `invalid` | a value that doesn't parse, of no type above |
| `auth.failed`, `auth.throttle`, `current_password` | Renox's login and account pages |

Which label a message uses, first match wins: `.label(…)` (or `label = "…"`), the app's
`renox.validation.attributes.<field>`, then the field name with spaces. Labels on Renox's own
forms give way to the app's translations. `.message(…)` and `rule(…)` messages are used as
written; translate them in Rust with the `Lang` extractor if you need to.

## Live validation

Add `data-live-validate` to a form that `Valid<T>` handles and the UI kit's script checks each
field as it's left, then as it's corrected. It posts the form with the header
`X-Renox-Validate: <field>`; `Valid` answers `200 {"field": …, "errors": […]}` with that field's
errors and **the handler doesn't run**, so nothing is saved. The rules are the form's own,
`unique` and `after` included. See [ui.md](ui.md), "Live validation".

## Uploads

A file input is a form field of type `Upload` (send the form as `multipart/form-data`). It goes
through the same rules and the same error handling as the text fields:

```rust
use renox::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct EntryForm {
    name: String,
    photo: Option<Upload>, // <input type="file" name="photo">; no file chosen → None
    #[serde(default)]
    scans: Vec<Upload>, // <input type="file" name="scans" multiple>
}

impl Validate for EntryForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(50);
        v.field("photo", &self.photo).image().max(2048); // KB
        v.field("scans", &self.scans).max(5);
        v.each("scans", &self.scans, |scan| scan.mimes(&["pdf", "jpg", "png"]).max(10_240));
    }
}

async fn store(State(state): State<AppState>, Valid(form): Valid<EntryForm>) -> Result<Redirect> {
    if let Some(photo) = &form.photo {
        let key = photo.store_public(&state.storage, "entries").await?; // public/entries/<random>.jpg
        let _ = (key, &form.name);
    }
    for scan in &form.scans {
        scan.store(&state.storage, "scans").await?; // private
    }
    Ok(Redirect::to("/"))
}
```

`image()` and `mimes()` check the file's content, not its name, for the formats Renox recognizes
(PNG, JPEG, GIF, WebP and PDF; the name decides for other types), so a text file named `.png`
is refused. Stored files get a random name with the extension the content indicates. The
request size is capped by `UPLOAD_MAX_SIZE` (a 413 above it).

## Validating outside a request

`Validator::rules_of` runs a form's rules anywhere, e.g. in a command that imports rows; `finish`
runs the database checks and returns the errors (`prepare`, `authorize` and `after` are not
called):

```rust
# use renox::prelude::*;
# #[derive(serde::Deserialize, Validate)]
# struct ProductForm { #[validate(required)] name: String }
# async fn demo(db: Db, form: ProductForm) -> Result {
let errors = Validator::rules_of(&form).finish(&db).await?;
if let Some(message) = errors.first("name") {
    eprintln!("skipped: {message}");
}
# Ok(()) }
```

## Testing validation

`assert_invalid("field")` expects a 422 with an error on that field, which only htmx and JSON
requests get: send the request with `app.htmx()` or `app.request().json()` (or `post_json`). A
plain post is answered with a 303 back to the form. More in [testing.md](testing.md).

```rust
use renox::prelude::*;
use renox::testing::TestApp;
use serde::Deserialize;

#[derive(Deserialize, Validate)]
struct ProductForm {
    #[validate(required, max = 100)]
    name: String,
    #[validate(min = 0)]
    price: i64,
}

async fn store(Valid(form): Valid<ProductForm>) -> Result<Redirect> {
    let _ = (form.name, form.price);
    Ok(Redirect::to("/products"))
}

struct Products;

impl Module for Products {
    fn name(&self) -> &'static str {
        "products"
    }

    fn routes(&self) -> Routes {
        Routes::new().post("/products", store)
    }
}

#[renox::test]
async fn invalid_products_are_refused() {
    let app = TestApp::new(App::new().module(Products)).await;
    app.htmx()
        .post("/products", &[("name", ""), ("price", "abc")])
        .await
        .assert_invalid("name")
        .assert_invalid("price");
    app.post_json("/products", &json!({ "name": "Coffee", "price": -1 }))
        .await
        .assert_invalid("price");
    app.post("/products", &[("name", ""), ("price", "1")]).await.assert_status(303);
    app.post("/products", &[("name", "Coffee"), ("price", "9000")])
        .await
        .assert_redirect("/products");
}
```

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `$request->validate([...])`, `FormRequest` | `Valid<T>` with `impl Validate` or `#[derive(Validate)]` |
| `'name' => 'required\|max:100'` | `v.field("name", &self.name).required().max(100)` or `#[validate(required, max = 100)]` |
| `prepareForValidation`, `authorize`, `after` / `withValidator` | `prepare`, `authorize`, `after` (`ValidateHooks` with the derive) |
| `in:a,b`, `not_in`, `regex` | `one_of(&[…])`, `none_of(&[…])`, `matches(r"…")` |
| `Rule::unique('users')->ignore($id)->where(...)` | `.unique("users", "email").ignore(id).where_eq(col, value)` |
| `exists:categories,id` | `.exists("categories", "id")` |
| `tags.*` rules, `distinct` | `v.each("tags", …)`, `v.distinct("tags", …)` |
| `Password::min(8)->mixedCase()->numbers()->symbols()` | `Password::min(8).mixed_case().numbers().symbols()` |
| `->uncompromised()`, `current_password` | `.uncompromised()` on the policy, `.current_password()` |
| Named error bags (`validateWithBag`, `@error('email', 'login')`) | `#[validate(bag = "login")]`, `error('email', bag='login')` |
| `Rule` classes, closures, `make:rule` | `impl Rule` + `apply`, `rule(valid, message)`, `rnx make:rule` |
| `nullable` | an `Option<T>` field |
| `messages()`, `attributes()` | `.message(…)`, `.label(…)`; `renox.validation.*` in lang files |
| `ValidationException::withMessages([...])` | `Err(ValidationError::new(errors).into())` |
| `@error('name')`, `old('name')`, `$errors` | `error('name')`, `old('name')`, `errors` (`errors_in('bag')`) |
| Precognition | `data-live-validate` |
| `assertInvalid`, `assertSessionHasErrors` | `assert_invalid` (htmx/JSON); `assert_status(303)` for plain posts |

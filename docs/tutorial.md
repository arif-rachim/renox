# Build your first Renox app

This tutorial builds one small, real app from an empty directory to a server: **Stash**, a
place to keep the links you mean to read. Each person who signs up has their own bookmarks,
saves new ones without a page reload, edits and deletes only their own, and gets a mail every
Monday listing what they saved that week. It has tests, and it ships as one binary.

You'll need Rust 1.94 or later. Knowing Laravel helps, but isn't needed: where Renox does
something differently, the tutorial says so and why. Each step ends with a link to the guide
that goes deeper.

| Step | You'll use |
|---|---|
| [1. Create the app](#1-create-the-app) | `rnx new`, `rnx serve`, the generated files |
| [2. A model and its table](#2-a-model-and-its-table) | `rnx make:model`, a SQL migration, `#[derive(Model)]` |
| [3. A list page](#3-a-list-page) | a module, routes, `require_auth`, `view()`, the UI kit |
| [4. A form that saves without a reload](#4-a-form-that-saves-without-a-reload) | `#[derive(Validate)]`, `Valid<T>`, htmx fragments, toasts |
| [5. Edit and delete, only your own](#5-edit-and-delete-only-your-own) | `Found<T>`, a policy, `Auth` |
| [6. A weekly digest mail](#6-a-weekly-digest-mail) | a job, a mail view, the scheduler |
| [7. Tests and demo data](#7-tests-and-demo-data) | factories, a seeder, `TestApp` |
| [8. Deploy](#8-deploy) | `rnx build`, `rnx make:deploy`, systemd, `.env`, backups |

## 1. Create the app

Install `rnx`, Renox's command-line tool (Laravel's `artisan` and installer in one):

```bash
cargo install --locked --git https://github.com/arif-rachim/renox renox-cli
```

Then make the app and run it:

```bash
rnx new stash
cd stash
rnx serve
```

The first build compiles every dependency and takes a few minutes; later ones take seconds
([development.md](development.md) has tips for faster builds). Open
<http://127.0.0.1:3000>: a home page with a navigation bar, and **Log in** and **Register**
buttons that already work. Register an account; you land back on the home page, logged in,
with your name in a menu on the right.

`rnx serve` keeps watching: change a `.rs` file and it rebuilds and restarts the app; change a
template and the browser reloads itself, with no rebuild. It also runs the database
migrations before each start, so you'll rarely run `rnx migrate` by hand.

### What `rnx new` made

```text
stash/
├── Cargo.toml              one dependency: renox (plus serde)
├── .env, .env.example      settings: APP_KEY (already generated), DATABASE_URL, MAIL_*…
├── build.rs                rebuilds when migrations, views or public files change
├── src/
│   ├── main.rs             runs the app: also its command line (migrate, queue:work…)
│   ├── lib.rs              builds the App: its modules, migrations and seeders
│   └── app/
│       ├── mod.rs          lists the modules
│       └── home/mod.rs     the home page's route and handler
├── resources/
│   ├── views/layouts/app.html   the layout: navigation bar, account menu, toasts
│   ├── views/home/index.html    the home page
│   ├── views/errors/default.html  404, 403 and 500 pages, in the layout
│   └── lang/en.json        the app's texts
├── public/app.css          your own styles (the UI kit brings the rest)
├── migrations/             SQL migrations (empty for now)
├── tests/home.rs           tests that boot the whole app
└── AGENTS.md, CLAUDE.md    a guide for coding assistants working on the app
```

The database is SQLite, in `storage/app.db`, made on first use. Pass `--database postgres` to
`rnx new` to start on PostgreSQL instead ([postgresql.md](postgresql.md)).

Two files are worth reading now. `src/main.rs` is one line:

```rust,no_run
# mod stash { pub fn app() -> renox::App { renox::App::new() } }
fn main() -> renox::Result {
    stash::app().run()
}
```

The app lives in the library, `src/lib.rs`, so that the tests can boot exactly the app
`main` runs. The binary is also the app's command line: `stash migrate`, `stash queue:work`,
`stash route:list`. While developing, `rnx <command>` runs it through `cargo run`, so
`rnx route:list` lists every route with its name and guards.

`src/app/home/mod.rs` is a **module**: a name and its routes. A Renox app is a list of modules;
Laravel keeps routes in `routes/web.php`, Renox keeps each feature's routes next to its
handlers.

```rust
use renox::prelude::*;

pub struct Home;

impl Module for Home {
    fn name(&self) -> &'static str {
        "home"
    }

    fn routes(&self) -> Routes {
        Routes::new().get("/", index).name("home")
    }
}

async fn index() -> View {
    view("home/index.html", context! {})
}
```

A handler is a plain async function. It takes what it needs as arguments (the database, the
logged-in user, a validated form) and returns a response; `view()` renders a MiniJinja
template, Renox's Blade. More in [routing.md](routing.md).

## 2. A model and its table

A bookmark has a title, an address, an optional note, and an owner. Make the module that will
hold everything about bookmarks, then the model in it:

```bash
rnx make:module bookmarks
rnx make:model Bookmark --module bookmarks
rnx make:migration create_bookmarks_table
```

`make:module` wrote `src/app/bookmarks/mod.rs` with a placeholder page and registered the
module in `src/lib.rs`. `make:model` wrote `src/app/bookmarks/model.rs`, and `make:migration`
wrote a pair of files in `migrations/`, named after the current time.

`make:model Bookmark -m` makes the migration too, but it names the table `bookmark`: Renox
never pluralises names (not every language makes plurals with an "s"), so a table is called
what its model is called unless you say otherwise. This tutorial uses the plural, which is
why the migration was made separately.

### The migration

Migrations are plain SQL, not a schema builder: you write the `CREATE TABLE` your database
runs. Fill in `migrations/<timestamp>_create_bookmarks_table.up.sql`:

```sql
CREATE TABLE "bookmarks" (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    note TEXT,
    created_at TEXT,
    updated_at TEXT
);
CREATE INDEX bookmarks_user_id ON bookmarks (user_id);
```

and the `.down.sql` next to it, which `rnx migrate:rollback` runs:

```sql
DROP TABLE "bookmarks";
```

The `users` table comes from Renox's `Auth` module, which has its own migrations; they run
before yours. Deleting a user deletes their bookmarks (`ON DELETE CASCADE`).

### The model

Replace `src/app/bookmarks/model.rs` with:

```rust
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "bookmarks")]
pub struct Bookmark {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub url: String,
    pub note: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}
```

`#[derive(Model)]` gives the struct `create`, `save`, `delete`, `find` and a query builder
(`Bookmark::where_eq("user_id", 7).latest().get(&db)`). The fields are the columns:

- `id: i64` is the key the database counts up; an `id` of `0` means "not saved yet". (For a
  ULID or UUID key, `rnx make:model Bookmark --key ulid`.)
- `Option<String>` is a column that may be `NULL`.
- `created_at` and `updated_at` are filled in when the model is saved.
- `Serialize` lets templates read the fields; `Default` lets you write
  `Bookmark { title, ..Default::default() }`.

Unlike Eloquent, a model is a plain struct: no attributes looked up at runtime, and a typo in a
field is a compile error. Relations are loaded explicitly, a page at a time, which keeps N+1
queries out ([relations.md](relations.md)).

Save the files and `rnx serve` runs the new migration (or run `rnx migrate`;
`rnx migrate:status` lists what has run). `rnx db:shell` opens a SQL prompt on the app's
database if you want to look.

## 3. A list page

Now the page that lists your bookmarks. Replace everything in `src/app/bookmarks/mod.rs`
below the `pub mod model;` line that `make:model` added at the top:

```rust
# fn main() {}
# mod bookmarks {
# pub mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
# }
use renox::prelude::*;

use model::Bookmark;

pub struct Bookmarks;

impl Module for Bookmarks {
    fn name(&self) -> &'static str {
        "bookmarks"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .resource("/bookmarks", "bookmarks", Resource::new().index(index))
            .require_auth()
    }
}

async fn index(State(db): State<Db>, user: AuthUser, Page(page): Page) -> Result<View> {
    let bookmarks = page_of(&db, user.id, page).await?;
    Ok(view("bookmarks/index.html", context! { bookmarks }))
}

/// One page of the user's bookmarks, newest first.
async fn page_of(db: &Db, user_id: i64, page: u32) -> Result<Paginated<Bookmark>> {
    Bookmark::where_eq("user_id", user_id).latest().paginate(db, page, 20).await
}
# }
```

What each piece does:

- `resource("/bookmarks", "bookmarks", …)` adds the routes of a resource, Laravel's
  `Route::resource`, but only the actions you give it. For now that's `index`: `GET /bookmarks`,
  named `bookmarks.index`. Templates link to it with `route('bookmarks.index')`.
- `.require_auth()` guards the routes added before it. A guest is sent to `/login` and, after
  logging in, back to the page they asked for.
- The handler's arguments are extractors. `State(db): State<Db>` is the database pool,
  `AuthUser` the logged-in user (it derefs to `User`, so `user.id` and `user.email` work), and
  `Page(page)` the `?page=` number, `1` when it's missing.
- `paginate` runs two queries (the count and the page) and returns a `Paginated` with
  `items`, `total`, `page` and what page links need.
- `Result<View>` is `renox::Result`: any error converts with `?`, and becomes the error page
  (with the error itself while `APP_DEBUG` is on).

### The view

Replace `resources/views/bookmarks/index.html`:

```html
{% extends "layouts/app.html" %}
{#- Imported at the top, outside every block, so that the `bookmarks` block can
    use them when it's rendered on its own (step 4). -#}
{% from "renox/ui.html" import page_header, list, empty %}

{% block seo %}{{ seo(title="Bookmarks") }}{% endblock %}

{% block content %}
<div class="rx-stack">
  {{ page_header("Bookmarks", subtitle="Links worth keeping.") }}

  {% block bookmarks %}
  <section id="bookmarks" class="rx-stack">
    {% if bookmarks.items %}
      {% call list(label="Your bookmarks") %}
        {% for bookmark in bookmarks.items %}
        <li>
          <span class="rx-list__main">
            <a class="rx-link" href="{{ bookmark.url }}" target="_blank" rel="noopener">{{ bookmark.title }}</a>
            {% if bookmark.note %}<br><span class="rx-subtitle">{{ bookmark.note }}</span>{% endif %}
          </span>
        </li>
        {% endfor %}
      {% endcall %}
      {% from "renox/pagination.html" import pagination %}
      {{ pagination(bookmarks) }}
    {% else %}
      {{ empty("No bookmarks yet", "Links you save show up here, newest first.") }}
    {% endif %}
  </section>
  {% endblock %}
</div>
{% endblock %}
```

The page is built from Renox's UI kit (`renox/ui.html`): `page_header` for the title, `list`
for the rows, `empty` for the "nothing here yet" state. The kit brings the styles, dark mode,
keyboard support and accessible markup, so `public/app.css` stays empty. MiniJinja escapes
every value, so a title with `<script>` in it is shown, not run. The full list of components
is in [ui.md](ui.md).

Last, a link in the navigation bar. In `resources/views/layouts/app.html`, add the section
links inside the `navbar` call, before the spacer:

```html
{% call navbar(app.name, href=route('home')) %}
  {% if auth.check %}
    {% call nav_links() %}
      {{ nav_link(route('bookmarks.index'), "Bookmarks", active=route_is('bookmarks.*')) }}
    {% endcall %}
  {% endif %}
  <span class="rx-spacer"></span>
  …
```

`auth` is there in every template (`auth.check`, `auth.user.name`), and `route_is` marks the
link as the current section on every `bookmarks.*` page.

Reload the browser: **Bookmarks** in the bar leads to an empty list. Log out and open
<http://127.0.0.1:3000/bookmarks>: you're sent to the login page, and back after logging in.

## 4. A form that saves without a reload

The form goes above the list. It's sent with htmx, which comes bundled with Renox (as does
Alpine.js): the server answers with the new list, htmx swaps it in, and no JavaScript is
written by hand.

### The form and its rules

Add this to `src/app/bookmarks/mod.rs`, with `use renox::Toast;` and
`use serde::Deserialize;` next to the other `use` lines:

```rust
# fn main() {}
# mod bookmarks {
# pub mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
# }
# use renox::prelude::*;
# use renox::Toast;
# use serde::Deserialize;
# use model::Bookmark;
# async fn page_of(db: &Db, user_id: i64, page: u32) -> Result<Paginated<Bookmark>> {
#     Bookmark::where_eq("user_id", user_id).latest().paginate(db, page, 20).await
# }
/// What the form sends, and the rules it must pass.
#[derive(Deserialize, Validate)]
struct BookmarkForm {
    #[validate(required, max = 200)]
    title: String,
    #[validate(required, url, max = 2000)]
    url: String,
    #[validate(max = 1000)]
    note: Option<String>,
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    htmx: Htmx,
    Valid(form): Valid<BookmarkForm>,
) -> Result<Response> {
    let bookmark = Bookmark::create(
        &db,
        Bookmark {
            user_id: user.id,
            title: form.title,
            url: form.url,
            note: form.note,
            ..Default::default()
        },
    )
    .await?;
    let toast = Toast::success(format!("Saved “{}”.", bookmark.title));

    if htmx.request {
        // htmx swaps in the new list: render only the `bookmarks` block.
        let bookmarks = page_of(&db, user.id, 1).await?;
        let list = view("bookmarks/index.html", context! { bookmarks }).fragment("bookmarks");
        return Ok((toast, list).into_response());
    }
    // Without JavaScript the form still works: back to the list.
    Ok((toast, Redirect::route("bookmarks.index", &[])?).into_response())
}
# }
```

And give the resource its `store` action:

```rust
# async fn index() -> &'static str { "" }
# async fn store() -> &'static str { "" }
# use renox::prelude::*;
# fn routes() -> Routes {
Routes::new()
    .resource("/bookmarks", "bookmarks", Resource::new().index(index).store(store))
    .require_auth()
# }
```

How it fits together:

- `#[derive(Validate)]` turns each `#[validate(…)]` into rules, much like a Laravel Form
  Request. `url` accepts only `http://` and `https://` addresses, so nobody can save a
  `javascript:` link that would run when clicked. An empty note arrives as `None`, and
  `max` checks it only when there is one.
- `Valid<BookmarkForm>` reads the form, checks the rules, and only then calls the handler: bad
  input never reaches your code. It must be the last argument, since it reads the request
  body.
- When the rules fail, an htmx request gets a `422` with the errors as JSON, and Renox's
  script puts each one under its field. A plain form post is redirected back with the errors
  and the old input in the session instead. Either way, `store` doesn't run.
- `Htmx` tells you whether htmx sent the request. `.fragment("bookmarks")` renders just that
  block of the template, from the same file as the full page, so there are no partials to
  keep in sync.
- `Toast::success` rides along with the response. With htmx it shows at once; after a redirect
  it waits in the session and `{{ toasts() }}` in the layout shows it on the next page.

### The form in the view

In `resources/views/bookmarks/index.html`, add `card`, `input`, `textarea` and `button` to the
import line, and put the form between the `page_header` and `{% block bookmarks %}`:

```html
{% from "renox/ui.html" import page_header, list, empty, card, input, textarea, button %}
```

```html
<form method="post" action="{{ route('bookmarks.store') }}"
      hx-post="{{ route('bookmarks.store') }}" hx-target="#bookmarks" hx-swap="outerHTML"
      x-data @htmx:after-request="if ($event.detail.successful) $el.reset()" novalidate>
  {{ csrf_field() }}
  {% call card(title="Save a link") %}
    {{ input("url", "Address", type="url", required=true, placeholder="https://…") }}
    {{ input("title", "Title", required=true) }}
    {{ textarea("note", "Note", rows=2, hint="Why it's worth reading.") }}
    <div class="rx-card__footer">{{ button("Save") }}</div>
  {% endcall %}
</form>
```

- `hx-post` sends the form with htmx, and `hx-target`/`hx-swap` replace the whole
  `<section id="bookmarks">` with the fragment `store` returns.
- `{{ csrf_field() }}` adds the CSRF token. htmx requests carry it in a header too, so every
  form and htmx call is protected without more code.
- The Alpine.js attribute clears the form after a successful save, and only then: a `422`
  counts as a failure, so what you typed stays for you to fix.
- `novalidate` leaves the checking to the server, so the messages are the same everywhere.
  `input` and `textarea` have a slot for their error under the field.

Try it. Press **Save** with the form empty: "The title field is required." appears under the
title, the address gets its own message, and the page doesn't move. Fill it in properly and
the bookmark appears at the top of the list, with a toast. [validation.md](validation.md) has
every rule; [ui.md](ui.md) covers fragments, toasts and the other htmx headers.

## 5. Edit and delete, only your own

Every bookmark belongs to someone. The list already shows only yours, but an edit page at
`/bookmarks/7/edit` could be opened by anyone who guesses the number. A **policy** says who
may do what to a row:

```bash
rnx make:policy Bookmark --module bookmarks
```

Fill in `src/app/bookmarks/policy.rs`:

```rust
# fn main() {}
# mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
# }
# mod policy {
use renox::prelude::*;

use super::model::Bookmark;

impl Policy for Bookmark {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "update" | "delete" => self.user_id == user.id,
            _ => false,
        }
    }
}
# }
```

A handler then asks with `user.authorize("update", &bookmark)?`, which answers `403` when the
policy says no. As in Laravel, anything not allowed is refused.

### The whole module

Here is `src/app/bookmarks/mod.rs` with every action. The new parts are `edit`, `update` and
`destroy`; the `pub mod model;` and `pub mod policy;` lines stay at the top.

```rust
# fn main() {}
# mod bookmarks {
# pub mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
#     impl Policy for Bookmark {
#         fn allows(&self, user: &User, ability: &str) -> bool {
#             matches!(ability, "update" | "delete") && self.user_id == user.id
#         }
#     }
# }
use renox::Toast;
use renox::prelude::*;
use serde::Deserialize;

use model::Bookmark;

pub struct Bookmarks;

impl Module for Bookmarks {
    fn name(&self) -> &'static str {
        "bookmarks"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .resource(
                "/bookmarks",
                "bookmarks",
                Resource::new()
                    .index(index) //     GET    /bookmarks            bookmarks.index
                    .store(store) //     POST   /bookmarks            bookmarks.store
                    .edit(edit) //       GET    /bookmarks/{id}/edit  bookmarks.edit
                    .update(update) //   PUT    /bookmarks/{id}       bookmarks.update
                    .destroy(destroy), // DELETE /bookmarks/{id}      bookmarks.destroy
            )
            .require_auth()
    }
}

/// What the form sends, and the rules it must pass.
#[derive(Deserialize, Validate)]
struct BookmarkForm {
    #[validate(required, max = 200)]
    title: String,
    #[validate(required, url, max = 2000)]
    url: String,
    #[validate(max = 1000)]
    note: Option<String>,
}

async fn index(State(db): State<Db>, user: AuthUser, Page(page): Page) -> Result<View> {
    let bookmarks = page_of(&db, user.id, page).await?;
    Ok(view("bookmarks/index.html", context! { bookmarks }))
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    htmx: Htmx,
    Valid(form): Valid<BookmarkForm>,
) -> Result<Response> {
    let bookmark = Bookmark::create(
        &db,
        Bookmark {
            user_id: user.id,
            title: form.title,
            url: form.url,
            note: form.note,
            ..Default::default()
        },
    )
    .await?;
    let toast = Toast::success(format!("Saved “{}”.", bookmark.title));

    if htmx.request {
        let bookmarks = page_of(&db, user.id, 1).await?;
        let list = view("bookmarks/index.html", context! { bookmarks }).fragment("bookmarks");
        return Ok((toast, list).into_response());
    }
    Ok((toast, Redirect::route("bookmarks.index", &[])?).into_response())
}

// `Found` loads the bookmark the route's `{id}` names, or answers 404.
async fn edit(user: AuthUser, Found(bookmark): Found<Bookmark>) -> Result<View> {
    user.authorize("update", &bookmark)?;
    Ok(view("bookmarks/edit.html", context! { bookmark }))
}

async fn update(
    State(db): State<Db>,
    user: AuthUser,
    Found(mut bookmark): Found<Bookmark>,
    Valid(form): Valid<BookmarkForm>,
) -> Result<(Toast, Redirect)> {
    user.authorize("update", &bookmark)?;
    bookmark.title = form.title;
    bookmark.url = form.url;
    bookmark.note = form.note;
    bookmark.save(&db).await?;
    Ok((
        Toast::success("Bookmark updated."),
        Redirect::route("bookmarks.index", &[])?,
    ))
}

async fn destroy(
    State(db): State<Db>,
    user: AuthUser,
    Found(mut bookmark): Found<Bookmark>,
) -> Result<(Toast, Redirect)> {
    user.authorize("delete", &bookmark)?;
    bookmark.delete(&db).await?;
    Ok((
        Toast::success(format!("Deleted “{}”.", bookmark.title)),
        Redirect::route("bookmarks.index", &[])?,
    ))
}

/// One page of the user's bookmarks, newest first.
async fn page_of(db: &Db, user_id: i64, page: u32) -> Result<Paginated<Bookmark>> {
    Bookmark::where_eq("user_id", user_id).latest().paginate(db, page, 20).await
}
# }
```

- `Found<Bookmark>` is route model binding: it reads the route's parameter (`{id}`), loads the
  row with that key and answers `404` when there's none. Laravel matches the parameter's name;
  Renox goes by the type you ask for. A route with several parameters names the one to use
  after the table: `/lists/{list}/bookmarks/{bookmarks}`.
- `authorize` comes right after loading, before anything changes. Someone else's bookmark gets
  a `403` page, in your layout (`resources/views/errors/default.html`).
- `save` writes every column and sets `updated_at`; `delete` removes the row. Both are methods
  of the model, as in Eloquent.
- The update and delete answer with a redirect and a toast, which the list page shows.

### The edit page and the row buttons

`resources/views/bookmarks/edit.html` is a plain form (no htmx), so you can see the other way
errors come back:

```html
{% extends "layouts/app.html" %}
{% from "renox/ui.html" import page_header, card, input, textarea, button, link_button, form_errors %}

{% block content %}
<div class="rx-stack">
  {{ page_header("Edit bookmark", back=route('bookmarks.index'), back_label="Bookmarks") }}

  <form method="post" action="{{ route('bookmarks.update', bookmark.id) }}" data-live-validate novalidate>
    {{ csrf_field() }}
    {{ method_field('PUT') }}
    {% call card() %}
      {{ form_errors() }}
      {{ input("url", "Address", type="url", value=bookmark.url, required=true) }}
      {{ input("title", "Title", value=bookmark.title, required=true) }}
      {{ textarea("note", "Note", value=bookmark.note, rows=2) }}
      <div class="rx-card__footer">
        {{ link_button(route('bookmarks.index'), "Cancel", variant="plain") }}
        {{ button("Save changes") }}
      </div>
    {% endcall %}
  </form>
</div>
{% endblock %}
```

- Browsers only send `GET` and `POST`; `method_field('PUT')` adds a hidden `_method` field and
  Renox routes the post to `update`, as Laravel does.
- When the rules fail, the post is redirected back here: `form_errors()` sums the errors up
  at the top, each field shows its own, and the fields keep what was typed (`old` input wins
  over `value`).
- `data-live-validate` checks each field when you leave it, with the same rules on the
  server, and saves nothing until you press **Save changes**.

In `index.html`, give each row its buttons. Add `row_actions`, `link_button` and `confirm` to
the import line, and replace the `<li>`:

```html
<li>
  <span class="rx-list__main">
    <a class="rx-link" href="{{ bookmark.url }}" target="_blank" rel="noopener">{{ bookmark.title }}</a>
    {% if bookmark.note %}<br><span class="rx-subtitle">{{ bookmark.note }}</span>{% endif %}
  </span>
  {% call row_actions() %}
    {{ link_button(route('bookmarks.edit', bookmark.id), "Edit", variant="plain", size="small") }}
    {{ confirm("delete-" ~ bookmark.id, "Delete", route('bookmarks.destroy', bookmark.id),
               "Delete “" ~ bookmark.title ~ "”?", "This can't be undone.", size="small") }}
  {% endcall %}
</li>
```

`confirm` is a **Delete** button that opens a sheet asking first; only the sheet's button
sends the form, as a `DELETE`. On a phone the row's buttons shrink to icons.

### Where to land after logging in

`rnx new` registered Renox's `Auth` module in `src/lib.rs`: the login, registration,
password-reset and account pages, with Argon2id hashing and a login throttle. After logging
in it sends people to the `home` route. Send them to their bookmarks instead:

```rust
# use renox::prelude::*;
# let _ = App::new()
.module(Auth::new().account().redirect_to("/bookmarks"))
# ;
```

`.account()` is the `/account` page (name, email, password, other devices, deleting the
account). [authorization.md](authorization.md) covers gates, roles and API tokens.

Try it with two accounts (a private window for the second): each sees only their own list,
and the second gets a 403 page for the first one's `/bookmarks/1/edit`.

> **The shortcut.** `rnx make:module bookmarks --resource --fields "title:string url:string
> note:text"` writes a module like this one in one go: model, migration, factory, form, the
> seven handlers, views on the UI kit, and tests. Building it by hand once shows you what
> those files do.

## 6. A weekly digest mail

On Monday mornings, each person gets a mail with the links they saved in the past week. Two
parts do it: a **job** that mails one person, and a **scheduled task** that queues a job for
each user. Renox's queue lives in your own database and its workers run inside `serve`, as
does the scheduler: no Redis, no extra process, no cron entry.

```bash
rnx make:job SendDigest --module bookmarks
rnx make:mail digest
```

### The job

`make:job` wrote `src/app/bookmarks/send_digest.rs`; replace it with:

```rust
# fn main() {}
# mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
# }
# mod send_digest {
use renox::chrono::TimeDelta;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use super::model::Bookmark;

/// Mails one user the bookmarks they saved in the last seven days.
#[derive(Serialize, Deserialize)]
pub struct SendDigest {
    pub user_id: i64,
}

impl Job for SendDigest {
    const NAME: &'static str = "send-digest";

    async fn handle(self, ctx: JobContext) -> Result {
        let state = &ctx.state;
        let Some(user) = User::find(&state.db, self.user_id).await? else {
            return Ok(()); // the account was deleted after the job was queued
        };
        let since = renox::db::now() - TimeDelta::days(7);
        let bookmarks = Bookmark::where_eq("user_id", user.id)
            .where_op("created_at", ">=", since)
            .latest()
            .get(&state.db)
            .await?;
        if bookmarks.is_empty() {
            return Ok(()); // nothing new: no mail
        }
        let mail = state.mail_view(
            &user.email,
            "Your week in bookmarks",
            "mail/digest",
            context! { name => user.name, bookmarks },
        )?;
        state.mailer.send(mail).await
    }
}

/// The scheduled task: a job per user, so one failing mail doesn't hold up the rest.
pub async fn send_digests(state: AppState) -> Result {
    for user in User::all(&state.db).await? {
        state.dispatch(SendDigest { user_id: user.id }).await?;
    }
    Ok(())
}
# }
```

- A job is a struct the queue stores as JSON, so it holds an id, not the user: the worker
  loads fresh data when it runs. `NAME` identifies it in the `jobs` table; keep it stable.
- When `handle` returns an error, the job is retried with a growing delay, three attempts by
  default; then it lands in `failed_jobs`, where `rnx queue:failed` lists it and
  `rnx queue:retry` tries it again.
- `renox::db::now()` is "now" as Renox's clock sees it. Tests can move that clock (step 7),
  which `chrono::Utc::now()` wouldn't follow.
- `state.mail_view` renders `mail/digest.html` (and `mail/digest.txt` for the text part)
  into a `Mail`. In a request you'd rather call `state.queue_mail(mail)`, so the visitor
  doesn't wait for the mail server; a job is already in the background, so it sends directly.

### The mail

`make:mail` wrote `resources/views/mail/digest.html` and `digest.txt`. Replace the HTML one:

```html
{% extends "renox/mail/layout.html" %}
{% from "renox/mail/components.html" import button %}
{% block content %}
<p>Hi {{ name }},</p>
<p>Here's what you saved this week:</p>
<ul>
  {% for bookmark in bookmarks %}
  <li><a href="{{ bookmark.url }}">{{ bookmark.title }}</a></li>
  {% endfor %}
</ul>
{{ button(app.url ~ "/bookmarks", "Open your bookmarks") }}
{% endblock %}
```

and the text one:

```text
Hi {{ name }},

Here's what you saved this week:
{% for bookmark in bookmarks %}
- {{ bookmark.title }}: {{ bookmark.url }}
{% endfor %}
{{ app.url }}/bookmarks
```

The layout and the `button` are styled inline, the way mail clients need. [mail.md](mail.md)
has the other components, attachments and notifications.

### The schedule

`make:job` also added a `register` method to the module, which registers the job. Add the
task to it, in `src/app/bookmarks/mod.rs`:

```rust
# fn main() {}
# mod bookmarks {
# pub mod send_digest {
#     use renox::prelude::*;
#     #[derive(serde::Serialize, serde::Deserialize)]
#     pub struct SendDigest { pub user_id: i64 }
#     impl Job for SendDigest {
#         const NAME: &'static str = "send-digest";
#         async fn handle(self, _ctx: JobContext) -> Result { Ok(()) }
#     }
#     pub async fn send_digests(_state: AppState) -> Result { Ok(()) }
# }
# use renox::prelude::*;
# async fn index() -> &'static str { "" }
# pub struct Bookmarks;
use renox::chrono::Weekday;

impl Module for Bookmarks {
    fn name(&self) -> &'static str {
        "bookmarks"
    }

    fn register(&self, app: &mut Registry) {
        app.job::<send_digest::SendDigest>();
        // Mondays at 08:00, in APP_TIMEZONE.
        app.schedule()
            .weekly_on(Weekday::Mon, "08:00", "weekly-digest", send_digest::send_digests);
    }

    fn routes(&self) -> Routes {
        // … as before
#         Routes::new().get("/bookmarks", index)
    }
}
# }
```

Times are in `APP_TIMEZONE` from `.env` (`UTC` unless you set one, such as `Europe/Amsterdam`,
daylight saving included). A job must be registered for a worker to know how to run it; one
that isn't goes straight to `failed_jobs`, where `rnx queue:retry` can run it once it is.

You don't have to wait for Monday. `rnx schedule:list` shows the task and its next run, and

```bash
rnx schedule:run weekly-digest
```

runs it now. The jobs it queues are picked up by the workers of the running `rnx serve`. With
`MAIL_MAILER=log` (the default in `.env`) mail goes to the server's log instead of out, and
while `APP_DEBUG` is on, <http://127.0.0.1:3000/_renox/mail> shows each one as the
recipient would see it. More on the scheduler in [scheduling.md](scheduling.md), on jobs in
[queue.md](queue.md).

## 7. Tests and demo data

### A factory and a seeder

A factory makes models with fake data, for tests and for a database to click around in:

```bash
rnx make:factory Bookmark --module bookmarks
```

Replace `src/app/bookmarks/bookmark_factory.rs`:

```rust
# fn main() {}
# mod model {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
# }
# mod bookmark_factory {
use renox::fake::Fake;
use renox::fake::faker::lorem::en::{Sentence, Word};
use renox::prelude::*;

use super::model::Bookmark;

impl Factory for Bookmark {
    fn definition() -> Self {
        let path: String = Word().fake();
        Bookmark {
            title: Sentence(2..5).fake(),
            url: format!("https://example.com/{path}"),
            ..Default::default()
        }
    }
}
# }
```

`Bookmark::factory().count(20).create(&db)` saves twenty; `.state(…)` changes each one first,
and `.make_one()` gives one without saving it.

Now `src/lib.rs`, with a seeder and the model exported for the tests (`tests/` can only see
what the library makes public):

```rust
# fn main() {}
# mod app {
#     pub mod home {
#         pub struct Home;
#         impl renox::Module for Home { fn name(&self) -> &'static str { "home" } }
#     }
#     pub mod bookmarks {
#         pub struct Bookmarks;
#         impl renox::Module for Bookmarks { fn name(&self) -> &'static str { "bookmarks" } }
#         pub mod model {
#             use renox::prelude::*;
#             #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#             #[model(table = "bookmarks")]
#             pub struct Bookmark {
#                 pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#                 pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#             }
#             impl Factory for Bookmark {
#                 fn definition() -> Self { Bookmark { title: "Rust".into(), url: "https://www.rust-lang.org".into(), ..Default::default() } }
#             }
#         }
#     }
# }
use renox::prelude::*;

pub use app::bookmarks::model::Bookmark;

/// The application: its modules, migrations and seeders. `main.rs` runs it,
/// and tests boot it with `renox::testing::TestApp`.
pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new().account().redirect_to("/bookmarks"))
        .module(app::home::Home)
        .module(app::bookmarks::Bookmarks)
        // `rnx db:seed`: a demo account with some bookmarks. Running it twice changes nothing.
        .seeder(|state| async move {
            if User::find_by_email(&state.db, "demo@example.com").await?.is_some() {
                return Ok(());
            }
            let demo = User::register(&state.db, "Demo", "demo@example.com", "password123").await?;
            let owner = demo.id;
            Bookmark::factory()
                .count(25)
                .state(move |b: &mut Bookmark| b.user_id = owner)
                .create(&state.db)
                .await?;
            Ok(())
        })
}
```

The `mod app;` line stays at the top of the file. Run `rnx db:seed`, log in as
`demo@example.com` with `password123`, and the list has two pages. (`rnx migrate:fresh --seed`
starts over: it drops every table, migrates and seeds.)

### The tests

`TestApp` boots the whole app in memory: a fresh, migrated database for each test (in-memory
SQLite), the memory mailer, and no workers or scheduler unless the test runs them. Requests
go through the same router, sessions and CSRF checks as in production, so a test drives the app
the way a browser does. Make `tests/bookmarks.rs`:

```rust
# fn main() {}
# mod stash {
#     use renox::prelude::*;
#     #[derive(Model, serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#     #[model(table = "bookmarks")]
#     pub struct Bookmark {
#         pub id: i64, pub user_id: i64, pub title: String, pub url: String,
#         pub note: Option<String>, pub created_at: Option<DateTime>, pub updated_at: Option<DateTime>,
#     }
#     impl Factory for Bookmark {
#         fn definition() -> Self { Bookmark { title: "Rust".into(), url: "https://www.rust-lang.org".into(), ..Default::default() } }
#     }
#     pub fn app() -> App { App::new().module(Auth::new()) }
# }
use renox::prelude::*;
use renox::testing::TestApp;
use stash::Bookmark;
use std::time::Duration;

async fn user(app: &TestApp, email: &str) -> User {
    User::register(app.db(), "Test", email, "password123").await.unwrap()
}

#[renox::test]
async fn guests_are_sent_to_the_login_page() {
    let app = TestApp::new(stash::app()).await;
    app.get("/bookmarks").await.assert_redirect("/login");
}

#[renox::test]
async fn users_save_bookmarks() {
    let app = TestApp::new(stash::app()).await;
    let ana = user(&app, "ana@example.com").await;
    app.acting_as(&ana);

    // As htmx sends it: the answer is the list, with the new bookmark.
    app.htmx()
        .post("/bookmarks", &[("title", "The Rust Book"), ("url", "https://doc.rust-lang.org/book/")])
        .await
        .assert_ok()
        .assert_see("The Rust Book");
    app.assert_database_has("bookmarks", &[("title", &"The Rust Book"), ("user_id", &ana.id)])
        .await;
}

#[renox::test]
async fn invalid_bookmarks_are_refused() {
    let app = TestApp::new(stash::app()).await;
    app.acting_as(&user(&app, "ana@example.com").await);

    app.htmx()
        .post("/bookmarks", &[("title", ""), ("url", "javascript:alert(1)")])
        .await
        .assert_invalid("title")
        .assert_invalid("url");
    app.assert_database_count("bookmarks", 0).await;
}

#[renox::test]
async fn only_the_owner_edits_and_deletes() {
    let app = TestApp::new(stash::app()).await;
    let ana = user(&app, "ana@example.com").await;
    let owner = ana.id;
    let bookmark = Bookmark::factory()
        .state(move |b: &mut Bookmark| b.user_id = owner)
        .create_one(app.db())
        .await
        .unwrap();
    let edit = format!("/bookmarks/{}/edit", bookmark.id);
    let member = format!("/bookmarks/{}", bookmark.id);

    app.acting_as(&user(&app, "ben@example.com").await);
    app.get(&edit).await.assert_forbidden();
    app.delete(&member).await.assert_forbidden();

    app.acting_as(&ana);
    app.put(&member, &[("title", "Renamed"), ("url", "https://example.com")])
        .await
        .assert_redirect("/bookmarks");
    app.get("/bookmarks").await.assert_see("Bookmark updated."); // the toast
    app.delete(&member).await.assert_redirect("/bookmarks");
    app.assert_database_count("bookmarks", 0).await;
}

#[renox::test]
async fn the_weekly_digest_mails_what_is_new() {
    let app = TestApp::new(stash::app()).await;
    let ana = user(&app, "ana@example.com").await;
    let owner = ana.id;
    Bookmark::factory()
        .count(2)
        .state(move |b: &mut Bookmark| b.user_id = owner)
        .create(app.db())
        .await
        .unwrap();
    user(&app, "ben@example.com").await; // saved nothing: gets no mail

    app.kernel().run_scheduled("weekly-digest").await.unwrap();
    assert_eq!(app.run_jobs().await, 2); // a job per user
    app.assert_mail_sent("ana@example.com", "Your week in bookmarks");
    assert_eq!(app.sent_mail().len(), 1);

    // Eight days later nothing is new, so nobody gets mail.
    app.travel(Duration::from_secs(8 * 24 * 60 * 60));
    app.at_travelled_time(app.kernel().run_scheduled("weekly-digest"))
        .await
        .unwrap();
    app.run_jobs().await;
    assert_eq!(app.sent_mail().len(), 1);
}
```

Run them with `cargo test`. One test from `rnx new` fails now: `guests_can_register` in
`tests/home.rs` expects new users to land on `/`. Since step 5 they land on their bookmarks,
so change its `.assert_redirect("/")` to `.assert_redirect("/bookmarks")`.

A few things to notice:

- `acting_as` logs a user in for the requests that follow; `htmx()` sends the next request as
  htmx would. `assert_invalid` expects the `422` an htmx form gets, with an error on that field.
- The CSRF token is sent for you, and the session cookie is kept between requests, so a toast
  set by one request shows on the next page.
- The test doesn't start the scheduler: `run_scheduled` runs a task now, `run_jobs` runs what
  it queued, and the memory mailer keeps what was sent.
- `travel` moves Renox's clock for what the app does next, so "a week later" takes no time.

[testing.md](testing.md) lists every assertion, and the fakes for events, notifications and
HTTP calls.

## 8. Deploy

A Renox app deploys as one binary: the views, translations, files in `public/` and migrations
are compiled into it (that's what `.embed(renox::embedded!())` and `renox::migrations!()` in
`src/lib.rs` do). Next to it you need a `.env` and a `storage/` directory for the SQLite
database. The queue workers and the scheduler run in the same process.

### Build

```bash
rnx build          # a release build, copied to dist/stash
rnx make:deploy    # Dockerfile, deploy/stash.service, deploy/stash.socket, deploy/litestream.yml, deploy/README.md
```

Build on the same OS and CPU as the server, or build the Docker image. `deploy/README.md` is
the full recipe for your app; the short version follows.

### On a Linux server with systemd

```bash
sudo useradd --system --home /opt/stash stash
sudo mkdir -p /opt/stash/storage
sudo cp dist/stash /opt/stash/
sudo cp .env.example /opt/stash/.env          # then edit it, below
sudo chown -R stash /opt/stash
sudo cp deploy/stash.service /etc/systemd/system/
sudo systemctl enable --now stash
```

The service runs `stash migrate` before each start, so a deploy is: copy the new binary,
then `sudo systemctl restart stash`.

### The production `.env`

```bash
APP_NAME="Stash"
APP_ENV=production
APP_DEBUG=false
APP_URL=https://stash.example.com
# A new key, from `rnx key:generate --show`. Keep it safe: it encrypts sessions.
APP_KEY=base64:...
APP_HOST=127.0.0.1
APP_PORT=3000
# Caddy or nginx on the same machine sends the visitor's address.
TRUSTED_PROXIES=127.0.0.1
# Other Host headers get a 400.
TRUSTED_HOSTS=stash.example.com
APP_TIMEZONE=Europe/Amsterdam
DATABASE_URL=sqlite://storage/app.db

MAIL_MAILER=smtp
MAIL_HOST=smtp.your-provider.com
MAIL_PORT=587
MAIL_ENCRYPTION=starttls
MAIL_USERNAME=...
MAIL_PASSWORD=...
MAIL_FROM_ADDRESS=hello@stash.example.com
```

- `APP_ENV=production` refuses to start without an `APP_KEY`, sends HSTS with an `https://`
  `APP_URL`, and lets search engines in (anywhere else, pages say `noindex`).
- `APP_DEBUG=false` hides error details from visitors.
- `TRUSTED_PROXIES` matters for the login throttle and rate limits: without it every visitor
  looks like the proxy.
- `APP_URL` is used in links in mails; an `https://` one also marks cookies `Secure`.

Put a reverse proxy in front for TLS. With Caddy, the whole configuration is:

```text
stash.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

### Restarts without refused connections

A restart closes the port for a second or two. With systemd's socket activation, systemd holds
the port and queues visitors while the app restarts:

```bash
sudo cp deploy/stash.socket /etc/systemd/system/ && sudo systemctl daemon-reload
sudo systemctl stop stash                 # it holds the port; the socket takes it over
sudo systemctl enable --now stash.socket
sudo systemctl start stash
```

From then on, `sudo systemctl restart stash` makes connections wait instead of failing. For
that moment the old code runs against the new migrations, so add columns as nullable or with a
default, and drop a column only once no deployed code reads it.

### Backups

The database is one file, `storage/app.db`. Litestream copies every change to S3-compatible
storage (S3, R2, MinIO) as it happens: install it, fill in the bucket in
`deploy/litestream.yml`, and copy it to `/etc/litestream.yml`; `deploy/README.md` has the
steps, including restoring onto a new server. For a one-off copy while the app runs, use
`sqlite3 storage/app.db ".backup backup.db"`, never `cp`. Keep `APP_KEY` with your backups:
without it, sessions and signed links stop working.

`GET /health` answers `200` while the database is reachable, for your uptime monitor.
[operations.md](operations.md) covers timeouts, failed jobs, logs, error reports and running
several servers.

## Where to go next

You've used most of what a typical app needs. From here:

- [CHEATSHEET.md](../CHEATSHEET.md): every common task in a few lines.
- [examples/crud](../examples/crud): this tutorial's patterns with pagination, soft deletes and
  model hooks; [examples/shop](../examples/shop): a whole shop with checkout, an admin and
  translations.
- The guides: [routing](routing.md), [validation](validation.md), [views and the UI
  kit](ui.md), [mail and notifications](mail.md), [the queue](queue.md),
  [scheduling](scheduling.md), [relations](relations.md), [testing](testing.md) and
  [production](operations.md).

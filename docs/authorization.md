# Authorization: gates, policies, roles, tokens and tenants

Who may do what, and to which rows. Renox has one tool per question; this guide shows when to
reach for each and how they fit together. For the short version of every API, see the
[cheat-sheet](../CHEATSHEET.md) ("Auth, policies, gates" and "Tenants, roles and permissions").
Complete apps: [examples/shop](../examples/shop) (an admin role, an audit trail),
[examples/api](../examples/api) (token abilities), [examples/crud](../examples/crud) (a policy)
and [examples/teams](../examples/teams) (tenants).

| Question | Tool | Where it's checked |
|---|---|---|
| Is someone logged in? | `AuthUser`, `.require_auth()` | extractor, route |
| May this user do X at all? | a gate (`App::gate`, `gate_async`) | `.require_gate("x")`, `user.gate("x")?`, `can('x')` |
| May this user do X to *this* row? | a policy (`impl Policy`) | `user.authorize("update", &row)?`, `can('update', row)` |
| Which job does the user have? | roles and permissions (the `Permissions` module) | `.require_role`, `.require_permission`, `has_role` |
| Who may do everything? | `App::gate_before` | before every gate, permission and policy |
| What may this API token do? | token abilities | `.require_ability("orders:write")`, `token_can` |
| Which rows exist for this user at all? | a default scope (tenants) | every query of the model |
| Is it really them, right now? | `.require_password_confirmed()` | route |
| Who did it? | the `Audit` module | `audit::record` |

## Gates: "may this user do X?"

A gate is a named yes/no about the user alone. Use one when the answer doesn't depend on a
particular row. A gate is sync (it sees the `User`), or async with the app's state when it
needs the database:

```rust
use renox::prelude::*;

fn app() -> App {
    App::new()
        .module(Auth::new())
        .gate("reports", |user| user.email.ends_with("@example.com"))
        .gate_async("billing", |user, state| async move {
            let owners: i64 = renox::db::sql("SELECT COUNT(*) FROM billing_owners WHERE user_id = ?")
                .bind(user.id)
                .scalar(&state.db)
                .await?;
            Ok(owners > 0)
        })
}

fn routes() -> Routes {
    // Guests are sent to log in, other users get 403. Like every guard, it covers the routes
    // added before it.
    Routes::new()
        .get("/reports", || async { "reports" })
        .require_gate("reports")
}

async fn invoices(user: AuthUser) -> Result<String> {
    user.gate_async("billing").await?; // 403 unless allowed; `allows_async` for a bool
    Ok(format!("reports too: {}", user.allows("reports")))
}
```

In templates, `{% if can('reports') %}` asks a gate (or a permission of that name).

## Policies: "may this user do X to this row?"

A policy lives on the model and answers per row, usually by ownership:

```rust
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "invoices")]
struct Invoice { id: i64, user_id: i64, paid: bool }

impl Policy for Invoice {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "view" => self.user_id == user.id,
            "update" => self.user_id == user.id && !self.paid,
            _ => false,
        }
    }
}

async fn edit(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let invoice = Invoice::find_or_404(&db, id).await?;
    user.authorize("update", &invoice)?; // 403 unless allowed
    Ok(view("invoices/edit.html", context! { invoice }))
}

async fn index(State(db): State<Db>, user: AuthUser) -> Result<View> {
    // For `{% if can('update', invoice) %}` in the view, wrap each row with its answers.
    let invoices: Vec<_> = Invoice::where_eq("user_id", user.id)
        .get(&db)
        .await?
        .into_iter()
        .map(|invoice| Can::new(invoice, Some(&user), &["update"]))
        .collect();
    Ok(view("invoices/index.html", context! { invoices }))
}
```

A policy gets the plain `User`, **without its roles**: "admins see every invoice" can't be
written inside `allows`. Ask the role first, then the policy
([examples/shop](../examples/shop/src/app/orders/mod.rs) does this):

```rust
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default)]
#[model(table = "invoices")]
struct Invoice { id: i64, user_id: i64 }

impl Policy for Invoice {
    fn allows(&self, user: &User, ability: &str) -> bool {
        ability == "view" && self.user_id == user.id
    }
}

async fn show(State(db): State<Db>, user: AuthUser, Path(id): Path<i64>) -> Result<View> {
    let invoice = Invoice::find_or_404(&db, id).await?;
    if !user.has_role("accountant") {
        user.authorize("view", &invoice)?;
    }
    Ok(view("invoices/show.html", context! { invoice }))
}
```

## Roles and permissions

The opt-in `Permissions` module stores roles, the permissions each role grants, and who has
which role (tables `roles`, `permissions`, `permission_role`, `role_user`). It loads the
user's roles and permissions once per request, so the checks below cost no query.

```rust
use renox::prelude::*;
use renox::auth::{Permissions, permissions};

fn app() -> App {
    App::new().module(Auth::new()).module(Permissions)
}

fn routes() -> Routes {
    let drafts = Routes::new().get("/drafts", || async { "drafts" }).require_role("editor");
    let publish = Routes::new()
        .post("/posts/{id}/publish", || async { "published" })
        .require_permission("posts.publish");
    drafts.merge(publish)
}

// In a seeder, a migration's data step or an admin page:
async fn setup(db: &Db, user: &User) -> Result {
    // Creates the role if it's new and sets exactly these permissions.
    permissions::define_role(db, "editor", &["posts.create", "posts.publish"]).await?;
    user.assign_role(db, "editor").await?; // also remove_role, sync_roles
    Ok(())
}

async fn check(user: AuthUser) -> String {
    // `allows` asks gate_before, then a gate of that name, then the permissions.
    format!("{} {}", user.has_role("editor"), user.allows("posts.publish"))
}
```

- Prefer permissions in checks (`require_permission("posts.publish")`) and roles as bundles of
  them; then a new role needs no code change. A role with no permissions (`&[]`) is fine when the
  app only checks the role itself, as examples/shop does with `admin`.
- In templates: `{% if 'editor' in auth.roles %}` and `{% if can('posts.publish') %}`.
- There is no loader for "the users with a role" yet; query `role_user` with a sub-query:

```rust
use renox::prelude::*;

async fn editors(db: &Db) -> Result<Vec<User>> {
    User::query()
        .where_raw(
            "id IN (SELECT ru.user_id FROM role_user ru JOIN roles r ON r.id = ru.role_id WHERE r.name = ?)",
            ["editor"],
        )
        .get(db)
        .await
}
```

## Super-admins: `gate_before`

`App::gate_before` is asked before every gate, permission and policy check. It returns
`Some(true)` (allow), `Some(false)` (deny) or `None` (go on to the check itself). It sees the
`User` without roles, and it doesn't answer `require_role` / `has_role`, which mean exactly
that role. Base it on the user row, e.g. a column:

```rust
use renox::prelude::*;

fn app() -> App {
    App::new()
        .module(Auth::new())
        .gate_before(|user, _ability| (user.get::<bool>("super_admin") == Some(true)).then_some(true))
}
```

## API tokens and abilities

Tokens (`Authorization: Bearer …`) can be limited to abilities and given an expiry.
`.require_ability(x)` checks requests made with a token; session users and tokens made without
abilities pass, so put `.require_auth()` (or another guard) next to it:

```rust
use renox::prelude::*;

async fn issue(State(db): State<Db>, user: AuthUser) -> Result<String> {
    let in_30_days = renox::db::now() + renox::chrono::Duration::days(30);
    let token = user
        .create_token_with(&db, "reporting", &["orders:read"], Some(in_30_days))
        .await?;
    Ok(token.plain) // shown once; only its hash is stored
}

fn api() -> Routes {
    let read = Routes::new().get("/api/orders", || async { "[]" }).require_ability("orders:read");
    let write = Routes::new().post("/api/orders", || async { "{}" }).require_ability("orders:write");
    read.merge(write).require_auth() // 401 without a valid token, then 403 without the ability
}
```

`user.token_can("orders:read")` answers in a handler; `rnx tokens:prune` deletes expired
tokens (schedule it daily, as examples/api does).

## Tenants: rows that belong to a team

In a multi-tenant app most rows belong to a team (or a shop, a school…), and one missing
`where team_id = ?` leaks another customer's data. A **default scope** makes the filter part of
the model, so handlers can't forget it:

```rust
use renox::prelude::*;
use renox::axum::{extract::Request, middleware::{Next, from_fn}};

#[derive(Clone)]
struct CurrentTeam(i64);

#[derive(Model, serde::Serialize, Default)]
#[model(table = "projects", default_scope = "team_only")]
struct Project { id: i64, team_id: i64, name: String }

fn team_only(query: renox::db::Query<Project>) -> renox::db::Query<Project> {
    match renox::context::get::<CurrentTeam>() {
        Some(team) => query.where_eq("team_id", team.0),
        None => query.none(), // no team, no rows: fail closed
    }
}

// Runs for every request: puts the user's current team in this request's context.
async fn pick_team(user: Option<AuthUser>, req: Request, next: Next) -> Response {
    if let Some(team) = user.as_ref().and_then(|u| u.get::<i64>("current_team_id")) {
        renox::context::set(CurrentTeam(team));
    }
    next.run(req).await
}

fn app() -> App {
    App::new().module(Auth::new()).layer(from_fn(pick_team))
}

async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    // Another team's project id is a 404: the scope is part of the query.
    let project = Project::find_or_404(&db, id).await?;
    Ok(view("projects/show.html", context! { project }))
}

async fn count_all(db: &Db) -> Result<u64> {
    Project::unscoped().count(db).await // admin code and commands that must see every team
}
```

- `query()`, `find`, `all`, `where_eq` and the relation loaders apply the scope; `unscoped()`
  skips it. Saving and deleting a loaded model work by its id.
- Validate the team the user asks for before putting it in the context (is the user a member?).
  [examples/teams](../examples/teams) keeps the current team in the session and checks the
  membership in the middleware.
- Every request, job, scheduled task and app command starts with an empty context. A job for a
  team carries the team id in its payload and calls `renox::context::set(CurrentTeam(id))`
  first; a `tokio::spawn`ed task starts without one (wrap it in `renox::context::scope`).
- Uniqueness is per team too: `.unique("projects", "name").ignore(id).where_eq("team_id", team)`.
- Bulk `Query::update` / `delete` start from `query()`, so they are scoped as well; raw
  `renox::db::sql(…)` is not.

## Sensitive actions and the audit trail

- `.require_password_confirmed()` asks for the password again (at most every three hours)
  before routes such as billing, API keys or deleting a team.
- The opt-in `Audit` module records logins, failed logins and account changes by itself;
  record the app's own actions with `audit::record(&db, Entry::new("order.refunded")
  .user(id).subject("orders", order_id).ip(ip))`, and read them back with `for_subject`,
  `for_user` or `latest`. `rnx audit:prune --days 365` trims the table.

## Testing authorization

`TestApp::acting_as(&user)` logs a user in; `assert_forbidden()`, `assert_not_found()` and
`assert_redirect("/login")` check the refusals. Test the negative cases: another user's row,
another team's row, a token without the ability, a guest.

## Coming from Laravel

| Laravel | Renox |
|---|---|
| `Gate::define`, `@can`, `can:` middleware | `App::gate` / `gate_async`, `can(…)`, `.require_gate(…)` |
| Policies, `$this->authorize()` | `impl Policy`, `user.authorize(…)?`, `Can::new` for views |
| `Gate::before` | `App::gate_before` |
| spatie/laravel-permission | the `Permissions` module |
| Sanctum abilities, `tokenCan` | `create_token_with`, `.require_ability(…)`, `token_can` |
| Global scopes (`addGlobalScope`), tenancy packages | `#[model(default_scope = "…")]` + `renox::context` |
| `password.confirm` middleware | `.require_password_confirmed()` |
| spatie/laravel-activitylog | the `Audit` module |

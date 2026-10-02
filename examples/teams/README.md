# examples/teams

The official pattern for a multi-tenant SaaS. Users belong to teams (as an owner or a member),
pick a current team, and only ever see that team's projects. Handlers never filter by team: the
current team lives in `renox::context`, and the `Project` model's default scope reads it. Also
shows a super-admin through `App::gate_before`, a team secret stored encrypted and revealed only
after the password is confirmed, and the account pages from `Auth::new().account()`.

```bash
cd examples/teams
rnx key:generate                 # keeps logins (and the encrypted secrets) across restarts
cargo run -- migrate
cargo run -- db:seed             # alice@, bob@ and carol@example.com / password123
SUPER_ADMINS=alice@example.com cargo run   # http://127.0.0.1:3000
cargo run -- projects:count      # every team's projects, across tenants
```

Alice owns Acme and is a member of Globex, so she can switch between them at `/teams`. Bob owns
Globex, Carol is a member of Acme. With `SUPER_ADMINS` set, Alice also sees `/admin`.

## What's where

| Feature | Where |
|---|---|
| Wiring: `Auth::new().account()`, the modules, the tenancy layer, the shared `team`, `gate_before`, the `projects:count` command, the seeder | [src/lib.rs](src/lib.rs) |
| The current team: session → membership check → `renox::context`, an extractor, a policy | [src/app/tenancy.rs](src/app/tenancy.rs) |
| `Team`, the `team_user` pivot with a `role` (`MEMBERS`, `USER_TEAMS`), the secret helpers | [src/app/teams/model.rs](src/app/teams/model.rs) |
| Create (the "New team" wizard: a name, then a repeater of members' emails read into `Vec<Invite>`, each row checked with `v.nested` and `after`), switch, members (a form request: `MemberForm`'s `prepare`, `authorize`, `after`), the encrypted secret behind `require_password_confirmed`, each team's public page on its own host (`Routes::domain`, `DomainParams`, a domain fallback) | [src/app/teams/mod.rs](src/app/teams/mod.rs) |
| `Project` with `default_scope = "team_only"` and a `saving` hook that fills `team_id` | [src/app/projects/model.rs](src/app/projects/model.rs) |
| Project CRUD with no `team_id` in sight; name unique per team | [src/app/projects/mod.rs](src/app/projects/mod.rs) |
| The super-admin check and the cross-team report with `Project::unscoped()` | [src/app/admin.rs](src/app/admin.rs) |
| Pages on the UI kit: a navigation bar with the current team and an account menu, kit forms with live validation (the project name suggests common names with `datalist`), tables, confirmation sheets (delete a project, replace the secret), toasts, an error page in the layout | [resources/views](resources/views) |
| Tables: `teams`, `team_user`, `projects` (unique `(team_id, name)`); `slug` added in a later migration | [migrations](migrations) |

## Things worth copying

- **A form in steps with rows of fields.** [teams/index.html](resources/views/teams/index.html)
  wraps "New team" in the kit's `wizard`: Next asks the server about the step's fields
  (`data-live-validate`) before moving on. The members are a `repeater`, so its inputs are
  named `invites[0][email]`, `invites[1][email]`… and `Valid` reads them into
  `Vec<Invite>`; `v.nested("invites", …)` runs each row's rules and `after` checks every email
  against the users, keyed `invites.1.email` so the error shows in that row.

- **A form request.** `MemberForm` (`impl Validate`) does what Laravel's form requests do:
  `prepare` lowercases the email, `authorize` lets only the team's owners through (403
  before any rule runs), the rules check the email, and `after` looks the person up in the
  database, adding a field error when nobody has that email. The handler is left with adding
  the member. The form has `data-live-validate`, so that lookup already answers when the field
  is left (a test sends `X-Renox-Validate: email`).

- **The tenant is set once per request.** An `App::layer` middleware reads `current_team_id`
  from the session, checks the membership in one query (falling back to the user's first team,
  so a removed member can't keep a stale team) and calls `renox::context::set(team)`.
- **A default scope that fails closed.** `team_only` adds `team_id = ?` for the current team and
  `query.none()` without one, so a request, job or command that forgot the team sees nothing
  rather than everything. `find_or_404` on another team's project is a plain 404.
- **New rows join the current team by themselves.** `#[model(hooks)]` with a `saving` hook sets
  `team_id` on create, and refuses to save without a team.
- **Unique per team.** `.unique("projects", "name").ignore(self.id).where_eq("team_id", team)`,
  backed by a unique index on `(team_id, name)`.
- **Per-team roles live in the pivot.** The `Permissions` module's roles are global; a member's
  role in one team is a `role` column on `team_user`, written with `attach_with` and read with
  `load_with_pivot` (both directions, thanks to `Pivot::inverse`).
- **`unscoped()` only where every tenant counts:** the super-admin page and `projects:count`.
- **Super-admins in one place.** `gate_before` answers `Some(true)` for the emails in
  `SUPER_ADMINS`, which passes `require_gate("admin")`, `user.authorize("manage", &team)` and
  `can('admin')` in views. Everyone else gets `None` and the normal check.
- **Secrets at rest.** `webhook_secret: Option<Encrypted<String>>` is stored sealed (AES-256-GCM
  under `APP_KEY`) and read as the plain secret, with no `encrypt`/`decrypt` calls in handlers;
  the settings page shows only its last characters, and `/team/secret` asks for the password
  again (`.require_password_confirmed()`) before showing it or making a new one.
- **A page per team on its own host.** `Routes::domain("{team}.localhost", …)` serves each
  team's public page at `acme.localhost:3000` (browsers send `*.localhost` to this machine;
  set `TEAM_DOMAIN=example.com` for `acme.example.com`). The handler reads the team from the
  `DomainParams` extractor (`domain.get("team")`), and a `fallback` inside the domain sends any
  other path there to `/`. That host gets only these routes; the app stays on the plain host.
  Slugs are made from the name (`acme`, then `acme-2`).
- **The current team in every view** with `App::share("team", …)`: `{% if team %}{{ team.name }}`.

## Tests

```bash
cargo test -p teams
```

[tests/teams.rs](tests/teams.rs) covers isolation between teams (lists, edit/update/delete by id),
switching (and being refused a team you're not in), falling back when removed from a team,
unique names per team, the fail-closed scope without a team, the unscoped counts and command,
the super-admin, adding members, the encrypted secret with its password confirmation, and the
public pages (per host, with counts but no project names, an unknown team a 404, other paths
redirected, a second "Acme" getting `acme-2`).

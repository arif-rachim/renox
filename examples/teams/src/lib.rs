//! Example: a multi-tenant SaaS. Users belong to teams, pick a current team,
//! and only ever see that team's projects.
//!
//! - Teams and memberships: a `team_user` pivot with a per-team `role`
//!   (`Pivot::attach_with`, `load_with_pivot`), in [`app::teams::model`].
//! - The current team in [`renox::context`](mod@renox::context): an `App::layer` middleware
//!   ([`app::tenancy`]) reads it from the session, checks the membership and
//!   sets it for the rest of the request.
//! - A tenant-scoped model: `Project` has `default_scope = "team_only"`, so
//!   handlers call `Project::query()` / `find_or_404` and never filter by
//!   team themselves. No team means no rows (fail closed).
//! - Scoped validation: a project name is unique per team.
//! - `Project::unscoped()` where every team counts: the super-admin page and
//!   the `projects:count` command.
//! - A super-admin through `App::gate_before` (the emails in `SUPER_ADMINS`).
//! - A team secret in an `Encrypted<String>` field, shown only after the
//!   password is confirmed (`require_password_confirmed`).
//! - Account pages from `Auth::new().account()`, with two-factor
//!   authentication from the `renox-2fa` plugin crate.
//! - "Continue with Google / GitHub" from the `renox-oauth` plugin crate,
//!   on when `GOOGLE_CLIENT_ID`/`_SECRET` or `GITHUB_CLIENT_ID`/`_SECRET` are set.
//!
//! Made with `rnx make:module teams`, `rnx make:module projects`,
//! `rnx make:model Team --module teams --migration`,
//! `rnx make:model Project --module projects --migration`, then filled in.
//! `projects:count` is an untyped `App::command` closure (`rnx make:command`
//! would write a typed one, as examples/hello and examples/crud have).
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed
//! SUPER_ADMINS=alice@example.com cargo run
//! ```

pub mod app;

use renox::axum::middleware::from_fn;
use renox::prelude::*;

pub use app::projects::model::Project;
pub use app::teams::model::{MEMBER, MEMBERS, OWNER, Team};
pub use app::tenancy::CurrentTeam;

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(Auth::new().account()) // login, register, and /account
        // Two-factor authentication, turned on from /account (renox-2fa).
        .module(renox_2fa::TwoFactor::new())
        // Social login (renox-oauth): buttons on the login and register pages,
        // linked logins on /account. Each provider is off until its
        // credentials are in `.env`.
        .module(renox_oauth::OAuth::new().google().github())
        .module(app::teams::Teams)
        .module(app::projects::Projects)
        .module(app::admin::Admin)
        // Runs after the user is loaded: puts the current team in the context.
        .layer(from_fn(app::tenancy::middleware))
        // `{% if team %}` in every view: the current team, or none.
        .share("team", |_| async { Ok(CurrentTeam::get()) })
        // Only super-admins pass this gate: `gate_before` answers for them,
        // everyone else gets the gate's own `false`.
        .gate("admin", |_| false)
        // Asked before every gate and policy check (`require_gate`,
        // `user.authorize`, `can(…)` in views): super-admins may do anything.
        .gate_before(|user, _ability| app::admin::is_super_admin(user).then_some(true))
        .command(
            "projects:count",
            "Count the projects of every team",
            |_args, state| async move {
                for (team, projects) in app::admin::project_counts(&state.db).await? {
                    println!("{:>5}  {}", projects, team.name);
                }
                Ok(())
            },
        )
        .seeder(seed)
}

/// `rnx db:seed`: two teams, three users (password `password123`), and a few
/// projects. Alice is in both teams, so she can switch between them.
async fn seed(state: AppState) -> Result {
    let db = state.db.clone();
    // Seeding twice is harmless: a seeded database stays as it is.
    if User::find_by_email(&db, "alice@example.com")
        .await?
        .is_some()
    {
        return Ok(());
    }
    let alice = User::register(&db, "Alice", "alice@example.com", "password123").await?;
    let bob = User::register(&db, "Bob", "bob@example.com", "password123").await?;
    let carol = User::register(&db, "Carol", "carol@example.com", "password123").await?;

    let acme = Team::found(&db, "Acme", &alice).await?;
    let globex = Team::found(&db, "Globex", &bob).await?;
    MEMBERS
        .attach_with(&db, acme.id, carol.id, &[("role", &MEMBER)])
        .await?;
    MEMBERS
        .attach_with(&db, globex.id, alice.id, &[("role", &MEMBER)])
        .await?;

    // Seeders have no current team, so they set `team_id` themselves.
    for (team, name) in [
        (&acme, "Website"),
        (&acme, "Mobile app"),
        (&acme, "Billing"),
        (&globex, "Website"), // the same name in another team is fine
        (&globex, "Warehouse"),
    ] {
        let project = Project {
            team_id: team.id,
            name: name.into(),
            description: format!("{name} for {}.", team.name),
            ..Default::default()
        };
        Project::create(&db, project).await?;
    }
    Ok(())
}

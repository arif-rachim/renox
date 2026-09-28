use renox::db::sql;
use renox::prelude::*;
use renox::testing::TestApp;
use teams::{CurrentTeam, MEMBER, MEMBERS, Project, Team};

/// Alice owns Acme, Bob owns Globex; each team has a few projects.
struct World {
    app: TestApp,
    alice: User,
    bob: User,
    acme: Team,
    globex: Team,
}

async fn world() -> World {
    world_with(|_| {}).await
}

async fn world_with(configure: impl FnOnce(&mut Config)) -> World {
    let app = TestApp::with_config(teams::app(), configure).await;
    let db = app.db();
    let alice = user(&app, "Alice", "alice@example.com").await;
    let bob = user(&app, "Bob", "bob@example.com").await;
    let acme = Team::found(db, "Acme", &alice).await.unwrap();
    let globex = Team::found(db, "Globex", &bob).await.unwrap();
    project(&app, &acme, "Website").await;
    project(&app, &acme, "Rocket skates").await;
    project(&app, &globex, "Warehouse").await;
    World {
        app,
        alice,
        bob,
        acme,
        globex,
    }
}

async fn user(app: &TestApp, name: &str, email: &str) -> User {
    User::register(app.db(), name, email, "password123")
        .await
        .unwrap()
}

/// Tests have no current team, so they set `team_id` themselves.
async fn project(app: &TestApp, team: &Team, name: &str) -> Project {
    let project = Project {
        team_id: team.id,
        name: name.into(),
        ..Default::default()
    };
    Project::create(app.db(), project).await.unwrap()
}

async fn join(app: &TestApp, team: &Team, user: &User) {
    MEMBERS
        .attach_with(app.db(), team.id, user.id, &[("role", &MEMBER)])
        .await
        .unwrap();
}

async fn project_named(app: &TestApp, name: &str) -> Project {
    Project::unscoped()
        .where_eq("name", name)
        .first(app.db())
        .await
        .unwrap()
        .unwrap()
}

#[renox::test]
async fn members_see_only_their_teams_projects() {
    let w = world().await;
    let theirs = project_named(&w.app, "Warehouse").await;
    w.app.acting_as(&w.alice);

    w.app
        .get("/projects")
        .await
        .assert_ok()
        .assert_see("Acme's projects")
        .assert_see("Rocket skates")
        .assert_dont_see("Warehouse");

    // Another team's project doesn't exist, as far as Alice can tell.
    let edit = format!("/projects/{}/edit", theirs.id);
    let url = format!("/projects/{}", theirs.id);
    w.app.get(&edit).await.assert_not_found();
    let id = theirs.id.to_string();
    w.app
        .put(&url, &[("id", &id), ("name", "Mine now")])
        .await
        .assert_not_found();
    w.app.delete(&url).await.assert_not_found();
    w.app
        .assert_database_has("projects", &[("id", &theirs.id), ("name", &"Warehouse")])
        .await;
}

#[renox::test]
async fn new_projects_go_into_the_current_team() {
    let w = world().await;
    w.app.acting_as(&w.alice);

    w.app
        .post("/projects", &[("name", "Roadmap"), ("description", "Q4")])
        .await
        .assert_redirect("/projects");
    w.app
        .assert_database_has("projects", &[("name", &"Roadmap"), ("team_id", &w.acme.id)])
        .await;
}

#[renox::test]
async fn switching_teams_changes_what_is_visible() {
    let w = world().await;
    join(&w.app, &w.globex, &w.alice).await;
    w.app.acting_as(&w.alice);

    // Her first team until she picks another.
    w.app
        .get("/projects")
        .await
        .assert_see("Rocket skates")
        .assert_dont_see("Warehouse");

    w.app
        .post(&format!("/teams/{}/switch", w.globex.id), &[])
        .await
        .assert_redirect("/projects");
    w.app
        .get("/projects")
        .await
        .assert_see("Globex's projects")
        .assert_see("Warehouse")
        .assert_dont_see("Rocket skates");

    // New projects now land in Globex.
    w.app
        .post("/projects", &[("name", "Forklifts")])
        .await
        .assert_redirect("/projects");
    w.app
        .assert_database_has(
            "projects",
            &[("name", &"Forklifts"), ("team_id", &w.globex.id)],
        )
        .await;
}

#[renox::test]
async fn switching_to_someone_elses_team_is_refused() {
    let w = world().await;
    w.app.acting_as(&w.alice);

    w.app
        .post(&format!("/teams/{}/switch", w.globex.id), &[])
        .await
        .assert_forbidden();
    w.app
        .get("/projects")
        .await
        .assert_see("Rocket skates")
        .assert_dont_see("Warehouse");
}

#[renox::test]
async fn a_team_removed_from_the_session_falls_back_to_another() {
    let w = world().await;
    join(&w.app, &w.globex, &w.alice).await;
    w.app.acting_as(&w.alice);
    w.app
        .post(&format!("/teams/{}/switch", w.globex.id), &[])
        .await
        .assert_redirect("/projects");

    // Bob removes Alice from Globex: her session still names it, but the
    // middleware checks the membership on every request.
    MEMBERS
        .detach(w.app.db(), w.globex.id, [w.alice.id])
        .await
        .unwrap();
    w.app
        .get("/projects")
        .await
        .assert_see("Rocket skates")
        .assert_dont_see("Warehouse");
}

#[renox::test]
async fn project_names_are_unique_per_team() {
    let w = world().await;

    // Taken in Acme…
    w.app.acting_as(&w.alice);
    w.app
        .htmx()
        .post("/projects", &[("name", "Website")])
        .await
        .assert_invalid("name");
    // …but free in Globex.
    w.app.acting_as(&w.bob);
    w.app
        .post("/projects", &[("name", "Website")])
        .await
        .assert_redirect("/projects");
    w.app.assert_database_count("projects", 4).await;

    // Saving a project under its own name is fine.
    w.app.acting_as(&w.alice);
    let website = sql("SELECT id FROM projects WHERE team_id = ? AND name = 'Website'")
        .bind(w.acme.id)
        .scalar::<i64>(w.app.db())
        .await
        .unwrap();
    let id = website.to_string();
    w.app
        .put(
            &format!("/projects/{website}"),
            &[("id", &id), ("name", "Website"), ("description", "New")],
        )
        .await
        .assert_redirect("/projects");
}

#[renox::test]
async fn without_a_team_there_are_no_rows() {
    let w = world().await;

    // No request, so no current team: the default scope fails closed.
    assert_eq!(Project::query().count(w.app.db()).await.unwrap(), 0);
    assert!(Project::find(w.app.db(), 1).await.unwrap().is_none());
    assert_eq!(Project::unscoped().count(w.app.db()).await.unwrap(), 3);

    // With a team in the context (as a job or a command would set it).
    let acme = w.acme.id;
    let db = w.app.db().clone();
    let count = renox::context::scope(async move {
        renox::context::set(CurrentTeam {
            id: acme,
            name: "Acme".into(),
            role: "owner".into(),
        });
        Project::query().count(&db).await.unwrap()
    })
    .await;
    assert_eq!(count, 2);

    // And saving a new project without one is refused.
    let err = Project::create(
        w.app.db(),
        Project {
            name: "Orphan".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.status(), StatusCode::FORBIDDEN);

    // A user in no team is asked to create one.
    let dave = user(&w.app, "Dave", "dave@example.com").await;
    w.app.acting_as(&dave);
    w.app.get("/projects").await.assert_redirect("/teams");
    w.app
        .get("/teams")
        .await
        .assert_ok()
        .assert_see("not in a team yet");
    w.app
        .post("/teams", &[("name", "Initech")])
        .await
        .assert_redirect("/projects");
    w.app
        .get("/projects")
        .await
        .assert_see("Initech's projects")
        .assert_see("No projects yet");
}

#[renox::test]
async fn unscoped_counts_see_every_team() {
    let w = world().await;
    let counts: Vec<(String, i64)> = teams::app::admin::project_counts(w.app.db())
        .await
        .unwrap()
        .into_iter()
        .map(|(team, n)| (team.name, n))
        .collect();
    assert_eq!(counts, [("Acme".into(), 2), ("Globex".into(), 1)]);
    w.app
        .kernel()
        .call("projects:count", Vec::<String>::new())
        .await
        .unwrap();
}

#[renox::test]
async fn super_admins_pass_every_gate_and_policy() {
    let w = world_with(|c| {
        c.vars.insert(
            "SUPER_ADMINS".into(),
            "ops@example.com, BOB@example.com".into(),
        );
    })
    .await;

    w.app.acting_as(&w.alice);
    w.app.get("/admin").await.assert_forbidden();

    w.app.acting_as(&w.bob);
    w.app
        .get("/admin")
        .await
        .assert_ok()
        .assert_see("Acme")
        .assert_see("Globex");

    // A plain member of Acme, yet `gate_before` lets Bob manage it.
    join(&w.app, &w.acme, &w.bob).await;
    w.app
        .post(&format!("/teams/{}/switch", w.acme.id), &[])
        .await
        .assert_redirect("/projects");
    w.app
        .get("/team")
        .await
        .assert_ok()
        .assert_see("Add member");
}

#[renox::test]
async fn owners_add_members_and_members_cannot() {
    let w = world().await;
    let carol = user(&w.app, "Carol", "carol@example.com").await;

    w.app.acting_as(&w.alice);
    w.app
        .htmx()
        .post("/team/members", &[("email", "nobody@example.com")])
        .await
        .assert_invalid("email");
    w.app
        .post("/team/members", &[("email", "Carol@Example.com")])
        .await
        .assert_redirect("/team");
    w.app
        .assert_database_has(
            "team_user",
            &[
                ("team_id", &w.acme.id),
                ("user_id", &carol.id),
                ("role", &"member"),
            ],
        )
        .await;
    w.app
        .get("/team")
        .await
        .assert_see("carol@example.com")
        .assert_see("Add member");

    // Carol is a member, not an owner.
    w.app.acting_as(&carol);
    w.app
        .get("/team")
        .await
        .assert_ok()
        .assert_see("Alice")
        .assert_dont_see("Add member");
    w.app
        .post("/team/members", &[("email", "bob@example.com")])
        .await
        .assert_forbidden();
}

#[renox::test]
async fn the_secret_is_encrypted_and_needs_the_password() {
    let w = world().await;
    let carol = user(&w.app, "Carol", "carol@example.com").await;
    join(&w.app, &w.acme, &carol).await;
    w.app.acting_as(&w.alice);

    // Not confirmed yet: to the confirmation page, then back.
    w.app
        .get("/team/secret")
        .await
        .assert_redirect("/confirm-password");
    w.app
        .post("/team/secret", &[])
        .await
        .assert_redirect("/confirm-password");
    w.app
        .post("/confirm-password", &[("password", "password123")])
        .await
        .assert_redirect("/team/secret");
    w.app
        .get("/team/secret")
        .await
        .assert_ok()
        .assert_see("No secret yet");

    w.app
        .post("/team/secret", &[])
        .await
        .assert_redirect("/team/secret");
    let stored: String = sql("SELECT webhook_secret FROM teams WHERE id = ?")
        .bind(w.acme.id)
        .scalar(w.app.db())
        .await
        .unwrap();
    let secret = w.app.state().decrypt(&stored).unwrap();
    assert!(secret.starts_with("whsec_"), "{secret}");
    assert!(!stored.contains(&secret), "the column holds ciphertext");
    assert!(!stored.starts_with("whsec_"));

    w.app.get("/team/secret").await.assert_see(&secret);
    // The settings page shows only its last characters.
    let tail = &secret[secret.len() - 4..];
    w.app
        .get("/team")
        .await
        .assert_see(&format!("whsec_…{tail}"))
        .assert_dont_see(&secret);

    // A member of the team can't see it, password or not.
    w.app.acting_as(&carol);
    w.app
        .post("/confirm-password", &[("password", "password123")])
        .await
        .assert_status(303);
    w.app.get("/team/secret").await.assert_forbidden();
}

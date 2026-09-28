//! Teams: create one, switch between yours, and (as an owner) add members
//! and manage the team's webhook secret.

pub mod model;

use renox::prelude::*;
use serde::Deserialize;

use super::tenancy::{CurrentTeam, SESSION_KEY};
use model::{MEMBER, MEMBERS, Membership, Team, USER_TEAMS};

pub struct Teams;

impl Module for Teams {
    fn name(&self) -> &'static str {
        "teams"
    }

    fn routes(&self) -> Routes {
        let members = Routes::new()
            .get("/teams", index)
            .name("teams.index")
            .post("/teams", store)
            .name("teams.store")
            .post("/teams/{id}/switch", switch)
            .name("teams.switch")
            .get("/team", settings)
            .name("team.settings")
            .post("/team/members", add_member)
            .name("team.members.store")
            .require_auth();
        // The secret asks for the password again (at most every three hours).
        let secret = Routes::new()
            .get("/team/secret", show_secret)
            .name("team.secret")
            .post("/team/secret", rotate_secret)
            .name("team.secret.rotate")
            .require_password_confirmed();
        members.merge(secret)
    }
}

#[derive(Deserialize)]
struct TeamForm {
    name: String,
}

impl Validate for TeamForm {
    fn rules(&self, v: &mut Validator) {
        v.field("name", &self.name).required().max(60);
    }
}

#[derive(Deserialize)]
struct MemberForm {
    email: String,
}

impl Validate for MemberForm {
    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
    }
}

/// The user's teams, with a switch button each, and a form for a new one.
async fn index(State(db): State<Db>, user: AuthUser) -> Result<View> {
    let mut teams = USER_TEAMS
        .load_with_pivot::<Team, Membership>(&db, [user.id])
        .await?
        .remove(&user.id)
        .unwrap_or_default();
    teams.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    Ok(view("teams/index.html", context! { teams }))
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Valid(form): Valid<TeamForm>,
) -> Result<Redirect> {
    let team = Team::found(&db, &form.name, &user).await?;
    session.put(SESSION_KEY, team.id)?; // work in the new team right away
    session.flash("status", format!("Team {} created.", team.name))?;
    Ok(Redirect::to("/projects"))
}

/// Makes another of the user's teams the current one.
async fn switch(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Redirect> {
    // Only teams the user belongs to (the middleware checks again anyway).
    if Team::role_of(&db, id, user.id).await?.is_none() {
        return Err(Error::Forbidden);
    }
    let team = Team::find_or_404(&db, id).await?;
    session.put(SESSION_KEY, team.id)?;
    session.flash("status", format!("Switched to {}.", team.name))?;
    Ok(Redirect::to("/projects"))
}

/// Members and the (masked) secret of the current team.
async fn settings(
    State(state): State<AppState>,
    user: AuthUser,
    team: CurrentTeam,
) -> Result<View> {
    let row = Team::find_or_404(&state.db, team.id).await?;
    let members = MEMBERS
        .load_with_pivot::<User, Membership>(&state.db, [team.id])
        .await?
        .remove(&team.id)
        .unwrap_or_default();
    let secret = match &row.webhook_secret {
        Some(sealed) => Some(model::mask(&state.decrypt(sealed)?)),
        None => None,
    };
    let manage = user.can("manage", &team);
    Ok(view(
        "teams/settings.html",
        context! { members, secret, manage },
    ))
}

/// Adds an existing user to the current team as a member.
async fn add_member(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    team: CurrentTeam,
    Valid(form): Valid<MemberForm>,
) -> Result<Redirect> {
    user.authorize("manage", &team)?;
    let Some(member) = User::find_by_email(&db, &form.email).await? else {
        let mut errors = Errors::new();
        errors.add("email", "Nobody has signed up with this email address yet.");
        return Err(errors.into());
    };
    let added = MEMBERS
        .attach_with(&db, team.id, member.id, &[("role", &MEMBER)])
        .await?;
    let status = if added {
        format!("{} joined {}.", member.name, team.name)
    } else {
        format!("{} is already in {}.", member.name, team.name)
    };
    session.flash("status", status)?;
    Ok(Redirect::to("/team"))
}

/// The secret in full, after the password was confirmed.
async fn show_secret(
    State(state): State<AppState>,
    user: AuthUser,
    team: CurrentTeam,
) -> Result<View> {
    user.authorize("manage", &team)?;
    let row = Team::find_or_404(&state.db, team.id).await?;
    let secret = match &row.webhook_secret {
        Some(sealed) => Some(state.decrypt(sealed)?), // Err if tampered with
        None => None,
    };
    Ok(view("teams/secret.html", context! { secret }))
}

/// Replaces the secret with a new one, stored encrypted.
async fn rotate_secret(
    State(state): State<AppState>,
    user: AuthUser,
    session: Session,
    team: CurrentTeam,
) -> Result<Redirect> {
    user.authorize("manage", &team)?;
    let mut row = Team::find_or_404(&state.db, team.id).await?;
    row.webhook_secret = Some(state.encrypt(&model::new_secret()));
    row.save_only(&state.db, &["webhook_secret"]).await?;
    session.flash(
        "status",
        "A new secret was made. Update your webhook sender.",
    )?;
    Ok(Redirect::to("/team/secret"))
}

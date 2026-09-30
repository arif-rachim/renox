//! Teams: create one, switch between yours, and (as an owner) add members
//! and manage the team's webhook secret.

pub mod model;

use renox::Toast;
use renox::db::Encrypted;
use renox::prelude::*;
use renox::validation::FormContext;
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

/// A form request: `prepare`, `authorize` and `after` run inside `Valid`,
/// so the handler only adds the member.
impl Validate for MemberForm {
    fn prepare(&mut self) {
        self.email = self.email.trim().to_lowercase();
    }

    /// Only the team's owners add members; anyone else gets 403 before the
    /// email is even looked at.
    async fn authorize(&self, form: &FormContext<'_>) -> Result<bool> {
        let (Some(user), Some(team)) = (form.user, CurrentTeam::get()) else {
            return Ok(false);
        };
        Ok(user.authorize("manage", &team).is_ok())
    }

    fn rules(&self, v: &mut Validator) {
        v.field("email", &self.email).required().email();
    }

    /// Checks the database once the email is valid: an error here is shown
    /// next to the field, like a rule's.
    async fn after(&self, form: &FormContext<'_>, errors: &mut Errors) -> Result {
        if User::find_by_email(&form.state.db, &self.email)
            .await?
            .is_none()
        {
            errors.add("email", "Nobody has signed up with this email address yet.");
        }
        Ok(())
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
) -> Result<(Toast, Redirect)> {
    let team = Team::found(&db, &form.name, &user).await?;
    session.put(SESSION_KEY, team.id)?; // work in the new team right away
    Ok((
        Toast::success(format!("Team {} created.", team.name)),
        Redirect::to("/projects"),
    ))
}

/// Makes another of the user's teams the current one.
async fn switch(
    State(db): State<Db>,
    user: AuthUser,
    session: Session,
    Path(id): Path<i64>,
) -> Result<(Toast, Redirect)> {
    // Only teams the user belongs to (the middleware checks again anyway).
    if Team::role_of(&db, id, user.id).await?.is_none() {
        return Err(Error::Forbidden);
    }
    let team = Team::find_or_404(&db, id).await?;
    session.put(SESSION_KEY, team.id)?;
    Ok((
        Toast::success(format!("Switched to {}.", team.name)),
        Redirect::to("/projects"),
    ))
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
    let secret = row.webhook_secret.as_deref().map(|s| model::mask(s));
    let manage = user.can("manage", &team);
    Ok(view(
        "teams/settings.html",
        context! { members, secret, manage },
    ))
}

/// Adds an existing user to the current team as a member.
async fn add_member(
    State(db): State<Db>,
    team: CurrentTeam,
    Valid(form): Valid<MemberForm>,
) -> Result<(Toast, Redirect)> {
    // `MemberForm::authorize` checked the role and `after` that the user exists.
    let member = User::find_by_email(&db, &form.email)
        .await?
        .ok_or(Error::NotFound)?;
    let added = MEMBERS
        .attach_with(&db, team.id, member.id, &[("role", &MEMBER)])
        .await?;
    let toast = if added {
        Toast::success(format!("{} joined {}.", member.name, team.name))
    } else {
        Toast::info(format!("{} is already in {}.", member.name, team.name))
    };
    Ok((toast, Redirect::to("/team")))
}

/// The secret in full, after the password was confirmed.
async fn show_secret(
    State(state): State<AppState>,
    user: AuthUser,
    team: CurrentTeam,
) -> Result<View> {
    user.authorize("manage", &team)?;
    let row = Team::find_or_404(&state.db, team.id).await?;
    // Opened when the row was read (an error if the column was tampered with).
    let secret = row.webhook_secret.map(Encrypted::into_inner);
    Ok(view("teams/secret.html", context! { secret }))
}

/// Replaces the secret with a new one, stored encrypted.
async fn rotate_secret(
    State(state): State<AppState>,
    user: AuthUser,
    team: CurrentTeam,
) -> Result<(Toast, Redirect)> {
    user.authorize("manage", &team)?;
    let mut row = Team::find_or_404(&state.db, team.id).await?;
    row.webhook_secret = Some(Encrypted::new(model::new_secret()));
    row.save_only(&state.db, &["webhook_secret"]).await?;
    Ok((
        Toast::success("A new secret was made. Update your webhook sender."),
        Redirect::to("/team/secret"),
    ))
}

//! The current team: which tenant this request works for.
//!
//! The session remembers the team the user picked (`current_team_id`). On
//! every request, [`middleware`] checks that the user still belongs to it
//! (falling back to their first team) and puts it in [`renox::context`](mod@renox::context).
//! From there the `Project` model's default scope, the validation rules and
//! the views read it; handlers take it as an extractor.

use renox::axum::extract::{FromRequestParts, Request};
use renox::axum::http::request::Parts;
use renox::axum::middleware::Next;
use renox::prelude::*;
use serde::Serialize;

/// Where the session keeps the team the user picked.
pub const SESSION_KEY: &str = "current_team_id";

/// The team this request works for, with the user's role in it.
#[derive(Clone, Debug, Serialize, FromRow)]
pub struct CurrentTeam {
    pub id: i64,
    pub name: String,
    /// `owner` or `member`.
    pub role: String,
}

impl CurrentTeam {
    /// The current team, if the middleware found one.
    pub fn get() -> Option<Self> {
        renox::context::get::<CurrentTeam>()
    }

    pub fn is_owner(&self) -> bool {
        self.role == crate::OWNER
    }
}

/// What a member may do with their current team. `App::gate_before` is
/// asked first, so super-admins pass too.
impl Policy for CurrentTeam {
    fn allows(&self, _user: &User, ability: &str) -> bool {
        match ability {
            "manage" => self.is_owner(), // members, the secret
            _ => false,
        }
    }
}

/// `team: CurrentTeam` in a handler: users without a team go to `/teams`
/// to create one.
impl<S: Send + Sync> FromRequestParts<S> for CurrentTeam {
    type Rejection = Redirect;

    async fn from_request_parts(_: &mut Parts, _: &S) -> Result<Self, Redirect> {
        CurrentTeam::get().ok_or_else(|| Redirect::to("/teams"))
    }
}

/// `App::layer` middleware: sets the current team for logged-in users.
pub async fn middleware(
    user: Option<AuthUser>,
    session: Session,
    req: Request,
    next: Next,
) -> Response {
    if let Some(user) = user
        && let Some(state) = renox::context::app()
    {
        let picked = session.get::<i64>(SESSION_KEY);
        match resolve(&state.db, user.id, picked).await {
            Ok(Some(team)) => {
                if picked != Some(team.id)
                    && let Err(err) = session.put(SESSION_KEY, team.id)
                {
                    return err.into_response();
                }
                renox::context::set(team);
            }
            Ok(None) => {} // in no team yet
            Err(err) => return err.into_response(),
        }
    }
    next.run(req).await
}

/// The team `picked` if the user belongs to it, else their first team.
pub async fn resolve(db: &Db, user_id: i64, picked: Option<i64>) -> Result<Option<CurrentTeam>> {
    Ok(renox::db::sql(
        "SELECT teams.id, teams.name, team_user.role
         FROM team_user JOIN teams ON teams.id = team_user.team_id
         WHERE team_user.user_id = ?
         ORDER BY CASE WHEN teams.id = ? THEN 0 ELSE 1 END, teams.id
         LIMIT 1",
    )
    .bind(user_id)
    .bind(picked.unwrap_or(0))
    .fetch_optional_as(db)
    .await?)
}

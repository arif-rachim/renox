use renox::db::{ModelHooks, Query};
use renox::prelude::*;
use serde::Serialize;

use crate::app::tenancy::CurrentTeam;

/// A project belongs to one team. Every query starts with [`team_only`], so
/// `Project::query()`, `find`, `find_or_404` and `where_eq` only ever see
/// the current team's rows; `Project::unscoped()` sees them all.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "projects", default_scope = "team_only", hooks)]
pub struct Project {
    pub id: i64,
    pub team_id: i64,
    pub name: String,
    pub description: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// The default scope: the current team's projects, and none without one
/// (a request with no team, or a job that forgot to set it, sees nothing
/// rather than everything).
pub fn team_only(query: Query<Project>) -> Query<Project> {
    match CurrentTeam::get() {
        Some(team) => query.where_eq("team_id", team.id),
        None => query.none(),
    }
}

impl ModelHooks for Project {
    /// New projects go into the current team, so handlers never set
    /// `team_id` themselves. Without a team the save fails.
    fn saving(&mut self, creating: bool) -> Result {
        if creating && self.team_id == 0 {
            let team = CurrentTeam::get().ok_or_else(|| {
                abort(StatusCode::FORBIDDEN, "Pick a team before adding projects.")
            })?;
            self.team_id = team.id;
        }
        Ok(())
    }
}

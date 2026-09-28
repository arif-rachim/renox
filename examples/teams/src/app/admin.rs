//! The super-admin's view across every team. It's the one place that uses
//! `Project::unscoped()`: an admin's report isn't about the current team.

use std::collections::HashMap;

use renox::prelude::*;

use super::projects::model::Project;
use super::teams::model::Team;

pub struct Admin;

impl Module for Admin {
    fn name(&self) -> &'static str {
        "admin"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/admin", teams)
            .name("admin.teams")
            .require_gate("admin") // 403 unless `gate_before` lets the user in
    }
}

/// Super-admins are the users whose email is in `SUPER_ADMINS`
/// (comma-separated), read from the running app's config.
pub fn is_super_admin(user: &User) -> bool {
    let Some(list) = renox::context::app().and_then(|state| state.config.var("SUPER_ADMINS"))
    else {
        return false;
    };
    list.split(',')
        .any(|email| email.trim().eq_ignore_ascii_case(&user.email))
}

/// Every team with its number of projects, across all tenants.
pub async fn project_counts(db: &Db) -> Result<Vec<(Team, i64)>> {
    let rows: Vec<(i64, i64)> = Project::unscoped() // no default scope: every team
        .group_by("team_id")
        .select_as(db, "team_id, COUNT(*)")
        .await?;
    let counts: HashMap<i64, i64> = rows.into_iter().collect();
    Ok(Team::query()
        .order_by("name")
        .get(db)
        .await?
        .into_iter()
        .map(|team| {
            let projects = counts.get(&team.id).copied().unwrap_or(0);
            (team, projects)
        })
        .collect())
}

async fn teams(State(db): State<Db>) -> Result<View> {
    let teams = project_counts(&db).await?;
    Ok(view("admin/teams.html", context! { teams }))
}

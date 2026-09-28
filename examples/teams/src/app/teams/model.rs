use renox::db::relations::Pivot;
use renox::prelude::*;
use serde::Serialize;

/// A tenant. Not scoped itself: memberships decide who sees it.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "teams")]
pub struct Team {
    pub id: i64,
    pub name: String,
    /// Sealed with `state.encrypt`; open it with `state.decrypt`. Never sent
    /// to templates.
    #[serde(skip_serializing)]
    pub webhook_secret: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

pub const OWNER: &str = "owner";
pub const MEMBER: &str = "member";

/// Team → users, through `team_user`, which also holds each member's role.
pub const MEMBERS: Pivot = Pivot::new("team_user", "team_id", "user_id").with_timestamps();

/// The same pivot seen from the user: user → teams.
pub const USER_TEAMS: Pivot = MEMBERS.inverse();

/// The pivot columns read with `load_with_pivot`.
#[derive(FromRow, Serialize, Debug, Clone)]
pub struct Membership {
    pub role: String,
    pub created_at: Option<DateTime>,
}

impl Team {
    /// Creates a team with `owner` as its owner, in one transaction.
    pub async fn found(db: &Db, name: &str, owner: &User) -> Result<Team> {
        let mut tx = db.begin().await?;
        let team = Team::create(
            &mut tx,
            Team {
                name: name.into(),
                ..Default::default()
            },
        )
        .await?;
        MEMBERS
            .attach_with(&mut tx, team.id, owner.id, &[("role", &OWNER)])
            .await?;
        tx.commit().await?;
        Ok(team)
    }

    /// The user's role in the team, or `None` when they aren't a member.
    pub async fn role_of(db: &Db, team_id: i64, user_id: i64) -> Result<Option<String>> {
        Ok(
            renox::db::sql("SELECT role FROM team_user WHERE team_id = ? AND user_id = ?")
                .bind(team_id)
                .bind(user_id)
                .scalar_optional(db)
                .await?,
        )
    }
}

/// A new webhook signing secret (32 random bytes, base64).
pub fn new_secret() -> String {
    let key = renox::generate_key(); // "base64:…", the same randomness as APP_KEY
    format!("whsec_{}", key.trim_start_matches("base64:"))
}

/// `whsec_…ab12`: enough to recognise a secret, not to use it.
pub fn mask(secret: &str) -> String {
    let tail: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("whsec_…{tail}")
}

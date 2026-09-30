use renox::db::Encrypted;
use renox::db::relations::Pivot;
use renox::prelude::*;
use serde::Serialize;

/// A tenant. Not scoped itself: memberships decide who sees it.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "teams")]
pub struct Team {
    pub id: i64,
    pub name: String,
    /// The team's host name part: `acme` for `acme.localhost`.
    pub slug: String,
    /// Stored encrypted with `APP_KEY`, read as the plain secret. Never sent
    /// to templates.
    #[serde(skip_serializing)]
    pub webhook_secret: Option<Encrypted<String>>,
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
        let slug = free_slug(&mut tx, name).await?;
        let team = Team::create(
            &mut tx,
            Team {
                name: name.into(),
                slug,
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

/// A new webhook signing secret (32 random bytes, URL-safe base64).
pub fn new_secret() -> String {
    format!("whsec_{}", renox::random_token())
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

/// `name` as a host label ("Acme Corp" → `acme-corp`), with `-2`, `-3`… when
/// another team has it.
async fn free_slug(tx: &mut renox::db::Transaction, name: &str) -> Result<String> {
    let base: String = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() {
        "team".to_owned()
    } else {
        base
    };
    let mut slug = base.clone();
    let mut n = 1;
    while Team::where_eq("slug", &slug).exists(&mut *tx).await? {
        n += 1;
        slug = format!("{base}-{n}");
    }
    Ok(slug)
}

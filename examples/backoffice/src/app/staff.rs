//! Staff and their roles (the `Permissions` module), for admins
//! (`staff.manage`). There is no sign-up: an admin adds a person with a
//! first password, and they get a mail to verify their address; until
//! they do, the back office sends them to the verification page. Every
//! change is written to the activity log.

use std::collections::BTreeMap;

use renox::Toast;
use renox::auth::send_verification;
use renox::grid::{Column, Grid, GridRequest};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ROLES;

pub fn grid() -> Grid {
    Grid::new("staff")
        .title("Staff")
        .column(
            Column::text("name", "Name")
                .mobile()
                .searchable()
                .description("email"),
        )
        .column(
            Column::text("email", "Email")
                .hidden()
                .searchable()
                .copyable(),
        )
        .column(Column::custom("roles", "Roles").mobile())
        .column(Column::datetime("email_verified_at", "Verified"))
        .column(Column::date("created_at", "Added").hidden())
        .column(Column::custom("actions", "Change"))
        .sort_by("name")
        .cards_on_mobile()
}

/// Every user's roles, by user id.
async fn roles_by_user(db: &Db) -> Result<BTreeMap<i64, Vec<String>>> {
    let pairs: Vec<(i64, String)> = renox::db::sql(
        "SELECT role_user.user_id, roles.name FROM role_user \
         JOIN roles ON roles.id = role_user.role_id ORDER BY roles.name",
    )
    .fetch_as(db)
    .await?;
    let mut map: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (user, role) in pairs {
        map.entry(user).or_default().push(role);
    }
    Ok(map)
}

pub(super) async fn index(State(db): State<Db>, request: GridRequest) -> Result<View> {
    let roles = roles_by_user(&db).await?;
    let page = grid().page(User::query(), &request).await?;
    // The roles sheets sit after the grid (it is a form, and forms don't
    // nest), one per person on the page.
    let members: Vec<_> = page
        .items()
        .iter()
        .map(|user| {
            json!({
                "id": user.id,
                "name": user.name,
                "roles": roles.get(&user.id).cloned().unwrap_or_default(),
            })
        })
        .collect();
    let staff =
        page.extend(|user| json!({ "roles": roles.get(&user.id).cloned().unwrap_or_default() }));
    let role_names: Vec<&str> = ROLES.iter().map(|(name, _)| *name).collect();
    Ok(view(
        "staff/index.html",
        context! { staff, members, role_names },
    ))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct StaffForm {
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, email, max = 150, unique("users", "email"))]
    pub email: String,
    /// Told to them in person; they can change it on `/account`.
    #[validate(required, min = 8, max = 100)]
    pub password: String,
    #[validate(required, one_of(&["admin", "cashier", "warehouse"]))]
    pub role: String,
}

pub(super) async fn store(
    State(state): State<AppState>,
    admin: AuthUser,
    Valid(form): Valid<StaffForm>,
) -> Result<(Toast, HxRefresh)> {
    let user = User::register(&state.db, &form.name, &form.email, &form.password).await?;
    user.assign_role(&state.db, &form.role).await?;
    send_verification(&state, &user).await?;
    renox::audit::record(
        &state.db,
        renox::audit::Entry::new("staff.added")
            .user(admin.id)
            .subject("users", user.id)
            .data(json!({ "email": user.email, "role": form.role })),
    )
    .await?;
    Ok((
        Toast::success(format!("{} added.", user.name))
            .body("A mail asks them to verify their address."),
        HxRefresh,
    ))
}

/// Ticked boxes: `roles=admin&roles=cashier` (none ticked: none sent).
#[derive(Deserialize, Serialize, Validate)]
pub struct RolesForm {
    #[serde(default)]
    #[validate(max = 3, each(one_of(&["admin", "cashier", "warehouse"])))]
    pub roles: Vec<String>,
}

pub(super) async fn update_roles(
    State(db): State<Db>,
    admin: AuthUser,
    Path(id): Path<i64>,
    Valid(form): Valid<RolesForm>,
) -> Result<(Toast, HxRefresh)> {
    let user = User::find_or_404(&db, id).await?;
    let known: Vec<&str> = form.roles.iter().map(String::as_str).collect();
    // Nobody locks themselves out of this page.
    if user.id == admin.id && !known.contains(&"admin") {
        let mut errors = Errors::new();
        errors.add("roles", "You can't take away your own admin role.");
        return Err(ValidationError::new(errors).into());
    }
    let before = user.roles(&db).await?;
    user.sync_roles(&db, &known).await?;
    renox::audit::record(
        &db,
        renox::audit::Entry::new("staff.roles_changed")
            .user(admin.id)
            .subject("users", user.id)
            .data(json!({ "from": before, "to": known })),
    )
    .await?;
    Ok((
        Toast::success(format!("{}'s roles saved.", user.name)),
        HxRefresh,
    ))
}

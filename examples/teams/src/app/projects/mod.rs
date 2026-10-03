//! The current team's projects. Nothing here mentions `team_id`: the model's
//! default scope and its `saving` hook take care of the tenant.

pub mod model;

use renox::Toast;
use renox::prelude::*;
use serde::Deserialize;

use super::tenancy::CurrentTeam;
use model::Project;

pub struct Projects;

impl Module for Projects {
    fn name(&self) -> &'static str {
        "projects"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", home)
            .name("home")
            .get("/projects", index)
            .name("projects.index")
            .get("/projects/new", create)
            .name("projects.create")
            .post("/projects", store)
            .name("projects.store")
            .get("/projects/{id}/edit", edit)
            .name("projects.edit")
            .put("/projects/{id}", update)
            .name("projects.update")
            .delete("/projects/{id}", destroy)
            .name("projects.destroy")
            .require_auth()
    }
}

/// What the create and edit forms send.
#[derive(Deserialize)]
struct ProjectForm {
    /// `<input type="hidden" name="id">` when editing; 0 when creating.
    #[serde(default)]
    id: i64,
    name: String,
    description: Option<String>,
}

impl Validate for ProjectForm {
    fn rules(&self, v: &mut Validator) {
        // Validation runs inside the request, so the current team is known.
        let team = CurrentTeam::get().map_or(0, |t| t.id);
        v.field("name", &self.name)
            .required()
            .max(80)
            .unique("projects", "name")
            .ignore(self.id)
            .where_eq("team_id", team) // unique per team, not across teams
            .message("Your team already has a project with this name.");
        v.field("description", &self.description).max(500);
    }
}

async fn home() -> Result<Redirect> {
    Redirect::route("projects.index", &[])
}

async fn index(State(db): State<Db>, _team: CurrentTeam) -> Result<View> {
    let projects = Project::query().order_by("name").get(&db).await?;
    Ok(view("projects/index.html", context! { projects }))
}

async fn create(_team: CurrentTeam) -> View {
    view("projects/form.html", context! {})
}

async fn store(
    State(db): State<Db>,
    _team: CurrentTeam,
    Valid(form): Valid<ProjectForm>,
) -> Result<(Toast, Redirect)> {
    let project = Project {
        name: form.name,
        description: form.description.unwrap_or_default(),
        ..Default::default() // team_id: filled in by the `saving` hook
    };
    let project = Project::create(&db, project).await?;
    Ok((
        Toast::success(format!("“{}” created.", project.name)),
        Redirect::route("projects.index", &[])?,
    ))
}

async fn edit(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    // Another team's project is a 404, as if it didn't exist.
    let project = Project::find_or_404(&db, id).await?;
    Ok(view("projects/form.html", context! { project }))
}

async fn update(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Valid(form): Valid<ProjectForm>,
) -> Result<(Toast, Redirect)> {
    let mut project = Project::find_or_404(&db, id).await?;
    // The unique rule skipped the form's `id`: it must be this project.
    abort_if(
        form.id != project.id,
        StatusCode::BAD_REQUEST,
        "Wrong project.",
    )?;
    project.name = form.name;
    project.description = form.description.unwrap_or_default();
    project.save(&db).await?;
    Ok((
        Toast::success(format!("“{}” saved.", project.name)),
        Redirect::route("projects.index", &[])?,
    ))
}

async fn destroy(State(db): State<Db>, Path(id): Path<i64>) -> Result<(Toast, Redirect)> {
    let mut project = Project::find_or_404(&db, id).await?;
    project.delete(&db).await?;
    Ok((
        Toast::success(format!("“{}” deleted.", project.name)),
        Redirect::route("projects.index", &[])?,
    ))
}

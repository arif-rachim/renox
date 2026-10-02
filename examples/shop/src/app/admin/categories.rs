//! The options of the product form's category select (`renox::select`):
//! searched as the admin types, added from what was typed ("Add “Juice”"),
//! and renamed in place. One URL: GET searches (or looks values up), POST
//! adds, PUT renames. The admin group's `require_role` covers all three.

use renox::prelude::*;
use renox::select::{OptionQuery, SelectOption};
use serde::Deserialize;

use crate::app::catalog::model::{Category, slug};

fn option(category: &Category) -> SelectOption {
    SelectOption::new(category.id, &category.name)
}

/// `GET ?q=te`: the categories whose names contain it (20 at most);
/// `GET ?values=2`: those categories, for labels the page lacks.
pub async fn options(State(db): State<Db>, query: OptionQuery) -> Result<Json<Vec<SelectOption>>> {
    let categories = if query.is_lookup() {
        Category::query()
            .where_in("id", query.values_as::<i64>())
            .get(&db)
            .await?
    } else {
        Category::query()
            .where_op("name", "like", format!("%{}%", query.q))
            .order_by("name")
            .limit(20)
            .get(&db)
            .await?
    };
    Ok(Json(categories.iter().map(option).collect()))
}

/// What the select sends to add a category: the text typed.
#[derive(Deserialize)]
pub struct NewCategory {
    label: String,
    /// Made from the label in `prepare`; unique like the name.
    #[serde(skip)]
    slug: String,
}

impl Validate for NewCategory {
    fn prepare(&mut self) {
        self.label = self.label.trim().to_owned();
        self.slug = slug(&self.label);
    }

    fn rules(&self, v: &mut Validator) {
        v.field("label", &self.label)
            .label("category")
            .required()
            .max(60)
            .unique("categories", "name");
        v.field("slug", &self.slug)
            .label("category")
            .unique("categories", "slug");
    }
}

/// `POST label=Juice`: the new category, which the select then chooses.
pub async fn create_option(
    State(db): State<Db>,
    Valid(form): Valid<NewCategory>,
) -> Result<Json<SelectOption>> {
    let category = Category::create(
        &db,
        Category {
            name: form.label,
            slug: form.slug,
            ..Default::default()
        },
    )
    .await?;
    Ok(Json(option(&category)))
}

/// What the select sends to rename the chosen category.
#[derive(Deserialize)]
pub struct RenamedCategory {
    value: i64,
    label: String,
}

impl Validate for RenamedCategory {
    fn prepare(&mut self) {
        self.label = self.label.trim().to_owned();
    }

    fn rules(&self, v: &mut Validator) {
        v.field("value", &self.value).exists("categories", "id");
        v.field("label", &self.label)
            .label("category")
            .required()
            .max(60)
            .unique("categories", "name")
            .ignore(self.value);
    }
}

/// `PUT value=2&label=Teas`: renamed. The slug stays, so the shop's links
/// to the category keep working.
pub async fn update_option(
    State(db): State<Db>,
    Valid(form): Valid<RenamedCategory>,
) -> Result<Json<SelectOption>> {
    let mut category = Category::find_or_404(&db, form.value).await?;
    category.name = form.label;
    category.save(&db).await?;
    Ok(Json(option(&category)))
}

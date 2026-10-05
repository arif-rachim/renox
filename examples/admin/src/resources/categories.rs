//! Categories: a small resource with no view page (its rows open the edit
//! page) and how many products each has (`Column::count_of`).

use renox::grid::Column;
use renox::prelude::*;
use renox_admin::{AdminResource, Entry, Field};
use serde::{Deserialize, Serialize};

use crate::{CATALOG, DELETE};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "categories")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Policy for Category {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "viewAny" | "view" => true,
            "create" | "update" => user.has_permission(CATALOG),
            _ => user.has_permission(DELETE),
        }
    }
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CategoryForm {
    #[validate(required, max = 60)]
    pub name: String,
    #[validate(max = 500)]
    pub description: Option<String>,
}

pub struct CategoryResource;

impl AdminResource for CategoryResource {
    type Model = Category;
    type Form = CategoryForm;

    fn label(&self) -> &str {
        "Category"
    }

    fn plural_label(&self) -> &str {
        "Categories"
    }

    fn navigation_group(&self) -> Option<&str> {
        Some("Catalog")
    }

    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name").searchable(),
            Column::count_of("products", "Products", "products", "category_id"),
            Column::text("description", "Description").limit(60),
        ]
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("name", "Name").required(),
            Field::textarea("description", "Description").span_full(),
        ]
    }

    /// No view page: there's nothing more to see than the list shows.
    fn entries(&self) -> Vec<Entry> {
        Vec::new()
    }

    fn fill(&self, category: &mut Category, form: CategoryForm) {
        category.name = form.name;
        category.description = form.description;
    }

    fn rules(&self, form: &CategoryForm, category: Option<&Category>, v: &mut Validator) {
        let rule = v.field("name", &form.name).unique("categories", "name");
        if let Some(category) = category {
            rule.ignore(category.id);
        }
    }
}

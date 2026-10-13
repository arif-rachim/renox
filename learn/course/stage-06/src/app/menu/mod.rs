pub mod category;
pub mod model;

use category::Category;
use model::{MenuItem, MenuItemForm};
use renox::prelude::*;
use serde::Serialize;

pub struct Menu;

impl Module for Menu {
    fn name(&self) -> &'static str {
        "menu"
    }

    fn register(&self, app: &mut Registry) {
        app.model::<category::Category>();
        app.model::<model::MenuItem>();
        app.view_data::<MenuPage>();
        app.view_data::<ItemPage>();
        app.view_data::<NewItemPage>();
    }

    fn routes(&self) -> Routes {
        let staff = Routes::new()
            .get("/menu/new", create)
            .name("menu.create")
            .post("/menu", store)
            .name("menu.store")
            .require_auth();
        Routes::new()
            .get("/menu", index)
            .name("menu.index")
            .get("/menu/{id}", show)
            .name("menu.show")
            .merge(staff)
    }
}

/// What `menu/index.html` gets.
#[derive(Serialize, ViewData)]
#[view(template = "menu/index.html")]
struct MenuPage {
    items: Vec<MenuItem>,
}

/// What `menu/show.html` gets.
#[derive(Serialize, ViewData)]
#[view(template = "menu/show.html")]
struct ItemPage {
    item: MenuItem,
    category: Category,
}

/// What `menu/create.html` gets: the categories as (id, name) pairs for the select.
#[derive(Serialize, ViewData)]
#[view(template = "menu/create.html")]
struct NewItemPage {
    categories: Vec<(i64, String)>,
}

async fn index(State(db): State<Db>) -> Result<View> {
    let items = MenuItem::query()
        .where_(MenuItem::AVAILABLE.eq(true))
        .order_by(MenuItem::CATEGORY_ID)
        .order_by(MenuItem::NAME)
        .get(&db)
        .await?;
    Ok(View::typed(MenuPage { items }))
}

async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let item = MenuItem::find_or_404(&db, id).await?;
    let category = Category::find_or_404(&db, item.category_id).await?;
    Ok(View::typed(ItemPage { item, category }))
}

async fn create(State(db): State<Db>) -> Result<View> {
    let categories = Category::query()
        .order_by(Category::NAME)
        .get(&db)
        .await?
        .into_iter()
        .map(|category| (category.id, category.name))
        .collect();
    Ok(View::typed(NewItemPage { categories }))
}

async fn store(
    State(db): State<Db>,
    user: AuthUser,
    Valid(form): Valid<MenuItemForm>,
) -> Result<(Toast, Redirect)> {
    let mut item = MenuItem::default();
    form.fill(&mut item);
    item.user_id = user.id;
    let created = MenuItem::create(&db, item).await?;
    Ok((
        Toast::success("Added to the menu."),
        Redirect::to(&format!("/menu/{}", created.id)),
    ))
}

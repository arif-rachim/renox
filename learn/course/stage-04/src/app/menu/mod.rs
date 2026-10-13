pub mod category;
pub mod model;

use category::Category;
use model::MenuItem;
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
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/menu", index)
            .name("menu.index")
            .get("/menu/{id}", show)
            .name("menu.show")
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

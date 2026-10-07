//! Products: the panel's main resource. A grid with a column from the
//! categories table, named filters, "Publish" and "Feature" actions, a
//! form with every common kind of field, a SKU unique among the products,
//! and soft deletes (a trash to restore from).

use renox::chrono::NaiveDate;
use renox::grid::{Column, Grid, Summary};
use renox::prelude::*;
use renox_admin::{AdminAction, AdminResource, Entry, Field, Filter};
use serde::{Deserialize, Serialize};

use crate::{CATALOG, DELETE};

const STATUSES: [(&str, &str); 3] = [
    ("draft", "Draft"),
    ("live", "Live"),
    ("archived", "Archived"),
];

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "products", soft_deletes)]
pub struct Product {
    pub id: i64,
    pub category_id: Option<i64>,
    pub name: String,
    pub sku: String,
    /// In cents (the smallest unit of `APP_CURRENCY`, USD).
    pub price: i64,
    pub stock: i64,
    pub status: String,
    pub featured: bool,
    pub released_on: Option<NaiveDate>,
    pub description: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub deleted_at: Option<DateTime>,
}

/// Everyone in the panel looks; editors and admins change the catalog;
/// only admins delete, restore and purge.
impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "viewAny" | "view" => true,
            "create" | "update" => user.has_permission(CATALOG),
            _ => user.has_permission(DELETE),
        }
    }
}

/// The create and edit form, checked by its own rules.
#[derive(Deserialize, Serialize, Validate)]
pub struct ProductForm {
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, max = 30, alpha_dash)]
    pub sku: String,
    #[validate(exists("categories", "id"))]
    pub category_id: Option<i64>,
    #[validate(required, min = 0)]
    pub price: i64,
    #[validate(required, min = 0)]
    pub stock: i64,
    #[validate(required, one_of(&["draft", "live", "archived"]))]
    pub status: String,
    pub featured: bool,
    pub released_on: Option<NaiveDate>,
    #[validate(max = 2000)]
    pub description: Option<String>,
}

pub struct ProductResource;

impl AdminResource for ProductResource {
    type Model = Product;
    type Form = ProductForm;

    fn label(&self) -> &str {
        "Product"
    }

    fn plural_label(&self) -> &str {
        "Products"
    }

    fn navigation_group(&self) -> Option<&str> {
        Some("Catalog")
    }

    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name")
                .searchable()
                .frozen()
                .mobile()
                .description("sku"),
            Column::text("sku", "SKU").searchable().hidden().copyable(),
            Column::related("category", "Category", "categories", "category_id", "name"),
            Column::money("price", "Price").mobile(),
            Column::custom("level", "Stock"),
            Column::select("status", "Status", STATUSES).badges(&[
                ("live", "success"),
                ("draft", "neutral"),
                ("archived", "warning"),
            ]),
            Column::bool("featured", "Featured").icons(),
            Column::date("released_on", "Released"),
            Column::number("stock", "Units")
                .hidden()
                .summary(Summary::Sum),
        ]
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("name", "Name").required().autocomplete("off"),
            Field::text("sku", "SKU")
                .required()
                .hint("Letters, digits, dashes and underscores.")
                .autocomplete("off"),
            Field::belongs_to("category_id", "Category", "categories", "name"),
            Field::select("status", "Status", STATUSES)
                .required()
                .default_value("draft"),
            Field::money("price", "Price").required(),
            Field::number("stock", "In stock")
                .required()
                .min(0)
                .default_value(0),
            Field::date("released_on", "Released"),
            Field::toggle("featured", "Featured on the home page"),
            Field::textarea("description", "Description").span_full(),
        ]
    }

    fn entries(&self) -> Vec<Entry> {
        vec![
            Entry::text("name", "Name"),
            Entry::text("sku", "SKU").copyable(),
            Entry::new("price", "Price").format("money"),
            Entry::new("stock", "In stock").format("number"),
            Entry::new("status", "Status").labels(STATUSES).badge(),
            Entry::new("featured", "Featured").format("bool"),
            Entry::new("released_on", "Released").format("date"),
            Entry::new("updated_at", "Last changed").format("since"),
            Entry::new("description", "Description")
                .format("markdown")
                .span_full(),
        ]
    }

    fn record_title(&self, product: &Product) -> String {
        product.name.clone()
    }

    fn fill(&self, product: &mut Product, form: ProductForm) {
        product.name = form.name;
        product.sku = form.sku.to_uppercase();
        product.category_id = form.category_id;
        product.price = form.price;
        product.stock = form.stock;
        product.status = form.status;
        product.featured = form.featured;
        product.released_on = form.released_on;
        product.description = form.description;
    }

    /// The SKU is unique among the products, but a product keeps its own.
    fn rules(&self, form: &ProductForm, product: Option<&Product>, v: &mut Validator) {
        let sku = form.sku.to_uppercase();
        let rule = v.field("sku", &sku).label("SKU").unique("products", "sku");
        if let Some(product) = product {
            rule.ignore(product.id);
        }
    }

    fn filters(&self) -> Vec<Filter<Product>> {
        vec![
            Filter::new("live", "Live", |q| q.where_eq("status", "live")),
            Filter::new("low", "Low stock", |q| q.where_op("stock", "<", 5)),
            Filter::new("featured", "Featured", |q| q.where_eq("featured", true)),
        ]
    }

    fn actions(&self) -> Vec<AdminAction<Product>> {
        vec![
            AdminAction::new(
                "publish",
                "Publish",
                |products: Vec<Product>, cx| async move {
                    let ids: Vec<i64> = products.iter().map(|p| p.id).collect();
                    let n = Product::query()
                        .where_in("id", ids)
                        .update(&cx.state.db, &[("status", &"live")])
                        .await?;
                    Ok(Toast::success(format!("{n} published.")))
                },
            )
            .row(),
            AdminAction::new(
                "feature",
                "Feature",
                |products: Vec<Product>, cx| async move {
                    let ids: Vec<i64> = products.iter().map(|p| p.id).collect();
                    let n = Product::query()
                        .where_in("id", ids)
                        .update(&cx.state.db, &[("featured", &true)])
                        .await?;
                    Ok(Toast::success(format!("{n} featured.")))
                },
            ),
        ]
    }

    fn grid(&self, grid: Grid) -> Grid {
        grid.sort_by("name").cards_on_mobile()
    }
}

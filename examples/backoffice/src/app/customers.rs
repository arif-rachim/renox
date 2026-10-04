//! Customers: a grid edited in place (cells send `PATCH /customers/{id}`
//! with only the changed field), and a "New customer" form in a sheet
//! (the kit's `action_sheet`). How many invoices each has and what they
//! owe come from the `invoices` table (`Column::count_of`, `sum_of`).

use renox::grid::{Column, Grid, GridRequest};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "customers")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub city: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// `can_edit`: staff with `customers.manage` edit cells in place.
pub fn grid(can_edit: bool) -> Grid {
    let grid = Grid::new("customers")
        .title("Customers")
        .column(
            Column::text("name", "Name")
                .frozen()
                .mobile()
                .searchable()
                .editable()
                .description("email"),
        )
        .column(
            Column::text("email", "Email")
                .hidden()
                .searchable()
                .editable(),
        )
        .column(Column::text("phone", "Phone").editable().copyable())
        .column(
            Column::text("city", "City")
                .mobile()
                .searchable()
                .editable(),
        )
        .column(Column::count_of(
            "invoices",
            "Invoices",
            "invoices",
            "customer_id",
        ))
        .column(
            Column::sum_of("billed", "Billed (Rp)", "invoices", "customer_id", "total")
                .summary(renox::grid::Summary::Sum),
        )
        .column(Column::date("created_at", "Since").hidden())
        .sort_by("name")
        .cards_on_mobile()
        .exports()
        .empty_state("No customers yet", Some("Add the first with New customer."));
    if can_edit {
        grid.edit_url("/customers/{id}")
    } else {
        grid
    }
}

pub(super) async fn index(request: GridRequest, user: AuthUser) -> Result<Response> {
    let grid = grid(user.has_permission(crate::CUSTOMERS));
    if let Some(file) = grid.export(Customer::query(), &request).await? {
        return Ok(file);
    }
    let customers = grid.page(Customer::query(), &request).await?;
    Ok(view("customers/index.html", context! { customers }).into_response())
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CustomerForm {
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(email, max = 150)]
    pub email: Option<String>,
    #[validate(max = 30)]
    pub phone: Option<String>,
    #[validate(max = 60)]
    pub city: Option<String>,
}

pub(super) async fn store(
    State(db): State<Db>,
    Valid(form): Valid<CustomerForm>,
) -> Result<(Toast, HxRefresh)> {
    let customer = Customer::create(
        &db,
        Customer {
            name: form.name,
            email: form.email,
            phone: form.phone,
            city: form.city,
            ..Default::default()
        },
    )
    .await?;
    Ok((
        Toast::success(format!("{} added.", customer.name)),
        HxRefresh,
    ))
}

/// What the grid sends when a cell changes: only that field.
#[derive(Deserialize, Validate)]
pub struct CustomerEdit {
    #[validate(max = 100)]
    pub name: Option<String>,
    #[validate(email, max = 150)]
    pub email: Option<String>,
    #[validate(max = 30)]
    pub phone: Option<String>,
    #[validate(max = 60)]
    pub city: Option<String>,
}

pub(super) async fn update(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Valid(edit): Valid<CustomerEdit>,
) -> Result<Toast> {
    let mut customer = Customer::find_or_404(&db, id).await?;
    if let Some(name) = edit.name.filter(|n| !n.trim().is_empty()) {
        customer.name = name.trim().to_owned();
    }
    // An emptied cell arrives as `None` and keeps the old value.
    if let Some(email) = edit.email {
        customer.email = Some(email);
    }
    if let Some(phone) = edit.phone {
        customer.phone = Some(phone);
    }
    if let Some(city) = edit.city {
        customer.city = Some(city);
    }
    customer.save(&db).await?;
    Ok(Toast::success(format!("{} saved.", customer.name)))
}

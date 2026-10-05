//! Customers: only admins change them; editors look. Its view page lists
//! an entry per column (the default).

use renox::grid::Column;
use renox::prelude::*;
use renox_admin::{AdminResource, Field};
use serde::{Deserialize, Serialize};

use crate::CUSTOMERS;

const TIERS: [(&str, &str); 3] = [
    ("regular", "Regular"),
    ("silver", "Silver"),
    ("gold", "Gold"),
];

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "customers")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub city: Option<String>,
    pub tier: String,
    pub newsletter: bool,
    pub notes: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl Policy for Customer {
    fn allows(&self, user: &User, ability: &str) -> bool {
        matches!(ability, "viewAny" | "view") || user.has_permission(CUSTOMERS)
    }
}

#[derive(Deserialize, Serialize, Validate)]
pub struct CustomerForm {
    #[validate(required, max = 100)]
    pub name: String,
    #[validate(required, email, max = 150)]
    pub email: String,
    #[validate(max = 30)]
    pub phone: Option<String>,
    #[validate(max = 60)]
    pub city: Option<String>,
    #[validate(required, one_of(&["regular", "silver", "gold"]))]
    pub tier: String,
    pub newsletter: bool,
    #[validate(max = 2000)]
    pub notes: Option<String>,
}

pub struct CustomerResource;

impl AdminResource for CustomerResource {
    type Model = Customer;
    type Form = CustomerForm;

    fn label(&self) -> &str {
        "Customer"
    }

    fn plural_label(&self) -> &str {
        "Customers"
    }

    fn columns(&self) -> Vec<Column> {
        vec![
            Column::text("name", "Name")
                .searchable()
                .mobile()
                .description("email"),
            Column::text("email", "Email")
                .searchable()
                .hidden()
                .copyable(),
            Column::text("phone", "Phone"),
            Column::text("city", "City").searchable().mobile(),
            Column::select("tier", "Tier", TIERS)
                .badges(&[("gold", "warning"), ("silver", "info")]),
            Column::bool("newsletter", "Newsletter"),
            Column::datetime("created_at", "Since").hidden(),
        ]
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("name", "Name").required(),
            Field::email("email", "Email").required(),
            Field::tel("phone", "Phone"),
            Field::text("city", "City"),
            Field::select("tier", "Tier", TIERS)
                .required()
                .default_value("regular"),
            Field::checkbox("newsletter", "Gets the newsletter"),
            Field::textarea("notes", "Notes").span_full(),
        ]
    }

    fn record_title(&self, customer: &Customer) -> String {
        customer.name.clone()
    }

    fn fill(&self, customer: &mut Customer, form: CustomerForm) {
        customer.name = form.name;
        customer.email = form.email.trim().to_lowercase();
        customer.phone = form.phone;
        customer.city = form.city;
        customer.tier = form.tier;
        customer.newsletter = form.newsletter;
        customer.notes = form.notes;
    }

    fn rules(&self, form: &CustomerForm, customer: Option<&Customer>, v: &mut Validator) {
        let email = form.email.trim().to_lowercase();
        let rule = v.field("email", &email).unique("customers", "email");
        if let Some(customer) = customer {
            rule.ignore(customer.id);
        }
    }
}

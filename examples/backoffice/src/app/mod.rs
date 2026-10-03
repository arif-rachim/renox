//! The back office as one module: its routes, guarded by role, and the
//! jobs and webhooks it registers. Each area is a file (or folder) here,
//! made with `rnx make:module <name>` and `rnx make:model <Name> --module
//! <name> -m`, then merged into this one module so every page shares one
//! guard: staff must be logged in and verified, and each change needs its
//! permission (`crate::ROLES`).

pub mod activity;
pub mod customers;
pub mod dashboard;
pub mod invoices;
pub mod products;
pub mod seed;
pub mod settings;
pub mod staff;

use renox::prelude::*;

use crate::{ACTIVITY, CUSTOMERS, EXPORTS, INVOICES, PRODUCTS, SETTINGS, STAFF, STOCK};

pub struct BackOffice;

impl Module for BackOffice {
    fn name(&self) -> &'static str {
        "backoffice"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", dashboard::show)
            .name("home")
            .get("/customers", customers::index)
            .name("customers.index")
            .get("/products", products::index)
            .name("products.index")
            .get("/products/{id}", products::show)
            .name("products.show")
            .get("/invoices", invoices::index)
            .name("invoices.index")
            .get("/invoices/{id}", invoices::show)
            .name("invoices.show")
            .get("/invoices/{id}/print", invoices::print)
            .name("invoices.print")
            // A guard covers the routes added before it, so each permission
            // is its own group, merged in.
            .merge(
                Routes::new()
                    .post("/customers", customers::store)
                    .name("customers.store")
                    .patch("/customers/{id}", customers::update)
                    .name("customers.update")
                    .require_permission(CUSTOMERS),
            )
            .merge(
                Routes::new()
                    .post("/products", products::store)
                    .name("products.store")
                    .patch("/products/{id}", products::update)
                    .name("products.update")
                    .post("/products/import", products::import::upload)
                    .name("products.import")
                    .post("/products/bulk/active/{active}", products::bulk_active)
                    .name("products.bulk_active")
                    .require_permission(PRODUCTS),
            )
            .merge(
                Routes::new()
                    .post("/products/{id}/stock", products::stock::adjust)
                    .name("products.stock")
                    .require_permission(STOCK),
            )
            .merge(
                Routes::new()
                    .get("/invoices/new", invoices::create)
                    .name("invoices.create")
                    .post("/invoices", invoices::store)
                    .name("invoices.store")
                    .post("/invoices/{id}/issue", invoices::issue)
                    .name("invoices.issue")
                    .post("/invoices/{id}/paid", invoices::mark_paid)
                    .name("invoices.paid")
                    .post("/invoices/{id}/void", invoices::void)
                    .name("invoices.void")
                    .post("/invoices/{id}/payment-link", invoices::payments::link)
                    .name("invoices.payment_link")
                    .require_permission(INVOICES),
            )
            .merge(
                Routes::new()
                    .post("/invoices/export", invoices::export::start)
                    .name("invoices.export")
                    .require_permission(EXPORTS),
            )
            .merge(
                Routes::new()
                    .get("/staff", staff::index)
                    .name("staff.index")
                    .post("/staff", staff::store)
                    .name("staff.store")
                    .put("/staff/{id}/roles", staff::update_roles)
                    .name("staff.roles")
                    .require_permission(STAFF),
            )
            .merge(
                Routes::new()
                    .get("/activity", activity::index)
                    .name("activity.index")
                    .require_permission(ACTIVITY),
            )
            .merge(
                Routes::new()
                    .get("/settings", settings::edit)
                    .name("settings.edit")
                    .put("/settings", settings::update)
                    .name("settings.update")
                    .require_permission(SETTINGS),
            )
            // Everything above: logged in, with a verified email.
            .require_verified()
            // The payment gateways call these; they check their own
            // signatures, so they sit outside the login.
            .webhook::<invoices::payments::Midtrans>("/webhooks/midtrans")
            .webhook::<invoices::payments::Xendit>("/webhooks/xendit")
    }

    fn register(&self, app: &mut Registry) {
        app.job::<invoices::export::ExportInvoices>()
            .webhook::<invoices::payments::Midtrans>()
            .webhook::<invoices::payments::Xendit>();
    }
}

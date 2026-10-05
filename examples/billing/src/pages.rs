//! Inkwell's own pages: the home page, and two pages for subscribers.

use renox::prelude::*;
use renox_billing::{Billing, SubscriptionRoutes};

pub struct Pages;

impl Module for Pages {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Routes {
        let subscribers = Routes::new()
            .get("/reports", reports)
            .name("reports")
            .require_subscription()
            .merge(
                Routes::new()
                    .get("/exports", exports)
                    .name("exports")
                    .require_plan(&["pro", "pro-idr"]),
            )
            .require_auth();
        Routes::new().get("/", home).name("home").merge(subscribers)
    }
}

/// What the user has, and where to go next.
async fn home(State(state): State<AppState>, user: Option<AuthUser>) -> Result<View> {
    let (subscribed, on_trial, plan) = match &user {
        Some(user) => {
            let billing = Billing::of(&state, &**user);
            let subscription = billing.subscription().await?.filter(|s| s.valid());
            (
                subscription.is_some(),
                subscription.as_ref().is_some_and(|s| s.on_trial()),
                subscription.map(|s| s.plan),
            )
        }
        None => (false, false, None),
    };
    Ok(view("home.html", context! { subscribed, on_trial, plan }))
}

async fn reports() -> View {
    view("reports.html", context! {})
}

async fn exports() -> View {
    view("exports.html", context! {})
}

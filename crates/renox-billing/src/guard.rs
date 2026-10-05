//! Routes for subscribers only.

use std::sync::Arc;

use renox::axum::extract::Request;
use renox::axum::http::header;
use renox::axum::middleware::{Next, from_fn};
use renox::prelude::*;

use crate::Billing;

/// Guards for [`Routes`]: only users with a valid `default` subscription
/// (on a trial, paid for, or canceled with time left) get through. Others
/// go to the plans page (`billing.plans`) with a toast; a JSON request gets
/// `402 Payment Required`.
///
/// Like `require_auth`, a guard covers the routes added before it, and it
/// needs a logged-in user: put `.require_auth()` after it.
///
/// ```
/// use renox::prelude::*;
/// use renox_billing::SubscriptionRoutes;
///
/// # async fn reports() -> &'static str { "" }
/// # async fn exports() -> &'static str { "" }
/// # let _: Routes =
/// Routes::new()
///     .get("/reports", reports)
///     .require_subscription()
///     .merge(Routes::new().get("/exports", exports).require_plan(&["pro", "business"]))
///     .require_auth()
/// # ;
/// ```
pub trait SubscriptionRoutes {
    /// Lets through users with a valid subscription to any plan.
    fn require_subscription(self) -> Self;

    /// Lets through users with a valid subscription to one of `plans`.
    fn require_plan(self, plans: &[&str]) -> Self;
}

impl SubscriptionRoutes for Routes {
    fn require_subscription(self) -> Self {
        guard(self, None)
    }

    fn require_plan(self, plans: &[&str]) -> Self {
        let plans = plans.iter().map(|p| (*p).to_owned()).collect();
        guard(self, Some(Arc::new(plans)))
    }
}

fn guard(routes: Routes, plans: Option<Arc<Vec<String>>>) -> Routes {
    routes.route_layer(from_fn(
        move |user: Option<AuthUser>, request: Request, next: Next| {
            let plans = plans.clone();
            async move { check(plans, user, request, next).await }
        },
    ))
}

async fn check(
    plans: Option<Arc<Vec<String>>>,
    user: Option<AuthUser>,
    request: Request,
    next: Next,
) -> Response {
    let Some(state) = request.extensions().get::<AppState>().cloned() else {
        return next.run(request).await;
    };
    let Some(user) = user else {
        return Error::Unauthorized.into_response();
    };
    let subscription = match Billing::of(&state, &user).subscription().await {
        Ok(subscription) => subscription,
        Err(err) => return err.into_response(),
    };
    let allowed = subscription
        .is_some_and(|s| s.valid() && plans.as_ref().is_none_or(|plans| plans.contains(&s.plan)));
    if allowed {
        return next.run(request).await;
    }
    let wants_json = request
        .headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains("application/json"));
    if wants_json {
        let body = json!({ "message": "This needs a subscription." });
        return (StatusCode::PAYMENT_REQUIRED, Json(body)).into_response();
    }
    let to = state
        .url("billing.plans", &[])
        .unwrap_or_else(|_| "/".into());
    let message = if plans.is_some() {
        "This needs another plan."
    } else {
        "Choose a plan to continue."
    };
    let htmx = request.headers().contains_key("hx-request");
    if htmx {
        (Toast::info(message), HxRedirect(to)).into_response()
    } else {
        (Toast::info(message), Redirect::to(&to)).into_response()
    }
}

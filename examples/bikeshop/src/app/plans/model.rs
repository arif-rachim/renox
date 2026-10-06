//! Service plans: a set of tasks repeated every week, fortnight, month or
//! quarter, and the customers' bikes subscribed to them. Each visit becomes
//! a work order (`WorkSource::Plan`) scheduled automatically (#237).
//!
//! Migration: `migrations/20260101000900_create_plans_tables.*`.

use renox::chrono::NaiveDate;
use renox::db::relations::Pivot;
use renox::prelude::*;
use serde::Serialize;

use crate::app::access::{StoreAttr, StoreRecord, catalogue};

/// How often a plan's visit comes round.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frequency {
    Weekly,
    Fortnightly,
    #[default]
    Monthly,
    Quarterly,
}

impl Frequency {
    /// Days between two visits (a month counts as 30 here; the scheduler
    /// uses calendar months).
    pub fn days(self) -> i64 {
        match self {
            Frequency::Weekly => 7,
            Frequency::Fortnightly => 14,
            Frequency::Monthly => 30,
            Frequency::Quarterly => 91,
        }
    }
}

/// A service plan customers subscribe a bike to.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "service_plans")]
pub struct ServicePlan {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub frequency: Frequency,
    /// Per period, in the smallest unit of `APP_CURRENCY`.
    pub price: i64,
    pub description: String,
    pub active: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// The tasks of each plan: plan → service tasks.
pub const PLAN_TASKS: Pivot =
    Pivot::new("plan_tasks", "service_plan_id", "service_task_id").with_timestamps();

/// Where a subscription stands.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscriptionStatus {
    #[default]
    Active,
    Paused,
    Cancelled,
}

/// A customer's bike on a plan, serviced at one store.
#[derive(Model, Serialize, Default, Debug, Clone)]
#[model(table = "plan_subscriptions")]
pub struct PlanSubscription {
    pub id: i64,
    pub customer_bike_id: i64,
    pub service_plan_id: i64,
    /// The store whose workshop does the visits.
    pub store_id: i64,
    /// 1 = Monday … 7 = Sunday.
    pub preferred_weekday: i64,
    pub status: SubscriptionStatus,
    pub starts_on: NaiveDate,
    pub next_visit_on: Option<NaiveDate>,
    pub cancelled_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

impl StoreRecord for PlanSubscription {
    const VIEW: &'static str = catalogue::WORKORDERS_VIEW;
    const STORE_COLUMNS: &'static [&'static str] = &["store_id"];

    fn store_id(&self, _attr: StoreAttr) -> Option<i64> {
        Some(self.store_id)
    }
}

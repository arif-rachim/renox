use renox::chrono::{Duration, NaiveDate};
use renox::db::Json;
use renox::fake::Fake;
use renox::fake::faker::internet::en::SafeEmail;
use renox::fake::faker::name::en::Name;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

/// An order on the sales dashboard. `tags` and `trend` are JSON arrays
/// (a `tags` column and a chart in the grid); `created_by`/`updated_by` are
/// names, for the audit details.
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "orders")]
pub struct Order {
    pub id: i64,
    pub number: String,
    pub customer: String,
    pub email: String,
    pub region: String,
    pub city: String,
    pub status: String,
    pub tags: Json<Vec<String>>,
    pub items: i64,
    /// In rupiah.
    pub total: i64,
    /// Percent.
    pub discount: f64,
    pub ordered_on: NaiveDate,
    pub paid: bool,
    /// Units sold on each of the last seven days.
    pub trend: Json<Vec<i64>>,
    /// Percent of the items shipped.
    pub fulfilled: i64,
    /// Where the row sits when sorted by hand (`Grid::reorder`).
    pub position: i64,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// Regions and their cities.
pub const PLACES: &[(&str, &[&str])] = &[
    ("java", &["Jakarta", "Bandung", "Surabaya", "Semarang"]),
    ("sumatra", &["Medan", "Palembang", "Padang"]),
    ("bali_nusa", &["Denpasar", "Mataram"]),
    ("sulawesi", &["Makassar", "Manado"]),
];

pub const REGIONS: [(&str, &str); 4] = [
    ("java", "Java"),
    ("sumatra", "Sumatra"),
    ("bali_nusa", "Bali & Nusa Tenggara"),
    ("sulawesi", "Sulawesi"),
];

pub const STATUSES: [(&str, &str); 4] = [
    ("new", "New"),
    ("paid", "Paid"),
    ("shipped", "Shipped"),
    ("cancelled", "Cancelled"),
];

pub const TAGS: [(&str, &str); 4] = [
    ("online", "Online"),
    ("store", "In store"),
    ("promo", "Promo"),
    ("wholesale", "Wholesale"),
];

const STAFF: [&str; 4] = ["Arif", "Dewi", "Budi", "Sari"];

fn pick<T: Copy>(items: &[T]) -> T {
    items[(0..items.len()).fake::<usize>()]
}

impl Factory for Order {
    fn definition() -> Self {
        let (region, cities) = pick(PLACES);
        let status = pick(&STATUSES).0;
        let tags: Vec<String> = TAGS
            .iter()
            .filter(|_| (0..3).fake::<u8>() == 0)
            .map(|(v, _)| (*v).to_owned())
            .collect();
        let items: i64 = (1..40).fake();
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default();
        Order {
            number: format!("SO-{:05}", (1..99_999).fake::<u32>()),
            customer: Name().fake(),
            email: SafeEmail().fake(),
            region: region.to_owned(),
            city: pick(cities).to_owned(),
            status: status.to_owned(),
            tags: Json(tags),
            items,
            total: items * (25..900).fake::<i64>() * 1_000,
            discount: f64::from((0..250).fake::<u32>()) / 10.0,
            ordered_on: start + Duration::days((0..270).fake::<i64>()),
            paid: status != "new" && status != "cancelled",
            trend: Json((0..7).map(|_| (0..30).fake::<i64>()).collect()),
            fulfilled: match status {
                "shipped" => 100,
                "paid" => (20..100).fake(),
                _ => 0,
            },
            created_by: pick(&STAFF).to_owned(),
            updated_by: pick(&STAFF).to_owned(),
            ..Default::default()
        }
    }
}

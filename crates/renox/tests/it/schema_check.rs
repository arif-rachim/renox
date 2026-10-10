//! `#[derive(Model)]` describes its columns, `#[derive(DbEnum)]` counts as text.

use renox::db::{ColumnKind, Encrypted, Json, Model};
use renox::prelude::*;

#[derive(DbEnum, Debug, Clone, Copy, Default, PartialEq)]
enum Status {
    #[default]
    Open,
    Closed,
}

/// A local type stored as text, but with no `ColumnType`.
#[derive(Debug, Clone, Default, PartialEq)]
struct Custom(String);

impl std::str::FromStr for Custom {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Ok(Custom(s.to_string()))
    }
}

impl std::fmt::Display for Custom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl renox::db::ToDbValue for Custom {
    fn to_db_value(&self) -> renox::db::DbValue {
        renox::db::DbValue::Text(self.0.clone())
    }
}

renox::__db_text_type!(Custom);

#[derive(Model, Debug, Clone, Default)]
#[model(table = "gadgets")]
struct Gadget {
    id: i64,
    name: String,
    price: Option<i64>,
    active: bool,
    born: Option<renox::chrono::NaiveDate>,
    at: DateTime,
    tags: Json<Vec<String>>,
    secret: Encrypted<String>,
    status: Status,
    custom: Custom,
    #[allow(dead_code)]
    #[model(skip)]
    scratch: String,
}

#[test]
fn the_derive_describes_every_column() {
    let columns = Gadget::column_info();
    let names: Vec<_> = columns.iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            "id", "name", "price", "active", "born", "at", "tags", "secret", "status", "custom"
        ]
    );
    let expect = [
        ("id", ColumnKind::BigInt, false, "i64"),
        ("name", ColumnKind::Text, false, "String"),
        ("price", ColumnKind::BigInt, true, "i64"),
        ("active", ColumnKind::Bool, false, "bool"),
        ("born", ColumnKind::Date, true, "NaiveDate"),
        ("at", ColumnKind::DateTime, false, "DateTime"),
        ("tags", ColumnKind::Json, false, "Json"),
        ("secret", ColumnKind::Text, false, "Encrypted"),
        ("status", ColumnKind::Text, false, "Status"),
        ("custom", ColumnKind::Unknown, false, "Custom"),
    ];
    for (c, (name, kind, nullable, ty)) in columns.iter().zip(expect) {
        assert_eq!(c.name, name);
        assert_eq!(c.kind, kind, "{name}");
        assert_eq!(c.nullable, nullable, "{name}");
        assert!(c.rust_type.contains(ty), "{name}: {}", c.rust_type);
    }
}

// `index(user_id)` twice in one list is what clippy flags; the columns differ.
#[allow(clippy::duplicated_attributes)]
#[derive(Model, Debug, Clone, Default)]
#[model(
    table = "posts",
    index(user_id),
    unique(slug),
    index(user_id, created_at)
)]
struct Post {
    id: i64,
    #[model(references = "users")]
    user_id: i64,
    slug: String,
    #[model(default = "0")]
    views: i64,
    created_at: DateTime,
}

#[test]
fn indexes_defaults_and_references_round_trip() {
    let indexes = Post::indexes();
    let got: Vec<_> = indexes.iter().map(|i| (i.columns, i.unique)).collect();
    assert_eq!(
        got,
        [
            (&["user_id"][..], false),
            (&["slug"][..], true),
            (&["user_id", "created_at"][..], false),
        ]
    );
    let columns = Post::column_info();
    let col = |n: &str| columns.iter().find(|c| c.name == n).unwrap().clone();
    assert_eq!(col("user_id").references, Some("users"));
    assert_eq!(col("user_id").default, None);
    assert_eq!(col("views").default, Some("0"));
    assert_eq!(col("views").references, None);
    assert!(Gadget::indexes().is_empty());
}

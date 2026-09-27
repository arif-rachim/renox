use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

/// A value bound to a query parameter.
///
/// SQLite stores booleans as `0`/`1` and dates as text; PostgreSQL gets them
/// with their own types (`BOOLEAN`, `TIMESTAMPTZ`, `TIMESTAMP`, `DATE`,
/// `TIME`), so the same model works on both.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DbValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
    Bool(bool),
    /// A point in time with its offset (`TIMESTAMPTZ` on PostgreSQL).
    DateTime(chrono::DateTime<FixedOffset>),
    /// A date and time without a zone (`TIMESTAMP` on PostgreSQL).
    NaiveDateTime(NaiveDateTime),
    Date(NaiveDate),
    Time(NaiveTime),
    /// Text on SQLite; JSON on PostgreSQL (fits `JSONB` and `TEXT` columns).
    Json(serde_json::Value),
    /// A 16-byte BLOB on SQLite; `UUID` on PostgreSQL.
    #[cfg(feature = "uuid")]
    Uuid(uuid::Uuid),
}

impl DbValue {
    /// The value as JSON, as templates and APIs would see it.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        use serde_json::Value;
        match self {
            DbValue::Null => Value::Null,
            DbValue::Integer(v) => Value::from(*v),
            DbValue::Real(v) => Value::from(*v),
            DbValue::Text(v) => Value::from(v.clone()),
            DbValue::Blob(v) => Value::from(v.clone()),
            DbValue::Bool(v) => Value::from(*v),
            DbValue::DateTime(v) => Value::from(v.to_rfc3339()),
            DbValue::NaiveDateTime(v) => Value::from(v.format("%Y-%m-%dT%H:%M:%S").to_string()),
            DbValue::Date(v) => Value::from(v.to_string()),
            DbValue::Time(v) => Value::from(v.to_string()),
            DbValue::Json(v) => v.clone(),
            #[cfg(feature = "uuid")]
            DbValue::Uuid(v) => Value::from(v.to_string()),
        }
    }

    /// The value as SQLite stores it: booleans as integers, dates in the text
    /// formats sqlx uses, so values written through models and through raw
    /// sqlx queries compare equal.
    pub(crate) fn for_sqlite(self) -> DbValue {
        match self {
            DbValue::Bool(v) => DbValue::Integer(i64::from(v)),
            DbValue::DateTime(v) => DbValue::Text(v.format("%F %T%.f%:z").to_string()),
            DbValue::NaiveDateTime(v) => DbValue::Text(v.format("%F %T%.f").to_string()),
            DbValue::Date(v) => DbValue::Text(v.format("%F").to_string()),
            DbValue::Time(v) => DbValue::Text(v.format("%T%.f").to_string()),
            DbValue::Json(v) => DbValue::Text(v.to_string()),
            #[cfg(feature = "uuid")]
            DbValue::Uuid(v) => DbValue::Blob(v.as_bytes().to_vec()),
            other => other,
        }
    }
}

/// Converts a Rust value into a query parameter. Implemented for the usual
/// scalar types, `Option`, chrono dates and `serde_json::Value`; implement it
/// for your own types to use them in models and `where_*` filters.
pub trait ToDbValue {
    fn to_db_value(&self) -> DbValue;
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl ToDbValue for $t {
            fn to_db_value(&self) -> DbValue {
                DbValue::Integer(i64::from(*self))
            }
        }
    )*};
}

integer!(i8, i16, i32, i64, u8, u16, u32);

impl ToDbValue for bool {
    fn to_db_value(&self) -> DbValue {
        DbValue::Bool(*self)
    }
}

impl ToDbValue for f32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Real(f64::from(*self))
    }
}

impl ToDbValue for f64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Real(*self)
    }
}

impl ToDbValue for str {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.to_owned())
    }
}

impl ToDbValue for String {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.clone())
    }
}

impl ToDbValue for Vec<u8> {
    fn to_db_value(&self) -> DbValue {
        DbValue::Blob(self.clone())
    }
}

impl ToDbValue for serde_json::Value {
    fn to_db_value(&self) -> DbValue {
        DbValue::Json(self.clone())
    }
}

#[cfg(feature = "uuid")]
impl ToDbValue for uuid::Uuid {
    fn to_db_value(&self) -> DbValue {
        DbValue::Uuid(*self)
    }
}

impl<Tz: TimeZone> ToDbValue for chrono::DateTime<Tz> {
    fn to_db_value(&self) -> DbValue {
        DbValue::DateTime(self.fixed_offset())
    }
}

impl ToDbValue for NaiveDateTime {
    fn to_db_value(&self) -> DbValue {
        DbValue::NaiveDateTime(*self)
    }
}

impl ToDbValue for NaiveDate {
    fn to_db_value(&self) -> DbValue {
        DbValue::Date(*self)
    }
}

impl ToDbValue for NaiveTime {
    fn to_db_value(&self) -> DbValue {
        DbValue::Time(*self)
    }
}

impl<T: ToDbValue> ToDbValue for Option<T> {
    fn to_db_value(&self) -> DbValue {
        self.as_ref().map_or(DbValue::Null, ToDbValue::to_db_value)
    }
}

impl<T: ToDbValue + ?Sized> ToDbValue for &T {
    fn to_db_value(&self) -> DbValue {
        (**self).to_db_value()
    }
}

impl ToDbValue for DbValue {
    fn to_db_value(&self) -> DbValue {
        self.clone()
    }
}

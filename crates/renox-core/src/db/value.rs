use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

/// A value bound to a query parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
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

integer!(i8, i16, i32, i64, u8, u16, u32, bool);

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
        DbValue::Text(self.to_string())
    }
}

// Same text formats sqlx uses when binding chrono types, so values written
// through models and through raw queries compare equal.
impl<Tz: TimeZone> ToDbValue for chrono::DateTime<Tz>
where
    Tz::Offset: std::fmt::Display,
{
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.format("%F %T%.f%:z").to_string())
    }
}

impl ToDbValue for NaiveDateTime {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.format("%F %T%.f").to_string())
    }
}

impl ToDbValue for NaiveDate {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.format("%F").to_string())
    }
}

impl ToDbValue for NaiveTime {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.format("%T%.f").to_string())
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

use crate::db::{DbValue, ToDbValue};

/// What a rule sees of a field's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Inspected {
    /// `None`, or text that is empty after trimming.
    Missing,
    Text(String),
    Number(f64),
    Bool(bool),
    Items(usize),
}

/// A value that can be validated. Implemented for strings, numbers, `bool`,
/// `Option<T>` and `Vec<T>`.
pub trait FieldValue {
    fn inspect(&self) -> Inspected;
    /// The value as a query parameter, for `unique` and `exists`.
    fn db_value(&self) -> DbValue;
}

impl FieldValue for str {
    fn inspect(&self) -> Inspected {
        if self.trim().is_empty() {
            Inspected::Missing
        } else {
            Inspected::Text(self.to_owned())
        }
    }

    fn db_value(&self) -> DbValue {
        self.to_db_value()
    }
}

impl FieldValue for String {
    fn inspect(&self) -> Inspected {
        self.as_str().inspect()
    }

    fn db_value(&self) -> DbValue {
        self.to_db_value()
    }
}

macro_rules! number {
    ($($t:ty),*) => {$(
        impl FieldValue for $t {
            fn inspect(&self) -> Inspected {
                Inspected::Number(*self as f64)
            }

            fn db_value(&self) -> DbValue {
                self.to_db_value()
            }
        }
    )*};
}

number!(i8, i16, i32, i64, u8, u16, u32, f32, f64);

impl FieldValue for bool {
    fn inspect(&self) -> Inspected {
        Inspected::Bool(*self)
    }

    fn db_value(&self) -> DbValue {
        self.to_db_value()
    }
}

impl<T: FieldValue> FieldValue for Option<T> {
    fn inspect(&self) -> Inspected {
        self.as_ref()
            .map_or(Inspected::Missing, FieldValue::inspect)
    }

    fn db_value(&self) -> DbValue {
        self.as_ref().map_or(DbValue::Null, FieldValue::db_value)
    }
}

impl<T> FieldValue for Vec<T> {
    fn inspect(&self) -> Inspected {
        if self.is_empty() {
            Inspected::Missing
        } else {
            Inspected::Items(self.len())
        }
    }

    fn db_value(&self) -> DbValue {
        DbValue::Null
    }
}

impl<T: FieldValue + ?Sized> FieldValue for &T {
    fn inspect(&self) -> Inspected {
        (**self).inspect()
    }

    fn db_value(&self) -> DbValue {
        (**self).db_value()
    }
}

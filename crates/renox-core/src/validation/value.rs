use crate::db::{DbValue, ToDbValue};

/// What a rule sees of a field's value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Inspected {
    /// `None`, or text that is empty after trimming.
    Missing,
    Text(String),
    Number(f64),
    Bool(bool),
    Items(usize),
    /// An uploaded file: size in kilobytes, the extension its content (or
    /// else its name) indicates, and whether it really is an image.
    File {
        kilobytes: f64,
        extension: String,
        image: bool,
    },
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

impl FieldValue for crate::upload::Upload {
    fn inspect(&self) -> Inspected {
        // An empty file that was chosen is a file with no type: `image()`
        // and `mimes()` refuse it. (No file chosen never gets here.)
        if self.bytes.is_empty() {
            return Inspected::File {
                kilobytes: 0.0,
                extension: String::new(),
                image: false,
            };
        }
        let extension = match self.sniffed_type() {
            Some("image/png") => "png".to_owned(),
            Some("image/jpeg") => "jpg".to_owned(),
            Some("image/gif") => "gif".to_owned(),
            Some("image/webp") => "webp".to_owned(),
            Some("application/pdf") => "pdf".to_owned(),
            _ => self.extension().unwrap_or_default(),
        };
        Inspected::File {
            kilobytes: self.size() as f64 / 1024.0,
            extension,
            image: self.is_image(),
        }
    }

    fn db_value(&self) -> DbValue {
        DbValue::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upload::Upload;
    use crate::validation::{Locale, Validator};

    #[test]
    fn a_text_file_named_png_is_not_an_image() {
        let fake = Some(Upload {
            file_name: "palsu.png".into(),
            content_type: "image/png".into(),
            bytes: axum::body::Bytes::from_static(b"ini bukan gambar\n"),
        });
        assert!(matches!(
            FieldValue::inspect(&fake),
            Inspected::File { image: false, .. }
        ));
        let mut v = Validator::new(Locale::Id);
        v.field("photo", &fake).label("foto").image().max(2048);
        assert_eq!(v.errors.first("photo"), Some("Foto harus berupa gambar."));
    }
}

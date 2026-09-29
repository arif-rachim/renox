/// Languages with built-in validation messages, from `APP_LOCALE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Locale {
    #[default]
    En,
    Id,
}

impl Locale {
    /// `en` or `id`; anything else falls back to English.
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "id" | "id-id" | "id_id" => Self::Id,
            _ => Self::En,
        }
    }
}

/// The message template for a rule. `:attribute` is the field's label,
/// `:Attribute` the same with a capital first letter, and `:min`, `:max`,
/// `:other` fill in rule arguments.
pub(crate) fn template(locale: Locale, key: &str) -> &'static str {
    match locale {
        Locale::En => match key {
            "required" => "The :attribute field is required.",
            "min.string" => "The :attribute must be at least :min characters.",
            "min.numeric" => "The :attribute must be at least :min.",
            "min.array" => "The :attribute must have at least :min items.",
            "max.string" => "The :attribute may not be longer than :max characters.",
            "max.numeric" => "The :attribute may not be greater than :max.",
            "max.array" => "The :attribute may not have more than :max items.",
            "between.string" => "The :attribute must be between :min and :max characters.",
            "between.numeric" => "The :attribute must be between :min and :max.",
            "between.array" => "The :attribute must have between :min and :max items.",
            "min.file" => "The :attribute must be at least :min kilobytes.",
            "max.file" => "The :attribute may not be greater than :max kilobytes.",
            "between.file" => "The :attribute must be between :min and :max kilobytes.",
            "file" => "The :attribute must be a file.",
            "image" => "The :attribute must be an image.",
            "mimes" => "The :attribute must be a file of type: :values.",
            "email" => "The :attribute must be a valid email address.",
            "url" => "The :attribute must be a valid URL.",
            "in" => "The selected :attribute is invalid.",
            "confirmed" => "The :attribute confirmation does not match.",
            "password.letters" => "The :attribute must contain at least one letter.",
            "password.mixed" => {
                "The :attribute must contain at least one uppercase and one lowercase letter."
            }
            "password.numbers" => "The :attribute must contain at least one number.",
            "password.symbols" => "The :attribute must contain at least one symbol.",
            "current_password" => "The :attribute is incorrect.",
            "accepted" => "The :attribute must be accepted.",
            "unique" => "The :attribute has already been taken.",
            "exists" => "The selected :attribute is invalid.",
            "numeric" => "The :attribute must be a number.",
            "regex" => "The :attribute format is invalid.",
            "digits" => "The :attribute must be :digits digits.",
            "digits_between" => "The :attribute must be between :min and :max digits.",
            "date" => "The :attribute is not a valid date.",
            "before" => "The :attribute must be a date before :date.",
            "before_or_equal" => "The :attribute must be a date before or equal to :date.",
            "after" => "The :attribute must be a date after :date.",
            "after_or_equal" => "The :attribute must be a date after or equal to :date.",
            "not_in" => "The selected :attribute is invalid.",
            "same" => "The :attribute and :other must match.",
            "different" => "The :attribute and :other must be different.",
            "alpha" => "The :attribute may only contain letters.",
            "alpha_num" => "The :attribute may only contain letters and numbers.",
            "alpha_dash" => {
                "The :attribute may only contain letters, numbers, dashes and underscores."
            }
            "lowercase" => "The :attribute must be lowercase.",
            "uppercase" => "The :attribute must be uppercase.",
            "starts_with" => "The :attribute must start with one of: :values.",
            "ends_with" => "The :attribute must end with one of: :values.",
            "uuid" => "The :attribute must be a valid UUID.",
            "ip" => "The :attribute must be a valid IP address.",
            "size.string" => "The :attribute must be :size characters.",
            "size.numeric" => "The :attribute must be :size.",
            "size.array" => "The :attribute must contain :size items.",
            "size.file" => "The :attribute must be :size kilobytes.",
            "prohibited" => "The :attribute field must be empty here.",
            "distinct" => "The :attribute is a duplicate.",
            "auth.failed" => "These credentials do not match our records.",
            "auth.throttle" => "Too many login attempts. Please try again in :seconds seconds.",
            _ => "The :attribute is invalid.",
        },
        Locale::Id => match key {
            "required" => ":Attribute wajib diisi.",
            "min.string" => ":Attribute minimal :min karakter.",
            "min.numeric" => ":Attribute minimal :min.",
            "min.array" => ":Attribute minimal berisi :min item.",
            "max.string" => ":Attribute maksimal :max karakter.",
            "max.numeric" => ":Attribute maksimal :max.",
            "max.array" => ":Attribute maksimal berisi :max item.",
            "between.string" => ":Attribute harus antara :min sampai :max karakter.",
            "between.numeric" => ":Attribute harus antara :min sampai :max.",
            "between.array" => ":Attribute harus berisi :min sampai :max item.",
            "min.file" => ":Attribute minimal :min kilobyte.",
            "max.file" => ":Attribute maksimal :max kilobyte.",
            "between.file" => ":Attribute harus antara :min sampai :max kilobyte.",
            "file" => ":Attribute harus berupa file.",
            "image" => ":Attribute harus berupa gambar.",
            "mimes" => ":Attribute harus berupa file bertipe: :values.",
            "email" => ":Attribute harus berupa alamat email yang valid.",
            "url" => ":Attribute harus berupa URL yang valid.",
            "in" => ":Attribute yang dipilih tidak valid.",
            "confirmed" => "Konfirmasi :attribute tidak cocok.",
            "password.letters" => ":Attribute harus berisi setidaknya satu huruf.",
            "password.mixed" => {
                ":Attribute harus berisi setidaknya satu huruf besar dan satu huruf kecil."
            }
            "password.numbers" => ":Attribute harus berisi setidaknya satu angka.",
            "password.symbols" => ":Attribute harus berisi setidaknya satu simbol.",
            "current_password" => ":Attribute salah.",
            "accepted" => ":Attribute harus disetujui.",
            "unique" => ":Attribute sudah digunakan.",
            "exists" => ":Attribute yang dipilih tidak valid.",
            "numeric" => ":Attribute harus berupa angka.",
            "regex" => "Format :attribute tidak valid.",
            "digits" => ":Attribute harus :digits digit.",
            "digits_between" => ":Attribute harus antara :min sampai :max digit.",
            "date" => ":Attribute bukan tanggal yang valid.",
            "before" => ":Attribute harus tanggal sebelum :date.",
            "before_or_equal" => ":Attribute harus tanggal sebelum atau sama dengan :date.",
            "after" => ":Attribute harus tanggal setelah :date.",
            "after_or_equal" => ":Attribute harus tanggal setelah atau sama dengan :date.",
            "not_in" => ":Attribute yang dipilih tidak valid.",
            "same" => ":Attribute dan :other harus sama.",
            "different" => ":Attribute dan :other harus berbeda.",
            "alpha" => ":Attribute hanya boleh berisi huruf.",
            "alpha_num" => ":Attribute hanya boleh berisi huruf dan angka.",
            "alpha_dash" => {
                ":Attribute hanya boleh berisi huruf, angka, tanda hubung dan garis bawah."
            }
            "lowercase" => ":Attribute harus huruf kecil.",
            "uppercase" => ":Attribute harus huruf besar.",
            "starts_with" => ":Attribute harus diawali salah satu dari: :values.",
            "ends_with" => ":Attribute harus diakhiri salah satu dari: :values.",
            "uuid" => ":Attribute harus UUID yang valid.",
            "ip" => ":Attribute harus alamat IP yang valid.",
            "size.string" => ":Attribute harus :size karakter.",
            "size.numeric" => ":Attribute harus :size.",
            "size.array" => ":Attribute harus berisi :size item.",
            "size.file" => ":Attribute harus :size kilobyte.",
            "prohibited" => ":Attribute harus dikosongkan di sini.",
            "distinct" => ":Attribute sudah ada sebelumnya.",
            "auth.failed" => "Email atau kata sandi salah.",
            "auth.throttle" => "Terlalu banyak percobaan masuk. Coba lagi dalam :seconds detik.",
            _ => ":Attribute tidak valid.",
        },
    }
}

/// The app's translation of a built-in message (`renox.validation.{key}` in
/// its lang file), or the built-in template.
pub(crate) fn template_for(
    locale: Locale,
    texts: Option<&crate::i18n::Texts>,
    key: &str,
) -> std::borrow::Cow<'static, str> {
    match texts.and_then(|t| t.get(&format!("renox.validation.{key}"))) {
        Some(text) => std::borrow::Cow::Owned(text.clone()),
        None => std::borrow::Cow::Borrowed(template(locale, key)),
    }
}

/// Fills a template's placeholders.
pub(crate) fn render(template: &str, label: &str, params: &[(&str, String)]) -> String {
    let mut capitalized = label.to_owned();
    if let Some(first) = capitalized.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    let mut out = template
        .replace(":Attribute", &capitalized)
        .replace(":attribute", label);
    for (name, value) in params {
        out = out.replace(&format!(":{name}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_both_languages() {
        let params = [("min", "3".to_owned())];
        assert_eq!(
            render(template(Locale::En, "min.string"), "nama", &params),
            "The nama must be at least 3 characters."
        );
        assert_eq!(
            render(template(Locale::Id, "min.string"), "nama", &params),
            "Nama minimal 3 karakter."
        );
        assert_eq!(Locale::parse("ID"), Locale::Id);
        assert_eq!(Locale::parse("fr"), Locale::En);
    }

    #[test]
    fn every_rule_has_its_own_message_in_both_languages() {
        for key in [
            "regex",
            "digits",
            "digits_between",
            "date",
            "before",
            "before_or_equal",
            "after",
            "after_or_equal",
            "not_in",
            "same",
            "different",
            "alpha",
            "alpha_num",
            "alpha_dash",
            "lowercase",
            "uppercase",
            "starts_with",
            "ends_with",
            "uuid",
            "ip",
            "size.string",
            "size.numeric",
            "size.array",
            "size.file",
            "prohibited",
            "distinct",
        ] {
            assert_ne!(
                template(Locale::En, key),
                "The :attribute is invalid.",
                "{key}"
            );
            assert_ne!(
                template(Locale::Id, key),
                ":Attribute tidak valid.",
                "{key}"
            );
        }
    }
}

pub const MARKETPLACE_SELLER_ADMIN_I18N_PREFIX: &str = "marketplaceSeller";

pub const SUPPORTED_LOCALES: &[&str] = &["en", "ru"];

pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    match locale {
        Some(value) if value.eq_ignore_ascii_case("ru") || value.starts_with("ru-") => "ru",
        _ => "en",
    }
}

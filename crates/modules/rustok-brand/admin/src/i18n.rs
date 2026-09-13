pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    match locale {
        Some(value) if value.eq_ignore_ascii_case("ru") || value.starts_with("ru-") => "ru",
        _ => "en",
    }
}

pub fn calculate_reading_time(plain_text: &str) -> u32 {
    let word_count = plain_text.split_whitespace().count();
    let minutes = word_count.div_ceil(200);
    (minutes as u32).max(1)
}

/// Format through the canonical catalog, including locale-specific plural rules.
pub fn format_reading_time(minutes: u32, locale: Option<&str>) -> String {
    let mut args = rustok_ui_i18n::FluentArgs::new();
    args.set("minutes", minutes as f64);
    crate::i18n::format(locale, "blog.readingTime", Some(&args), "")
}

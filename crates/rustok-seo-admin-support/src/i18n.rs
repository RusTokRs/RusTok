use crate::locale::locale_primary_language;
use crate::model::SeoRecommendation;

pub(crate) fn tr(locale: Option<&str>, en: &str, ru: &str) -> String {
    if is_russian(locale) {
        ru.to_string()
    } else {
        en.to_string()
    }
}

pub(crate) fn recommendation(locale: Option<&str>, item: &SeoRecommendation) -> String {
    match item {
        SeoRecommendation::AdjustTitleLength => tr(
            locale,
            "Adjust title length toward the 10-60 character range.",
            "Подведите длину заголовка к диапазону 10-60 символов.",
        ),
        SeoRecommendation::AddSeoTitle => tr(
            locale,
            "Add an explicit SEO title.",
            "Добавьте явный SEO-заголовок.",
        ),
        SeoRecommendation::AdjustDescriptionLength => tr(
            locale,
            "Adjust description length toward the 50-160 character range.",
            "Подведите длину описания к диапазону 50-160 символов.",
        ),
        SeoRecommendation::AddMetaDescription => tr(
            locale,
            "Add a meta description.",
            "Добавьте meta description.",
        ),
        SeoRecommendation::SetCanonicalUrl => tr(
            locale,
            "Set an explicit canonical URL when the route needs one.",
            "Задайте явный canonical URL, если он нужен маршруту.",
        ),
        SeoRecommendation::AddOpenGraphTitle => tr(
            locale,
            "Add an Open Graph title for social previews.",
            "Добавьте Open Graph title для социальных превью.",
        ),
        SeoRecommendation::AddOpenGraphDescription => tr(
            locale,
            "Add an Open Graph description.",
            "Добавьте Open Graph description.",
        ),
        SeoRecommendation::AddOpenGraphImage => tr(
            locale,
            "Provide an Open Graph image.",
            "Укажите Open Graph image.",
        ),
    }
}

pub(crate) fn source_label(locale: Option<&str>, source: &str) -> String {
    match source {
        "explicit" => tr(locale, "Explicit SEO", "Явное SEO"),
        "generated" => tr(locale, "Generated template SEO", "Шаблонное SEO"),
        "page_fallback" => tr(locale, "Page fallback", "Фоллбек страницы"),
        "product_fallback" => tr(locale, "Product fallback", "Фоллбек товара"),
        "blog_fallback" => tr(locale, "Blog fallback", "Фоллбек блога"),
        "forum_category_fallback" => tr(
            locale,
            "Forum category fallback",
            "Фоллбек категории форума",
        ),
        "forum_topic_fallback" => tr(locale, "Forum topic fallback", "Фоллбек темы форума"),
        other => other.to_string(),
    }
}

pub(crate) fn recommendations_count_label(locale: Option<&str>, count: usize) -> String {
    if is_russian(locale) {
        format!("{count} рекомендаций")
    } else {
        format!("{count} recommendations")
    }
}

pub(crate) fn working_label(locale: Option<&str>, busy_key: &str) -> String {
    if is_russian(locale) {
        format!("Выполняется: {busy_key}")
    } else {
        format!("Working: {busy_key}")
    }
}

pub(crate) fn validation_error(locale: Option<&str>, err: &str) -> String {
    match err {
        "Target id is required." => tr(
            locale,
            "Target id is required.",
            "Идентификатор сущности обязателен.",
        ),
        "Invalid target id." => tr(
            locale,
            "Invalid target id.",
            "Некорректный идентификатор сущности.",
        ),
        "Host locale is required." => tr(
            locale,
            "Host locale is required.",
            "Нужна локаль от host-приложения.",
        ),
        "Invalid host locale." => tr(
            locale,
            "Invalid host locale.",
            "Некорректная локаль от host-приложения.",
        ),
        other => other.to_string(),
    }
}

fn is_russian(locale: Option<&str>) -> bool {
    matches!(
        locale.and_then(locale_primary_language).as_deref(),
        Some("ru")
    )
}

#[cfg(test)]
mod tests {
    use super::{recommendations_count_label, source_label, tr, validation_error, working_label};

    #[test]
    fn tr_uses_primary_language_for_russian_locales() {
        assert_eq!(tr(Some("ru-RU"), "SEO", "СЕО"), "СЕО");
        assert_eq!(tr(Some("en-US"), "SEO", "СЕО"), "SEO");
    }

    #[test]
    fn source_label_localizes_known_values() {
        assert_eq!(source_label(Some("ru"), "explicit"), "Явное SEO");
        assert_eq!(
            source_label(Some("en"), "forum_topic_fallback"),
            "Forum topic fallback"
        );
    }

    #[test]
    fn helper_labels_follow_host_locale() {
        assert_eq!(recommendations_count_label(Some("ru"), 3), "3 рекомендаций");
        assert_eq!(working_label(Some("en"), "save"), "Working: save");
        assert_eq!(
            validation_error(Some("ru"), "Host locale is required."),
            "Нужна локаль от host-приложения."
        );
    }
}

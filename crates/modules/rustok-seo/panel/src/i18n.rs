use crate::model::{
    SeoControlPlaneWidgetStateKind, SeoEventDeliveryStatus, SeoRecommendation, SeoRemediationAction,
};
use rustok_api::locale_primary_language;

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

pub(crate) fn delivery_status_label(
    locale: Option<&str>,
    status: SeoEventDeliveryStatus,
) -> String {
    match status {
        SeoEventDeliveryStatus::Pending => tr(locale, "Pending", "В очереди"),
        SeoEventDeliveryStatus::Sent => tr(locale, "Sent", "Отправлено"),
        SeoEventDeliveryStatus::Retry => tr(locale, "Retry", "Повтор"),
        SeoEventDeliveryStatus::Failed => tr(locale, "Failed", "Ошибка"),
        SeoEventDeliveryStatus::DeadLetter => tr(locale, "Dead letter", "Dead letter"),
    }
}

pub(crate) fn control_plane_state_title(
    locale: Option<&str>,
    kind: SeoControlPlaneWidgetStateKind,
) -> String {
    match kind {
        SeoControlPlaneWidgetStateKind::Loading => {
            tr(locale, "Control-plane status", "Статус control-plane")
        }
        SeoControlPlaneWidgetStateKind::Ready => {
            tr(locale, "Control-plane status", "Статус control-plane")
        }
        SeoControlPlaneWidgetStateKind::Empty => {
            tr(locale, "No control-plane data", "Нет данных control-plane")
        }
        SeoControlPlaneWidgetStateKind::PermissionDenied => {
            tr(locale, "Permission required", "Нужны права доступа")
        }
        SeoControlPlaneWidgetStateKind::Error => tr(locale, "Widget error", "Ошибка виджета"),
    }
}

pub(crate) fn control_plane_state_body(
    locale: Option<&str>,
    kind: SeoControlPlaneWidgetStateKind,
) -> String {
    match kind {
        SeoControlPlaneWidgetStateKind::Loading => tr(
            locale,
            "Loading SEO control-plane data.",
            "Загрузка данных SEO control-plane.",
        ),
        SeoControlPlaneWidgetStateKind::Ready => tr(
            locale,
            "Delivery and remediation widgets are ready.",
            "Виджеты delivery/remediation готовы.",
        ),
        SeoControlPlaneWidgetStateKind::Empty => tr(
            locale,
            "Select an entity to see SEO control-plane widgets.",
            "Выберите сущность, чтобы увидеть SEO control-plane виджеты.",
        ),
        SeoControlPlaneWidgetStateKind::PermissionDenied => tr(
            locale,
            "You do not have access to SEO control-plane widgets for this tenant.",
            "У вас нет доступа к SEO control-plane виджетам для этого tenant.",
        ),
        SeoControlPlaneWidgetStateKind::Error => tr(
            locale,
            "Control-plane widgets are temporarily unavailable.",
            "Control-plane виджеты временно недоступны.",
        ),
    }
}

pub(crate) fn remediation_empty_label(locale: Option<&str>) -> String {
    tr(
        locale,
        "No remediation issue selected.",
        "Issue для remediation не выбран.",
    )
}

pub(crate) fn remediation_action_label(
    locale: Option<&str>,
    action: SeoRemediationAction,
) -> String {
    match action {
        SeoRemediationAction::OpenEntityEditor => {
            tr(locale, "Open entity editor", "Открыть редактор сущности")
        }
        SeoRemediationAction::OpenBulkJob => {
            tr(locale, "Open bulk remediation", "Открыть bulk remediation")
        }
        SeoRemediationAction::RunReindex => tr(locale, "Run reindex", "Запустить переиндексацию"),
    }
}

pub(crate) fn remediation_reason(locale: Option<&str>, reason_key: &str) -> String {
    match reason_key {
        "bulk_consistency_fix" => tr(
            locale,
            "Issue is usually resolved via coordinated bulk updates.",
            "Проблема обычно исправляется согласованным bulk-обновлением.",
        ),
        "index_sync_required" => tr(
            locale,
            "Index state is out-of-sync and needs replay/reindex.",
            "Состояние индекса рассинхронизировано и требует replay/reindex.",
        ),
        _ => tr(
            locale,
            "Resolve metadata directly in the owning entity editor.",
            "Исправьте метаданные напрямую в редакторе сущности.",
        ),
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
    use super::{
        control_plane_state_body, control_plane_state_title, delivery_status_label,
        recommendations_count_label, remediation_action_label, remediation_empty_label,
        remediation_reason, source_label, tr, validation_error, working_label,
    };
    use crate::model::{
        SeoControlPlaneWidgetStateKind, SeoEventDeliveryStatus, SeoRemediationAction,
    };

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

    #[test]
    fn delivery_and_remediation_labels_are_localized() {
        assert_eq!(
            delivery_status_label(Some("ru"), SeoEventDeliveryStatus::Retry),
            "Повтор"
        );
        assert_eq!(
            delivery_status_label(Some("en"), SeoEventDeliveryStatus::DeadLetter),
            "Dead letter"
        );
        assert_eq!(
            remediation_action_label(Some("en"), SeoRemediationAction::RunReindex),
            "Run reindex"
        );
        assert_eq!(
            remediation_reason(Some("en"), "index_sync_required"),
            "Index state is out-of-sync and needs replay/reindex."
        );
        assert_eq!(
            remediation_empty_label(Some("ru")),
            "Issue для remediation не выбран."
        );
    }

    #[test]
    fn control_plane_state_labels_are_localized() {
        assert_eq!(
            control_plane_state_title(Some("ru"), SeoControlPlaneWidgetStateKind::PermissionDenied),
            "Нужны права доступа"
        );
        assert_eq!(
            control_plane_state_body(Some("en"), SeoControlPlaneWidgetStateKind::Loading),
            "Loading SEO control-plane data."
        );
    }
}

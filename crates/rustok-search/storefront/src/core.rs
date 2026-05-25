pub fn parse_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRouteFilters {
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
}

pub fn parse_search_route_filters(
    entity_types: Option<&str>,
    source_modules: Option<&str>,
    statuses: Option<&str>,
) -> SearchRouteFilters {
    SearchRouteFilters {
        entity_types: parse_csv(entity_types.unwrap_or_default()),
        source_modules: parse_csv(source_modules.unwrap_or_default()),
        statuses: parse_csv(statuses.unwrap_or_default()),
    }
}

pub fn facet_display_name(raw_name: &str) -> String {
    raw_name.replace('_', " ")
}

pub fn facet_bucket_label(value: &str, count: u64) -> String {
    format!("{} ({})", value, count)
}

pub fn snippet_or_fallback(snippet: Option<String>, fallback: &str) -> String {
    snippet.unwrap_or_else(|| fallback.to_string())
}

pub fn score_label(score: f64) -> String {
    format!("score {:.3}", score)
}

pub fn score_value(score: f64) -> String {
    format!("{:.3}", score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csv_trims_and_skips_empty_segments() {
        assert_eq!(
            parse_csv(" products, blog ,, pages "),
            vec![
                "products".to_string(),
                "blog".to_string(),
                "pages".to_string()
            ]
        );
    }

    #[test]
    fn optional_text_returns_none_for_blank() {
        assert_eq!(
            optional_text(
                "   
	"
            ),
            None
        );
    }

    #[test]
    fn optional_text_returns_trimmed_value() {
        assert_eq!(optional_text("  abc  "), Some("abc".to_string()));
    }

    #[test]
    fn formatting_helpers_are_stable() {
        assert_eq!(facet_display_name("source_module"), "source module");
        assert_eq!(facet_bucket_label("product", 42), "product (42)");
        assert_eq!(score_label(0.12345), "score 0.123");
        assert_eq!(score_value(0.12345), "0.123");
        assert_eq!(
            snippet_or_fallback(None, "fallback"),
            "fallback".to_string()
        );
        assert_eq!(
            error_with_context("load failed", "timeout"),
            "load failed: timeout"
        );
    }

    #[test]
    fn parse_search_route_filters_handles_missing_and_csv_values() {
        let parsed = parse_search_route_filters(
            Some(" product , pages "),
            None,
            Some(" published, draft ,"),
        );

        assert_eq!(
            parsed,
            SearchRouteFilters {
                entity_types: vec!["product".to_string(), "pages".to_string()],
                source_modules: Vec::new(),
                statuses: vec!["published".to_string(), "draft".to_string()],
            }
        );
    }

    #[test]
    fn query_normalization_helpers_are_consistent() {
        assert_eq!(normalized_search_query("   "), None);
        assert_eq!(
            normalized_search_query("  phone  "),
            Some("phone".to_string())
        );
        assert_eq!(suggestion_query(" a ", 2), None);
        assert_eq!(suggestion_query("  rustok ", 2), Some("rustok".to_string()));
        assert_eq!(
            suggestion_kind_with_locale("document", Some("ru")),
            "document • ru".to_string()
        );
        assert_eq!(
            suggestion_kind_with_locale("query", None),
            "query".to_string()
        );
        assert_eq!(locale_or_all(None), "all".to_string());
        assert_eq!(
            applied_preset_or_selected(Some("featured".to_string()), "", "none"),
            "featured".to_string()
        );
        assert_eq!(
            applied_preset_or_selected(None, "manual", "none"),
            "manual".to_string()
        );
        assert_eq!(
            applied_preset_or_selected(None, "", "none"),
            "none".to_string()
        );
        assert_eq!(
            render_results_summary(
                "{count} results in {took_ms}ms via {engine}/{ranking_profile}",
                10,
                34,
                "pg",
                "default",
            ),
            "10 results in 34ms via pg/default".to_string()
        );
        assert_eq!(
            render_locale_label("locale: {locale}", "ru"),
            "locale: ru".to_string()
        );
        assert_eq!(
            render_preset_label("preset: {preset}", "featured"),
            "preset: featured".to_string()
        );
        assert!(is_document_suggestion("document"));
        assert_eq!(
            suggestion_action_label("document", "Open", "Search"),
            "Open".to_string()
        );
        assert_eq!(
            suggestion_action_label("query", "Open", "Search"),
            "Search".to_string()
        );
        assert_eq!(next_preset_selection("featured", "featured"), "");
        assert_eq!(next_preset_selection("", "featured"), "featured");
    }
}

pub fn entity_source_label(entity_type: &str, source_module: &str) -> String {
    format!("{} | {}", entity_type, source_module)
}

pub fn source_entity_status_label(source_module: &str, entity_type: &str, status: &str) -> String {
    format!("{}/{} ({})", source_module, entity_type, status)
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{}: {}", context, error)
}

pub fn normalized_search_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn suggestion_query(query: &str, min_len: usize) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.len() < min_len {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn suggestion_kind_with_locale(kind: &str, locale: Option<&str>) -> String {
    locale
        .map(|locale| format!("{kind} • {locale}"))
        .unwrap_or_else(|| kind.to_string())
}

pub fn locale_or_all(locale: Option<String>) -> String {
    locale.unwrap_or_else(|| "all".to_string())
}

pub fn applied_preset_or_selected(
    applied_preset_key: Option<String>,
    selected_preset: &str,
    none_label: &str,
) -> String {
    applied_preset_key
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if selected_preset.is_empty() {
                none_label.to_string()
            } else {
                selected_preset.to_string()
            }
        })
}

pub fn render_results_summary(
    template: &str,
    count: u64,
    took_ms: u64,
    engine: &str,
    ranking_profile: &str,
) -> String {
    template
        .replace("{count}", count.to_string().as_str())
        .replace("{took_ms}", took_ms.to_string().as_str())
        .replace("{engine}", engine)
        .replace("{ranking_profile}", ranking_profile)
}

pub fn render_locale_label(template: &str, locale: &str) -> String {
    template.replace("{locale}", locale)
}

pub fn render_preset_label(template: &str, preset: &str) -> String {
    template.replace("{preset}", preset)
}

pub fn is_document_suggestion(kind: &str) -> bool {
    kind == "document"
}

pub fn suggestion_action_label(kind: &str, open_label: &str, search_label: &str) -> String {
    if is_document_suggestion(kind) {
        open_label.to_string()
    } else {
        search_label.to_string()
    }
}

pub fn next_preset_selection(current: &str, selected_key: &str) -> String {
    if current == selected_key {
        String::new()
    } else {
        selected_key.to_string()
    }
}


pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
}

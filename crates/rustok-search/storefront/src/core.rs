use crate::model::{SearchFacetGroup, SearchPreviewPayload};

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

    #[test]
    fn search_results_view_model_prepares_render_ready_fields() {
        let payload = SearchPreviewPayload {
            query_log_id: Some("log-1".to_string()),
            preset_key: None,
            items: vec![crate::model::SearchPreviewResultItem {
                id: "doc-1".to_string(),
                entity_type: "product".to_string(),
                source_module: "catalog".to_string(),
                title: "Boots".to_string(),
                snippet: None,
                score: 0.98765,
                locale: Some("ru".to_string()),
                url: Some("/products/boots".to_string()),
                payload: "{}".to_string(),
            }],
            total: 1,
            took_ms: 12,
            engine: "postgres".to_string(),
            ranking_profile: "balanced".to_string(),
            facets: vec![SearchFacetGroup {
                name: "entity_type".to_string(),
                buckets: vec![],
            }],
        };
        let labels = SearchResultsLabels {
            summary_template: "{count} results in {took_ms}ms via {engine}/{ranking_profile}"
                .to_string(),
            preset_template: "preset: {preset}".to_string(),
            none_label: "none".to_string(),
            locale_template: "locale: {locale}".to_string(),
            no_snippet: "No snippet returned.".to_string(),
        };

        let view_model = build_search_results_view_model(payload, "manual", &labels);

        assert_eq!(view_model.query_log_id, Some("log-1".to_string()));
        assert_eq!(
            view_model.summary,
            "1 results in 12ms via postgres/balanced"
        );
        assert_eq!(view_model.preset, "preset: manual");
        assert_eq!(view_model.locale, "locale: ru");
        assert!(view_model.has_items);
        assert_eq!(view_model.items[0].id, "doc-1");
        assert_eq!(view_model.items[0].source_label, "product | catalog");
        assert_eq!(view_model.items[0].score_label, "score 0.988");
        assert_eq!(view_model.items[0].snippet, "No snippet returned.");
        assert_eq!(
            view_model.items[0].href,
            Some("/products/boots".to_string())
        );
        assert_eq!(view_model.facets.len(), 1);
    }
}

pub fn entity_source_label(entity_type: &str, source_module: &str) -> String {
    format!("{} | {}", entity_type, source_module)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultsLabels {
    pub summary_template: String,
    pub preset_template: String,
    pub none_label: String,
    pub locale_template: String,
    pub no_snippet: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultItemViewModel {
    pub id: String,
    pub source_label: String,
    pub score_label: String,
    pub title: String,
    pub snippet: String,
    pub href: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResultsViewModel {
    pub query_log_id: Option<String>,
    pub summary: String,
    pub preset: String,
    pub locale: String,
    pub has_items: bool,
    pub items: Vec<SearchResultItemViewModel>,
    pub facets: Vec<SearchFacetGroup>,
}

pub fn build_search_results_view_model(
    payload: SearchPreviewPayload,
    selected_preset: &str,
    labels: &SearchResultsLabels,
) -> SearchResultsViewModel {
    let locale = locale_or_all(payload.items.first().and_then(|item| item.locale.clone()));
    let SearchPreviewPayload {
        query_log_id,
        preset_key,
        items,
        total,
        took_ms,
        engine,
        ranking_profile,
        facets,
    } = payload;

    let has_items = has_items(items.as_slice());
    let items = items
        .into_iter()
        .map(|item| SearchResultItemViewModel {
            id: item.id,
            source_label: entity_source_label(&item.entity_type, &item.source_module),
            score_label: score_label(item.score),
            title: item.title,
            snippet: snippet_or_fallback(item.snippet, labels.no_snippet.as_str()),
            href: item.url,
        })
        .collect();

    SearchResultsViewModel {
        query_log_id,
        summary: render_results_summary(
            labels.summary_template.as_str(),
            total,
            took_ms,
            engine.as_str(),
            ranking_profile.as_str(),
        ),
        preset: render_preset_label(
            labels.preset_template.as_str(),
            applied_preset_or_selected(preset_key, selected_preset, labels.none_label.as_str())
                .as_str(),
        ),
        locale: render_locale_label(labels.locale_template.as_str(), locale.as_str()),
        has_items,
        items,
        facets,
    }
}

pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
}

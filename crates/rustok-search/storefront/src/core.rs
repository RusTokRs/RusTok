use crate::model::{SearchFacetGroup, SearchFilterPreset, SearchPreviewPayload, SearchSuggestion};

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
                buckets: vec![crate::model::SearchFacetBucket {
                    value: "product".to_string(),
                    count: 2,
                }],
            }],
        };
        let labels = SearchResultsLabels {
            summary_template: "{count} results in {took_ms}ms via {engine}/{ranking_profile}"
                .to_string(),
            preset_template: "preset: {preset}".to_string(),
            none_label: "none".to_string(),
            locale_template: "locale: {locale}".to_string(),
            query_label: "Query".to_string(),
            no_snippet: "No snippet returned.".to_string(),
            no_target_label: "No target".to_string(),
            open_result_label: "Open result".to_string(),
            no_results_title: "No results".to_string(),
            no_results_body: "Try again".to_string(),
            engine_title: "Engine".to_string(),
            engine_body: "Uses FTS".to_string(),
            facet_title: "Facets".to_string(),
            facet_body: "Facet details".to_string(),
        };

        let view_model = build_search_results_view_model(payload, "manual", "boots", &labels);

        assert_eq!(view_model.header.query_label, "Query");
        assert_eq!(view_model.header.query, "boots");
        assert_eq!(
            view_model.header.summary,
            "1 results in 12ms via postgres/balanced"
        );
        assert_eq!(view_model.header.preset, "preset: manual");
        assert_eq!(view_model.header.locale, "locale: ru");
        assert!(view_model.has_items);
        assert_eq!(view_model.items[0].id, "doc-1");
        assert_eq!(view_model.items[0].source_label, "product | catalog");
        assert_eq!(view_model.items[0].score_label, "score 0.988");
        assert_eq!(view_model.items[0].snippet, "No snippet returned.");
        assert_eq!(
            view_model.items[0].action,
            SearchResultActionViewModel::Open {
                label: "Open result".to_string(),
                href: "/products/boots".to_string(),
                query_log_id: Some("log-1".to_string()),
                document_id: "doc-1".to_string(),
                position: 1,
            }
        );
        assert_eq!(view_model.facets.len(), 1);
        assert_eq!(view_model.facets[0].display_name, "entity type");
        assert_eq!(view_model.facets[0].buckets[0].label, "product (2)");
        assert_eq!(view_model.no_results_empty_state.title, "No results");
        assert_eq!(view_model.feature_cards.len(), 2);
        assert_eq!(view_model.feature_cards[0].title, "Engine");
    }

    #[test]
    fn search_suggestion_view_models_prepare_labels_and_navigation() {
        let suggestions = vec![
            crate::model::SearchSuggestion {
                text: "Boots".to_string(),
                kind: "document".to_string(),
                document_id: Some("doc-1".to_string()),
                entity_type: Some("product".to_string()),
                source_module: Some("catalog".to_string()),
                locale: Some("ru".to_string()),
                url: Some("/products/boots".to_string()),
                score: 0.9,
            },
            crate::model::SearchSuggestion {
                text: "sneakers".to_string(),
                kind: "query".to_string(),
                document_id: None,
                entity_type: None,
                source_module: None,
                locale: None,
                url: None,
                score: 0.8,
            },
            crate::model::SearchSuggestion {
                text: "Fallback document".to_string(),
                kind: "document".to_string(),
                document_id: Some("doc-2".to_string()),
                entity_type: Some("blog".to_string()),
                source_module: Some("blog".to_string()),
                locale: None,
                url: None,
                score: 0.7,
            },
        ];

        let view_models = build_search_suggestion_view_models(
            suggestions,
            &SearchSuggestionsLabels {
                open_label: "Open".to_string(),
                search_label: "Search".to_string(),
            },
        );

        assert_eq!(view_models.len(), 3);
        assert_eq!(view_models[0].text, "Boots");
        assert_eq!(view_models[0].kind_label, "document • ru");
        assert_eq!(view_models[0].action_label, "Open");
        assert_eq!(
            view_models[0].navigation,
            SearchSuggestionNavigation::Href("/products/boots".to_string())
        );
        assert_eq!(view_models[1].kind_label, "query");
        assert_eq!(view_models[1].action_label, "Search");
        assert_eq!(
            view_models[1].navigation,
            SearchSuggestionNavigation::SearchQuery("sneakers".to_string())
        );
        assert_eq!(
            view_models[2].navigation,
            SearchSuggestionNavigation::SearchQuery("Fallback document".to_string())
        );
    }

    #[test]
    fn preset_chip_view_models_prepare_state_and_next_selection() {
        let presets = vec![
            crate::model::SearchFilterPreset {
                key: "featured".to_string(),
                label: "Featured".to_string(),
                entity_types: vec!["product".to_string()],
                source_modules: Vec::new(),
                statuses: vec!["published".to_string()],
                ranking_profile: Some("default".to_string()),
            },
            crate::model::SearchFilterPreset {
                key: "content".to_string(),
                label: "Content".to_string(),
                entity_types: vec!["page".to_string()],
                source_modules: vec!["pages".to_string()],
                statuses: vec!["published".to_string()],
                ranking_profile: None,
            },
        ];

        let view_models = build_search_preset_chip_view_models(presets, "featured");

        assert_eq!(view_models.len(), 2);
        assert_eq!(view_models[0].key, "featured");
        assert_eq!(view_models[0].label, "Featured");
        assert!(view_models[0].is_selected);
        assert_eq!(view_models[0].class_name, PRESET_CHIP_SELECTED_CLASS);
        assert_eq!(view_models[0].next_selection, "");
        assert!(!view_models[1].is_selected);
        assert_eq!(view_models[1].class_name, PRESET_CHIP_IDLE_CLASS);
        assert_eq!(view_models[1].next_selection, "content");
        assert_eq!(
            preset_chip_class("content", "content"),
            PRESET_CHIP_SELECTED_CLASS
        );
        assert_eq!(preset_chip_class("", "content"), PRESET_CHIP_IDLE_CLASS);
    }

    #[test]
    fn facet_view_models_prepare_display_names_and_bucket_labels() {
        let facets = build_search_facet_view_models(vec![SearchFacetGroup {
            name: "source_module".to_string(),
            buckets: vec![
                crate::model::SearchFacetBucket {
                    value: "catalog".to_string(),
                    count: 7,
                },
                crate::model::SearchFacetBucket {
                    value: "pages".to_string(),
                    count: 3,
                },
            ],
        }]);

        assert_eq!(facets.len(), 1);
        assert_eq!(facets[0].display_name, "source module");
        assert_eq!(facets[0].buckets[0].label, "catalog (7)");
        assert_eq!(facets[0].buckets[1].label, "pages (3)");
    }

    #[test]
    fn result_action_view_model_prepares_open_and_no_target_states() {
        let labels = SearchResultsLabels {
            summary_template: String::new(),
            preset_template: String::new(),
            none_label: String::new(),
            locale_template: String::new(),
            query_label: String::new(),
            no_snippet: String::new(),
            no_target_label: "No target".to_string(),
            open_result_label: "Open result".to_string(),
            no_results_title: "No results".to_string(),
            no_results_body: "Try again".to_string(),
            engine_title: "Engine".to_string(),
            engine_body: "Uses FTS".to_string(),
            facet_title: "Facets".to_string(),
            facet_body: "Facet details".to_string(),
        };

        assert_eq!(
            build_search_result_action_view_model(
                Some("log-1".to_string()),
                "doc-1".to_string(),
                Some("/products/boots".to_string()),
                2,
                &labels,
            ),
            SearchResultActionViewModel::Open {
                label: "Open result".to_string(),
                href: "/products/boots".to_string(),
                query_log_id: Some("log-1".to_string()),
                document_id: "doc-1".to_string(),
                position: 2,
            }
        );
        assert_eq!(
            build_search_result_action_view_model(None, "doc-2".to_string(), None, 1, &labels),
            SearchResultActionViewModel::NoTarget {
                label: "No target".to_string(),
            }
        );
    }

    #[test]
    fn empty_state_and_feature_card_view_models_prepare_render_ready_copy() {
        let empty = build_search_empty_state_view_model(
            "No results".to_string(),
            "Try a different query".to_string(),
        );
        assert_eq!(empty.title, "No results");
        assert_eq!(empty.body, "Try a different query");

        let labels = SearchResultsLabels {
            summary_template: String::new(),
            preset_template: String::new(),
            none_label: String::new(),
            locale_template: String::new(),
            query_label: String::new(),
            no_snippet: String::new(),
            no_target_label: String::new(),
            open_result_label: String::new(),
            no_results_title: String::new(),
            no_results_body: String::new(),
            engine_title: "Engine".to_string(),
            engine_body: "Uses FTS".to_string(),
            facet_title: "Facet model".to_string(),
            facet_body: "Facet details".to_string(),
        };
        let cards = build_search_results_feature_cards(&labels);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].title, "Engine");
        assert_eq!(cards[0].body, "Uses FTS");
        assert_eq!(cards[1].title, "Facet model");
        assert_eq!(cards[1].body, "Facet details");
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSuggestionsLabels {
    pub open_label: String,
    pub search_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchSuggestionNavigation {
    Href(String),
    SearchQuery(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSuggestionItemViewModel {
    pub text: String,
    pub kind_label: String,
    pub action_label: String,
    pub navigation: SearchSuggestionNavigation,
}

pub fn build_search_suggestion_view_models(
    suggestions: Vec<SearchSuggestion>,
    labels: &SearchSuggestionsLabels,
) -> Vec<SearchSuggestionItemViewModel> {
    suggestions
        .into_iter()
        .map(|suggestion| {
            let navigation = if is_document_suggestion(suggestion.kind.as_str()) {
                suggestion
                    .url
                    .clone()
                    .map(SearchSuggestionNavigation::Href)
                    .unwrap_or_else(|| {
                        SearchSuggestionNavigation::SearchQuery(suggestion.text.clone())
                    })
            } else {
                SearchSuggestionNavigation::SearchQuery(suggestion.text.clone())
            };

            SearchSuggestionItemViewModel {
                kind_label: suggestion_kind_with_locale(
                    suggestion.kind.as_str(),
                    suggestion.locale.as_deref(),
                ),
                action_label: suggestion_action_label(
                    suggestion.kind.as_str(),
                    labels.open_label.as_str(),
                    labels.search_label.as_str(),
                ),
                text: suggestion.text,
                navigation,
            }
        })
        .collect()
}

pub fn next_preset_selection(current: &str, selected_key: &str) -> String {
    if current == selected_key {
        String::new()
    } else {
        selected_key.to_string()
    }
}

pub const PRESET_CHIP_SELECTED_CLASS: &str = "inline-flex items-center rounded-full border border-primary bg-primary/10 px-3 py-1 text-xs font-medium text-primary";
pub const PRESET_CHIP_IDLE_CLASS: &str = "inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPresetChipViewModel {
    pub key: String,
    pub label: String,
    pub is_selected: bool,
    pub class_name: &'static str,
    pub next_selection: String,
}

pub fn preset_chip_class(current: &str, preset_key: &str) -> &'static str {
    if current == preset_key {
        PRESET_CHIP_SELECTED_CLASS
    } else {
        PRESET_CHIP_IDLE_CLASS
    }
}

pub fn build_search_preset_chip_view_models(
    presets: Vec<SearchFilterPreset>,
    selected_preset: &str,
) -> Vec<SearchPresetChipViewModel> {
    presets
        .into_iter()
        .map(|preset| {
            let is_selected = selected_preset == preset.key;
            SearchPresetChipViewModel {
                next_selection: next_preset_selection(selected_preset, preset.key.as_str()),
                class_name: preset_chip_class(selected_preset, preset.key.as_str()),
                is_selected,
                key: preset.key,
                label: preset.label,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFacetBucketViewModel {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFacetGroupViewModel {
    pub display_name: String,
    pub buckets: Vec<SearchFacetBucketViewModel>,
}

pub fn build_search_facet_view_models(
    facets: Vec<SearchFacetGroup>,
) -> Vec<SearchFacetGroupViewModel> {
    facets
        .into_iter()
        .map(|facet| SearchFacetGroupViewModel {
            display_name: facet_display_name(facet.name.as_str()),
            buckets: facet
                .buckets
                .into_iter()
                .map(|bucket| SearchFacetBucketViewModel {
                    label: facet_bucket_label(bucket.value.as_str(), bucket.count),
                })
                .collect(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEmptyStateViewModel {
    pub title: String,
    pub body: String,
}

pub fn build_search_empty_state_view_model(
    title: String,
    body: String,
) -> SearchEmptyStateViewModel {
    SearchEmptyStateViewModel { title, body }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFeatureCardViewModel {
    pub title: String,
    pub body: String,
}

pub fn build_search_results_feature_cards(
    labels: &SearchResultsLabels,
) -> Vec<SearchFeatureCardViewModel> {
    vec![
        SearchFeatureCardViewModel {
            title: labels.engine_title.clone(),
            body: labels.engine_body.clone(),
        },
        SearchFeatureCardViewModel {
            title: labels.facet_title.clone(),
            body: labels.facet_body.clone(),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultsLabels {
    pub summary_template: String,
    pub preset_template: String,
    pub none_label: String,
    pub locale_template: String,
    pub query_label: String,
    pub no_snippet: String,
    pub no_target_label: String,
    pub open_result_label: String,
    pub no_results_title: String,
    pub no_results_body: String,
    pub engine_title: String,
    pub engine_body: String,
    pub facet_title: String,
    pub facet_body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultItemViewModel {
    pub id: String,
    pub source_label: String,
    pub score_label: String,
    pub title: String,
    pub snippet: String,
    pub action: SearchResultActionViewModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultActionViewModel {
    Open {
        label: String,
        href: String,
        query_log_id: Option<String>,
        document_id: String,
        position: i32,
    },
    NoTarget {
        label: String,
    },
}

pub fn build_search_result_action_view_model(
    query_log_id: Option<String>,
    document_id: String,
    href: Option<String>,
    position: i32,
    labels: &SearchResultsLabels,
) -> SearchResultActionViewModel {
    href.map(|href| SearchResultActionViewModel::Open {
        label: labels.open_result_label.clone(),
        href,
        query_log_id,
        document_id,
        position,
    })
    .unwrap_or_else(|| SearchResultActionViewModel::NoTarget {
        label: labels.no_target_label.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultsHeaderViewModel {
    pub query_label: String,
    pub query: String,
    pub summary: String,
    pub preset: String,
    pub locale: String,
}

#[derive(Debug, Clone)]
pub struct SearchResultsViewModel {
    pub header: SearchResultsHeaderViewModel,
    pub has_items: bool,
    pub items: Vec<SearchResultItemViewModel>,
    pub facets: Vec<SearchFacetGroupViewModel>,
    pub no_results_empty_state: SearchEmptyStateViewModel,
    pub feature_cards: Vec<SearchFeatureCardViewModel>,
}

pub fn build_search_results_view_model(
    payload: SearchPreviewPayload,
    selected_preset: &str,
    query: &str,
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
        .enumerate()
        .map(|(index, item)| {
            let id = item.id;
            let action = build_search_result_action_view_model(
                query_log_id.clone(),
                id.clone(),
                item.url,
                (index + 1) as i32,
                labels,
            );

            SearchResultItemViewModel {
                id,
                source_label: entity_source_label(&item.entity_type, &item.source_module),
                score_label: score_label(item.score),
                title: item.title,
                snippet: snippet_or_fallback(item.snippet, labels.no_snippet.as_str()),
                action,
            }
        })
        .collect();

    SearchResultsViewModel {
        header: SearchResultsHeaderViewModel {
            query_label: labels.query_label.clone(),
            query: query.to_string(),
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
        },
        has_items,
        items,
        facets: build_search_facet_view_models(facets),
        no_results_empty_state: build_search_empty_state_view_model(
            labels.no_results_title.clone(),
            labels.no_results_body.clone(),
        ),
        feature_cards: build_search_results_feature_cards(labels),
    }
}

pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
}

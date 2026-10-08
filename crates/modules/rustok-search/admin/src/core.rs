pub use rustok_grid::{ColumnAlign, ColumnFilters, FilterValue, GridColumnDef, GridFilterType};
use rustok_ui_core::{UiRouteQueryUpdate, route_query_update_for_text};
pub use rustok_ui_core::{normalize_ui_text as optional_text, parse_ui_csv as parse_csv};

use crate::model::{
    LaggingSearchDocumentPayload, SearchAnalyticsInsightRowPayload, SearchAnalyticsQueryRowPayload,
    SearchAnalyticsSummaryPayload, SearchAttributeFilter, SearchConsistencyIssuePayload,
    SearchDiagnosticsPayload, SearchFacetGroup, SearchPreviewFilters, SearchPreviewPayload,
    SearchQueryRulePayload, SearchStopWordPayload, SearchSynonymPayload,
};

pub fn facet_display_name(raw_name: &str) -> String {
    raw_name.replace('_', " ")
}

pub fn facet_bucket_label(value: &str, count: u64) -> String {
    format!("{} ({})", value, count)
}

pub fn facet_bucket_display_label(value: &str, label: Option<&str>, count: u64) -> String {
    facet_bucket_label(
        label.filter(|label| !label.is_empty()).unwrap_or(value),
        count,
    )
}

pub fn snippet_or_fallback(snippet: Option<String>, fallback: &str) -> String {
    snippet.unwrap_or_else(|| fallback.to_string())
}

pub fn score_label(score: f64) -> String {
    format!("score {:.3}", score)
}

pub type RouteQueryUpdate = UiRouteQueryUpdate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPreviewLabels {
    pub title: String,
    pub summary_template: String,
    pub preset_template: String,
    pub none_label: String,
    pub no_snippet: String,
    pub no_target_url: String,
    pub open_result: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchPreviewItemViewModel {
    pub id: String,
    pub source_label: String,
    pub score_label: String,
    pub title: String,
    pub snippet: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchPreviewViewModel {
    pub title: String,
    pub summary: String,
    pub preset: String,
    pub query_log_id: Option<String>,
    pub facets: Vec<SearchFacetGroup>,
    pub items: Vec<SearchPreviewItemViewModel>,
}

pub fn build_search_preview_view_model(
    payload: SearchPreviewPayload,
    labels: &SearchPreviewLabels,
) -> SearchPreviewViewModel {
    SearchPreviewViewModel {
        title: labels.title.clone(),
        summary: render_preview_summary(
            labels.summary_template.as_str(),
            payload.total,
            payload.took_ms,
            payload.engine.as_str(),
            payload.ranking_profile.as_str(),
        ),
        preset: render_preview_preset(
            labels.preset_template.as_str(),
            payload.preset_key.as_deref(),
            labels.none_label.as_str(),
        ),
        query_log_id: payload.query_log_id,
        facets: payload.facets,
        items: payload
            .items
            .into_iter()
            .map(|item| SearchPreviewItemViewModel {
                source_label: entity_source_label(&item.entity_type, &item.source_module),
                score_label: score_label(item.score),
                snippet: snippet_or_fallback(item.snippet, labels.no_snippet.as_str()),
                id: item.id,
                title: item.title,
                url: item.url,
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPreviewFormInput<'a> {
    pub query: &'a str,
    pub channel_id: &'a str,
    pub entity_types: &'a str,
    pub source_modules: &'a str,
    pub statuses: &'a str,
    pub category_ids: &'a str,
    pub attribute_code: &'a str,
    pub attribute_values: &'a str,
    pub attribute_min: &'a str,
    pub attribute_max: &'a str,
    pub sort_attribute_code: &'a str,
    pub sort_desc: bool,
    pub ranking_profile: &'a str,
    pub preset_key: &'a str,
    pub locale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchPreviewRequest {
    pub query: String,
    pub locale: Option<String>,
    pub ranking_profile: Option<String>,
    pub preset_key: Option<String>,
    pub filters: SearchPreviewFilters,
    pub route_query_update: RouteQueryUpdate,
}

pub fn route_query_update(query: &str) -> RouteQueryUpdate {
    route_query_update_for_text(query)
}

pub fn build_search_preview_request(input: SearchPreviewFormInput<'_>) -> SearchPreviewRequest {
    let attribute_filters = optional_text(input.attribute_code)
        .map(|attribute_code| {
            vec![SearchAttributeFilter {
                attribute_code,
                values: parse_csv(input.attribute_values),
                min: optional_text(input.attribute_min),
                max: optional_text(input.attribute_max),
            }]
        })
        .unwrap_or_default();

    SearchPreviewRequest {
        query: input.query.to_string(),
        locale: input.locale,
        ranking_profile: optional_text(input.ranking_profile),
        preset_key: optional_text(input.preset_key),
        filters: SearchPreviewFilters {
            channel_id: optional_text(input.channel_id),
            entity_types: parse_csv(input.entity_types),
            source_modules: parse_csv(input.source_modules),
            statuses: parse_csv(input.statuses),
            category_ids: parse_csv(input.category_ids),
            attribute_filters,
            sort_attribute_code: optional_text(input.sort_attribute_code),
            sort_desc: input.sort_desc,
        },
        route_query_update: route_query_update(input.query),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSynonymMutationInput<'a> {
    pub term: &'a str,
    pub synonyms: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSynonymMutationRequest {
    pub term: String,
    pub synonyms: Vec<String>,
}

pub fn build_search_synonym_mutation_request(
    input: SearchSynonymMutationInput<'_>,
) -> SearchSynonymMutationRequest {
    SearchSynonymMutationRequest {
        term: input.term.to_string(),
        synonyms: parse_csv(input.synonyms),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStopWordMutationInput<'a> {
    pub value: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStopWordMutationRequest {
    pub value: String,
}

pub fn build_search_stop_word_mutation_request(
    input: SearchStopWordMutationInput<'_>,
) -> SearchStopWordMutationRequest {
    SearchStopWordMutationRequest {
        value: input.value.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPinRuleMutationInput<'a> {
    pub query_text: &'a str,
    pub document_id: &'a str,
    pub pinned_position: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPinRuleMutationRequest {
    pub query_text: String,
    pub document_id: String,
    pub pinned_position: Option<i32>,
}

pub fn build_search_pin_rule_mutation_request(
    input: SearchPinRuleMutationInput<'_>,
    invalid_position_message: &str,
) -> Result<SearchPinRuleMutationRequest, String> {
    let pinned_position = match optional_text(input.pinned_position) {
        Some(value) => Some(
            value
                .parse::<i32>()
                .map_err(|_| invalid_position_message.to_string())?,
        ),
        None => Some(1),
    };

    Ok(SearchPinRuleMutationRequest {
        query_text: input.query_text.to_string(),
        document_id: input.document_id.to_string(),
        pinned_position,
    })
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
    fn route_query_update_preserves_non_empty_preview_query() {
        assert_eq!(route_query_update("   "), RouteQueryUpdate::Clear);
        assert_eq!(
            route_query_update("  botas "),
            RouteQueryUpdate::Replace("  botas ".to_string())
        );
    }

    #[test]
    fn search_preview_request_normalizes_form_state_without_ui_runtime() {
        let request = build_search_preview_request(SearchPreviewFormInput {
            query: " botas ",
            channel_id: " 23da77fd-6cd5-49c7-a188-60db852d70a1 ",
            entity_types: " product, blog ,, ",
            source_modules: " catalog, cms ",
            statuses: "published, draft",
            category_ids: "2fc8574b-9eb8-4e93-9cb2-711982f67c46",
            attribute_code: " color ",
            attribute_values: "red, blue",
            attribute_min: "",
            attribute_max: "",
            sort_attribute_code: " price ",
            sort_desc: true,
            ranking_profile: " balanced ",
            preset_key: "  ",
            locale: Some("ru".to_string()),
        });

        assert_eq!(request.query, " botas ");
        assert_eq!(request.locale, Some("ru".to_string()));
        assert_eq!(request.ranking_profile, Some("balanced".to_string()));
        assert_eq!(request.preset_key, None);
        assert_eq!(
            request.route_query_update,
            RouteQueryUpdate::Replace(" botas ".to_string())
        );
        assert_eq!(
            request.filters.entity_types,
            vec!["product".to_string(), "blog".to_string()]
        );
        assert_eq!(
            request.filters.source_modules,
            vec!["catalog".to_string(), "cms".to_string()]
        );
        assert_eq!(
            request.filters.statuses,
            vec!["published".to_string(), "draft".to_string()]
        );
        assert_eq!(
            request.filters.channel_id.as_deref(),
            Some("23da77fd-6cd5-49c7-a188-60db852d70a1")
        );
        assert_eq!(request.filters.category_ids.len(), 1);
        assert_eq!(request.filters.attribute_filters.len(), 1);
        assert_eq!(request.filters.attribute_filters[0].attribute_code, "color");
        assert_eq!(
            request.filters.attribute_filters[0].values,
            vec!["red".to_string(), "blue".to_string()]
        );
        assert_eq!(
            request.filters.sort_attribute_code.as_deref(),
            Some("price")
        );
        assert!(request.filters.sort_desc);
    }

    #[test]
    fn dictionary_mutation_requests_normalize_ui_form_state_without_runtime() {
        let synonym = build_search_synonym_mutation_request(SearchSynonymMutationInput {
            term: " boot ",
            synonyms: " boots, shoe ,, sneaker ",
        });
        assert_eq!(synonym.term, " boot ");
        assert_eq!(synonym.synonyms, vec!["boots", "shoe", "sneaker"]);

        let stop_word =
            build_search_stop_word_mutation_request(SearchStopWordMutationInput { value: " the " });
        assert_eq!(stop_word.value, " the ");

        let pin_rule = build_search_pin_rule_mutation_request(
            SearchPinRuleMutationInput {
                query_text: " winter boots ",
                document_id: " product-1 ",
                pinned_position: " 2 ",
            },
            "invalid position",
        )
        .expect("valid pinned position");
        assert_eq!(pin_rule.query_text, " winter boots ");
        assert_eq!(pin_rule.document_id, " product-1 ");
        assert_eq!(pin_rule.pinned_position, Some(2));

        let default_pin_rule = build_search_pin_rule_mutation_request(
            SearchPinRuleMutationInput {
                query_text: "boots",
                document_id: "product-1",
                pinned_position: "  ",
            },
            "invalid position",
        )
        .expect("blank pinned position defaults to first position");
        assert_eq!(default_pin_rule.pinned_position, Some(1));

        let err = build_search_pin_rule_mutation_request(
            SearchPinRuleMutationInput {
                query_text: "boots",
                document_id: "product-1",
                pinned_position: "first",
            },
            "invalid position",
        )
        .expect_err("non-numeric pinned position should fail");
        assert_eq!(err, "invalid position");
    }

    #[test]
    fn search_preview_view_model_prepares_render_ready_fields() {
        let payload = SearchPreviewPayload {
            query_log_id: Some("log-1".to_string()),
            preset_key: None,
            total: 1,
            took_ms: 12,
            engine: "postgres".to_string(),
            ranking_profile: "balanced".to_string(),
            facets: vec![SearchFacetGroup {
                name: "entity_type".to_string(),
                buckets: vec![],
            }],
            items: vec![crate::model::SearchPreviewResultItem {
                id: "doc-1".to_string(),
                entity_type: "product".to_string(),
                source_module: "catalog".to_string(),
                title: "Boots".to_string(),
                snippet: None,
                score: 0.98765,
                locale: Some("en".to_string()),
                url: Some("/products/boots".to_string()),
                payload: "{}".to_string(),
            }],
        };
        let labels = SearchPreviewLabels {
            title: "Preview Results".to_string(),
            summary_template: "{total} results in {took_ms} ms via {engine} ({ranking_profile})"
                .to_string(),
            preset_template: "preset = {preset}".to_string(),
            none_label: "none".to_string(),
            no_snippet: "No snippet returned.".to_string(),
            no_target_url: "No target URL".to_string(),
            open_result: "Open".to_string(),
        };

        let view_model = build_search_preview_view_model(payload, &labels);

        assert_eq!(view_model.title, "Preview Results");
        assert_eq!(
            view_model.summary,
            "1 results in 12 ms via postgres (balanced)"
        );
        assert_eq!(view_model.preset, "preset = none");
        assert_eq!(view_model.query_log_id, Some("log-1".to_string()));
        assert_eq!(view_model.facets[0].name, "entity_type");
        assert_eq!(view_model.items[0].source_label, "product | catalog");
        assert_eq!(view_model.items[0].score_label, "score 0.988");
        assert_eq!(view_model.items[0].snippet, "No snippet returned.");
    }

    #[test]
    fn analytics_formatting_helpers_are_stable() {
        assert_eq!(format_days(14), "14d");
        assert_eq!(format_percent_fraction(0.1234), "12.3%");
        assert_eq!(format_milliseconds(12.34), "12.3 ms");
        assert_eq!(format_decimal_1(7.89), "7.9");
        assert_eq!(format_seconds(42), "42s");
        assert_eq!(
            document_source_path("doc-1", "catalog", "product"),
            "doc-1 / catalog / product"
        );
    }

    #[test]
    fn analytics_summary_view_model_formats_render_ready_values() {
        let view_model =
            build_search_analytics_summary_view_model(&SearchAnalyticsSummaryPayload {
                window_days: 30,
                total_queries: 1000,
                successful_queries: 900,
                zero_result_queries: 45,
                zero_result_rate: 0.045,
                slow_queries: 12,
                slow_query_rate: 0.012,
                avg_took_ms: 18.42,
                avg_results_per_query: 7.5,
                unique_queries: 250,
                clicked_queries: 375,
                total_clicks: 420,
                click_through_rate: 0.375,
                abandonment_queries: 125,
                abandonment_rate: 0.125,
                last_query_at: Some("2026-05-31T00:00:00Z".to_string()),
            });

        assert_eq!(view_model.window, "30d");
        assert_eq!(view_model.total_queries, "1000");
        assert_eq!(view_model.click_through_rate, "37.5%");
        assert_eq!(view_model.abandonment_rate, "12.5%");
        assert_eq!(view_model.zero_result_rate, "4.5%");
        assert_eq!(view_model.avg_took_ms, "18.4 ms");
        assert_eq!(view_model.slow_query_rate, "1.2%");
        assert_eq!(view_model.total_clicks, "420");
        assert_eq!(view_model.abandonment_queries, "125");
        assert_eq!(view_model.unique_queries, "250");
    }

    #[test]
    fn analytics_row_view_models_format_table_values() {
        let rows =
            build_search_analytics_query_row_view_models(vec![SearchAnalyticsQueryRowPayload {
                query: "boots".to_string(),
                hits: 12,
                zero_result_hits: 2,
                clicks: 5,
                avg_took_ms: 8.64,
                avg_results: 3.25,
                click_through_rate: 0.416,
                abandonment_rate: 0.125,
                last_seen_at: "2026-05-31T00:00:00Z".to_string(),
            }]);

        assert_eq!(rows[0].query, "boots");
        assert_eq!(rows[0].hits, "12");
        assert_eq!(rows[0].zero_result_hits, "2");
        assert_eq!(rows[0].clicks, "5");
        assert_eq!(rows[0].click_through_rate, "41.6%");
        assert_eq!(rows[0].abandonment_rate, "12.5%");
        assert_eq!(rows[0].avg_took_ms, "8.6 ms");
        assert_eq!(rows[0].avg_results, "3.2");
        assert_eq!(rows[0].last_seen_at, "2026-05-31T00:00:00Z");

        let insights = build_search_analytics_insight_row_view_models(vec![
            SearchAnalyticsInsightRowPayload {
                query: "sneakers".to_string(),
                hits: 20,
                zero_result_hits: 0,
                clicks: 3,
                click_through_rate: 0.15,
                abandonment_rate: 0.4,
                recommendation: "Add synonym".to_string(),
            },
        ]);

        assert_eq!(insights[0].query, "sneakers");
        assert_eq!(insights[0].hits, "20");
        assert_eq!(insights[0].zero_result_hits, "0");
        assert_eq!(insights[0].clicks, "3");
        assert_eq!(insights[0].click_through_rate, "15.0%");
        assert_eq!(insights[0].recommendation, "Add synonym");
    }

    #[test]
    fn navigation_and_rebuild_helpers_are_stable() {
        assert_eq!(module_overview_href("search"), "/modules/search");
        assert_eq!(
            module_section_href("search", "analytics"),
            "/modules/search/analytics"
        );
        assert_eq!(
            engine_option_label("PostgreSQL", "postgres"),
            "PostgreSQL (postgres)"
        );
        assert_eq!(rebuild_target_suffix(Some("doc-1")), " for target doc-1");
        assert_eq!(rebuild_target_suffix(None), "");
        assert_eq!(
            render_rebuild_feedback("Queued {scope}{suffix}", "product", Some("p1")),
            "Queued product for target p1"
        );
    }

    #[test]
    fn css_class_helpers_are_stable() {
        assert_eq!(
            diagnostics_state_badge_class("healthy"),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            diagnostics_state_badge_class("unknown"),
            "border-slate-200 bg-slate-50 text-slate-700"
        );
        assert_eq!(
            consistency_issue_badge_class("missing"),
            "border-rose-200 bg-rose-50 text-rose-700"
        );
        assert_eq!(
            consistency_issue_badge_class("orphaned"),
            "border-orange-200 bg-orange-50 text-orange-700"
        );
        assert!(tab_class(true).contains("bg-primary"));
        assert!(tab_class(false).contains("border-border"));
    }

    #[test]
    fn preview_and_fallback_rendering_helpers_are_stable() {
        assert_eq!(
            render_preview_summary(
                "{total} results in {took_ms} ms via {engine} ({ranking_profile})",
                7,
                42,
                "postgres",
                "balanced",
            ),
            "7 results in 42 ms via postgres (balanced)"
        );
        assert_eq!(
            render_preview_preset("preset = {preset}", Some("published"), "none"),
            "preset = published"
        );
        assert_eq!(
            render_preview_preset("preset = {preset}", None, "none"),
            "preset = none"
        );
        assert_eq!(
            value_or_fallback(Some("2026-05-28".to_string()), "n/a"),
            "2026-05-28"
        );
        assert_eq!(value_or_fallback(None, "n/a"), "n/a");
        assert_eq!(
            label_value_summary("Newest indexed", "n/a"),
            "Newest indexed: n/a"
        );
    }

    #[test]
    fn lagging_document_row_view_models_format_render_ready_values() {
        let rows =
            build_lagging_search_document_row_view_models(vec![LaggingSearchDocumentPayload {
                document_key: "product:boot-1".to_string(),
                document_id: "boot-1".to_string(),
                source_module: "catalog".to_string(),
                entity_type: "product".to_string(),
                locale: "en".to_string(),
                status: "published".to_string(),
                is_public: true,
                title: "Boots".to_string(),
                updated_at: "2026-05-31T00:01:00Z".to_string(),
                indexed_at: "2026-05-31T00:00:00Z".to_string(),
                lag_seconds: 61,
            }]);

        assert_eq!(rows[0].title, "Boots");
        assert_eq!(rows[0].document_key, "product:boot-1");
        assert_eq!(rows[0].source_status_label, "catalog/product (published)");
        assert_eq!(rows[0].locale, "en");
        assert_eq!(rows[0].lag, "61s");
        assert_eq!(rows[0].indexed_at, "2026-05-31T00:00:00Z");
        assert_eq!(rows[0].updated_at, "2026-05-31T00:01:00Z");
    }

    #[test]
    fn consistency_issue_row_view_models_format_render_ready_values() {
        let labels = SearchConsistencyIssueLabels {
            missing: "missing".to_string(),
            orphaned: "orphaned".to_string(),
            not_indexed: "not indexed".to_string(),
        };
        let rows = build_search_consistency_issue_row_view_models(
            vec![
                SearchConsistencyIssuePayload {
                    issue_kind: "missing".to_string(),
                    document_key: "product:boot-1".to_string(),
                    document_id: "boot-1".to_string(),
                    source_module: "catalog".to_string(),
                    entity_type: "product".to_string(),
                    locale: "en".to_string(),
                    status: "published".to_string(),
                    title: "Boots".to_string(),
                    updated_at: "2026-05-31T00:01:00Z".to_string(),
                    indexed_at: None,
                },
                SearchConsistencyIssuePayload {
                    issue_kind: "orphaned".to_string(),
                    document_key: "blog:post-1".to_string(),
                    document_id: "post-1".to_string(),
                    source_module: "blog".to_string(),
                    entity_type: "post".to_string(),
                    locale: "ru".to_string(),
                    status: "archived".to_string(),
                    title: "Post".to_string(),
                    updated_at: "2026-05-30T00:00:00Z".to_string(),
                    indexed_at: Some("2026-05-29T00:00:00Z".to_string()),
                },
            ],
            &labels,
        );

        assert_eq!(rows[0].issue_label, "missing");
        assert_eq!(
            rows[0].issue_badge_class,
            "border-rose-200 bg-rose-50 text-rose-700"
        );
        assert_eq!(rows[0].source_status_label, "catalog/product (published)");
        assert_eq!(rows[0].indexed_at, "not indexed");
        assert_eq!(rows[1].issue_label, "orphaned");
        assert_eq!(
            rows[1].issue_badge_class,
            "border-orange-200 bg-orange-50 text-orange-700"
        );
        assert_eq!(rows[1].source_status_label, "blog/post (archived)");
        assert_eq!(rows[1].indexed_at, "2026-05-29T00:00:00Z");
    }

    #[test]
    fn dictionary_row_view_models_prepare_render_ready_values() {
        let synonyms = build_search_synonym_row_view_models(vec![SearchSynonymPayload {
            id: "syn-1".to_string(),
            term: "boot".to_string(),
            synonyms: vec!["boots".to_string(), "shoe".to_string()],
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        }]);
        assert_eq!(synonyms[0].id, "syn-1");
        assert_eq!(synonyms[0].term, "boot");
        assert_eq!(synonyms[0].synonyms_summary, "boots, shoe");
        assert_eq!(synonyms[0].updated_at, "2026-06-01T00:00:00Z");

        let stop_words = build_search_stop_word_row_view_models(vec![SearchStopWordPayload {
            id: "stop-1".to_string(),
            value: "the".to_string(),
            updated_at: "2026-06-01T00:01:00Z".to_string(),
        }]);
        assert_eq!(stop_words[0].id, "stop-1");
        assert_eq!(stop_words[0].value, "the");
        assert_eq!(stop_words[0].updated_at, "2026-06-01T00:01:00Z");

        let query_rules = build_search_query_rule_row_view_models(vec![SearchQueryRulePayload {
            id: "pin-1".to_string(),
            query_text: "winter boots".to_string(),
            query_normalized: "winter boots".to_string(),
            rule_kind: "pin".to_string(),
            document_id: "product-1".to_string(),
            entity_type: "product".to_string(),
            source_module: "catalog".to_string(),
            title: "Winter Boots".to_string(),
            pinned_position: 1,
            updated_at: "2026-06-01T00:02:00Z".to_string(),
        }]);
        assert_eq!(query_rules[0].id, "pin-1");
        assert_eq!(query_rules[0].query_text, "winter boots");
        assert_eq!(query_rules[0].query_normalized, "winter boots");
        assert_eq!(query_rules[0].title, "Winter Boots");
        assert_eq!(
            query_rules[0].target_source_path,
            "product-1 / catalog / product"
        );
        assert_eq!(query_rules[0].pinned_position, "1");
        assert_eq!(query_rules[0].updated_at, "2026-06-01T00:02:00Z");
    }

    #[test]
    fn diagnostics_card_view_model_formats_state_and_newest_indexed() {
        let labels = SearchDiagnosticsLabels {
            healthy: "healthy".to_string(),
            inconsistent: "inconsistent".to_string(),
            lagging: "lagging".to_string(),
            not_indexed_yet: "not indexed yet".to_string(),
            newest_indexed: "Newest indexed".to_string(),
        };
        let diagnostics = SearchDiagnosticsPayload {
            tenant_id: "tenant-1".to_string(),
            total_documents: 10,
            public_documents: 8,
            content_documents: 4,
            product_documents: 6,
            stale_documents: 2,
            missing_documents: 1,
            orphaned_documents: 0,
            newest_indexed_at: None,
            oldest_indexed_at: Some("2026-05-30T00:00:00Z".to_string()),
            max_lag_seconds: 42,
            state: "lagging".to_string(),
        };

        let view_model = build_search_diagnostics_card_view_model(diagnostics, &labels);

        assert_eq!(view_model.state_label, "lagging");
        assert_eq!(
            view_model.badge_class,
            "border-amber-200 bg-amber-50 text-amber-700"
        );
        assert_eq!(
            view_model.newest_indexed_summary,
            "Newest indexed: not indexed yet"
        );

        let unknown = build_search_diagnostics_card_view_model(
            SearchDiagnosticsPayload {
                tenant_id: "tenant-1".to_string(),
                total_documents: 0,
                public_documents: 0,
                content_documents: 0,
                product_documents: 0,
                stale_documents: 0,
                missing_documents: 0,
                orphaned_documents: 0,
                newest_indexed_at: Some("2026-05-31T00:00:00Z".to_string()),
                oldest_indexed_at: None,
                max_lag_seconds: 0,
                state: "custom".to_string(),
            },
            &labels,
        );

        assert_eq!(unknown.state_label, "custom");
        assert_eq!(
            unknown.newest_indexed_summary,
            "Newest indexed: 2026-05-31T00:00:00Z"
        );
    }

    #[test]
    fn relevance_editor_merge_helpers_are_stable() {
        let merged = merge_relevance_editor_config(
            RelevanceEditorConfigInput {
                config_text: "{}",
                ranking_default: "balanced",
                ranking_preview: "freshness",
                ranking_storefront: "balanced",
                ranking_admin_global: "exact",
                preview_presets: "[{\"key\":\"published\"}]",
                storefront_presets: "[]",
            },
            RelevanceEditorMessages {
                invalid_settings_json: "invalid settings",
                settings_root_object: "root must be object",
                preview_presets_label: "Preview presets",
                storefront_presets_label: "Storefront presets",
                editor_array_json: "{label} JSON error: {err}",
                editor_array_type: "{label} must be an array",
                serialize_merged_settings: "serialize failed",
            },
        )
        .expect("merge should succeed");
        let parsed = parse_json_for_editor(&merged).expect("merged config should be JSON");

        assert_eq!(
            extract_ranking_profile_value(&parsed, "search_preview"),
            "freshness"
        );
        assert!(parsed["filter_presets"]["search_preview"].is_array());
        assert_eq!(
            parse_json_array_for_editor(
                "Preview presets",
                "{}",
                "{label} JSON error: {err}",
                "{label} must be an array",
            ),
            Err("Preview presets must be an array".to_string())
        );
    }

    #[test]
    fn relevance_editor_json_helpers_are_stable() {
        let config = serde_json::json!({
            "ranking_profiles": {
                "search_preview": "freshness",
                "admin_global_search": "operator"
            },
            "filter_presets": {
                "search_preview": [
                    {"key": "published", "label": "Published"}
                ]
            }
        });

        assert_eq!(
            extract_ranking_profile_value(&config, "search_preview"),
            "freshness"
        );
        assert_eq!(
            extract_ranking_profile_value(&config, "storefront_search"),
            "balanced"
        );
        assert_eq!(
            extract_ranking_profile_value(&serde_json::json!({}), "admin_global_search"),
            "exact"
        );
        assert_eq!(
            extract_surface_presets_json(&config, "search_preview"),
            "[\n  {\n    \"key\": \"published\",\n    \"label\": \"Published\"\n  }\n]"
        );
        assert_eq!(extract_surface_presets_json(&config, "missing"), "[]");
        assert_eq!(pretty_json_string("{\"a\":1}"), "{\n  \"a\": 1\n}");
        assert_eq!(pretty_json_string("not-json"), "not-json");
    }

    #[test]
    fn search_grid_column_definitions_and_filtering_work() {
        // 1. Analytics Queries
        let query_cols = search_analytics_query_grid_columns(Some("en"));
        assert_eq!(query_cols.len(), 9);
        let ru_query_cols = search_analytics_query_grid_columns(Some("ru"));
        assert_eq!(ru_query_cols[0].title, "Запрос");

        let analytics_rows = vec![
            SearchAnalyticsQueryRowViewModel {
                query: "winter boots".to_string(),
                hits: "10".to_string(),
                zero_result_hits: "0".to_string(),
                clicks: "5".to_string(),
                click_through_rate: "50%".to_string(),
                abandonment_rate: "10%".to_string(),
                avg_took_ms: "12 ms".to_string(),
                avg_results: "8.0".to_string(),
                last_seen_at: "2026-06-01".to_string(),
            },
            SearchAnalyticsQueryRowViewModel {
                query: "summer sneakers".to_string(),
                hits: "20".to_string(),
                zero_result_hits: "1".to_string(),
                clicks: "8".to_string(),
                click_through_rate: "40%".to_string(),
                abandonment_rate: "15%".to_string(),
                avg_took_ms: "15 ms".to_string(),
                avg_results: "12.0".to_string(),
                last_seen_at: "2026-06-02".to_string(),
            },
        ];
        let mut filters = ColumnFilters::new();
        assert_eq!(
            filter_search_analytics_queries(&analytics_rows, &filters, Some("boots")).len(),
            1
        );
        filters.set("query", FilterValue::Text("sneakers".to_string()));
        assert_eq!(
            filter_search_analytics_queries(&analytics_rows, &filters, None).len(),
            1
        );

        // 2. Analytics Insights
        let insight_cols = search_analytics_insight_grid_columns(Some("en"));
        assert_eq!(insight_cols.len(), 6);
        let insight_rows = vec![SearchAnalyticsInsightRowViewModel {
            query: "running shoes".to_string(),
            hits: "15".to_string(),
            zero_result_hits: "2".to_string(),
            clicks: "4".to_string(),
            click_through_rate: "26%".to_string(),
            recommendation: "Add synonym".to_string(),
        }];
        let mut insight_filters = ColumnFilters::new();
        insight_filters.set("recommendation", FilterValue::Text("synonym".to_string()));
        assert_eq!(
            filter_search_analytics_insights(&insight_rows, &insight_filters, None).len(),
            1
        );

        // 3. Lagging Documents
        let lag_cols = lagging_search_document_grid_columns(Some("en"));
        assert_eq!(lag_cols.len(), 6);
        let lag_rows = vec![LaggingSearchDocumentRowViewModel {
            title: "Leather Jacket".to_string(),
            document_key: "product:jacket-1".to_string(),
            source_status_label: "catalog/product (published)".to_string(),
            locale: "en".to_string(),
            lag: "120s".to_string(),
            indexed_at: "2026-06-01T00:00:00Z".to_string(),
            updated_at: "2026-06-01T00:02:00Z".to_string(),
        }];
        assert_eq!(
            filter_lagging_search_documents(&lag_rows, &ColumnFilters::new(), Some("jacket")).len(),
            1
        );

        // 4. Consistency Issues
        let consistency_cols = search_consistency_issue_grid_columns(Some("en"));
        assert_eq!(consistency_cols.len(), 6);
        let consistency_rows = vec![SearchConsistencyIssueRowViewModel {
            issue_badge_class: "border-rose-200",
            issue_label: "missing".to_string(),
            title: "Missing Cap".to_string(),
            document_key: "product:cap-1".to_string(),
            source_status_label: "catalog/product (published)".to_string(),
            locale: "en".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
            indexed_at: "not indexed".to_string(),
        }];
        assert_eq!(
            filter_search_consistency_issues(&consistency_rows, &ColumnFilters::new(), Some("cap"))
                .len(),
            1
        );

        // 5. Synonyms
        let syn_cols = search_synonym_grid_columns(Some("en"));
        assert_eq!(syn_cols.len(), 4);
        let syn_rows = vec![SearchSynonymRowViewModel {
            id: "syn-1".to_string(),
            term: "sneaker".to_string(),
            synonyms_summary: "shoe, trainer".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        }];
        assert_eq!(
            filter_search_synonyms(&syn_rows, &ColumnFilters::new(), Some("trainer")).len(),
            1
        );

        // 6. Stop Words
        let stop_cols = search_stop_word_grid_columns(Some("en"));
        assert_eq!(stop_cols.len(), 3);
        let stop_rows = vec![SearchStopWordRowViewModel {
            id: "stop-1".to_string(),
            value: "the".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        }];
        assert_eq!(
            filter_search_stop_words(&stop_rows, &ColumnFilters::new(), Some("the")).len(),
            1
        );

        // 7. Query Rules
        let rule_cols = search_query_rule_grid_columns(Some("en"));
        assert_eq!(rule_cols.len(), 5);
        let rule_rows = vec![SearchQueryRuleRowViewModel {
            id: "rule-1".to_string(),
            query_text: "warm coat".to_string(),
            query_normalized: "warm coat".to_string(),
            title: "Winter Coat".to_string(),
            target_source_path: "p-1 / catalog / product".to_string(),
            pinned_position: "1".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        }];
        assert_eq!(
            filter_search_query_rules(&rule_rows, &ColumnFilters::new(), Some("coat")).len(),
            1
        );
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

pub fn pretty_json_string(value: &str) -> String {
    parse_json_for_editor(value)
        .and_then(|json| serde_json::to_string_pretty(&json).ok())
        .unwrap_or_else(|| value.to_string())
}

pub fn parse_json_for_editor(value: &str) -> Option<serde_json::Value> {
    serde_json::from_str(value).ok()
}

pub fn extract_ranking_profile_value(config: &serde_json::Value, surface: &str) -> String {
    config
        .get("ranking_profiles")
        .and_then(|value| value.get(surface))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(match surface {
            "admin_global_search" => "exact",
            _ => "balanced",
        })
        .to_string()
}

pub fn extract_surface_presets_json(config: &serde_json::Value, surface: &str) -> String {
    config
        .get("filter_presets")
        .and_then(|value| value.get(surface))
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| "[]".to_string())
}

pub fn format_days(days: u32) -> String {
    format!("{}d", days)
}

pub fn format_percent_fraction(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

pub fn format_milliseconds(value: f64) -> String {
    format!("{:.1} ms", value)
}

pub fn format_decimal_1(value: f64) -> String {
    format!("{:.1}", value)
}

pub fn format_seconds(seconds: u64) -> String {
    format!("{}s", seconds)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchAnalyticsSummaryViewModel {
    pub window: String,
    pub total_queries: String,
    pub click_through_rate: String,
    pub abandonment_rate: String,
    pub zero_result_rate: String,
    pub avg_took_ms: String,
    pub slow_query_rate: String,
    pub total_clicks: String,
    pub abandonment_queries: String,
    pub unique_queries: String,
}

pub fn build_search_analytics_summary_view_model(
    summary: &SearchAnalyticsSummaryPayload,
) -> SearchAnalyticsSummaryViewModel {
    SearchAnalyticsSummaryViewModel {
        window: format_days(summary.window_days),
        total_queries: summary.total_queries.to_string(),
        click_through_rate: format_percent_fraction(summary.click_through_rate),
        abandonment_rate: format_percent_fraction(summary.abandonment_rate),
        zero_result_rate: format_percent_fraction(summary.zero_result_rate),
        avg_took_ms: format_milliseconds(summary.avg_took_ms),
        slow_query_rate: format_percent_fraction(summary.slow_query_rate),
        total_clicks: summary.total_clicks.to_string(),
        abandonment_queries: summary.abandonment_queries.to_string(),
        unique_queries: summary.unique_queries.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchAnalyticsQueryRowViewModel {
    pub query: String,
    pub hits: String,
    pub zero_result_hits: String,
    pub clicks: String,
    pub click_through_rate: String,
    pub abandonment_rate: String,
    pub avg_took_ms: String,
    pub avg_results: String,
    pub last_seen_at: String,
}

pub fn build_search_analytics_query_row_view_models(
    rows: Vec<SearchAnalyticsQueryRowPayload>,
) -> Vec<SearchAnalyticsQueryRowViewModel> {
    rows.into_iter()
        .map(|row| SearchAnalyticsQueryRowViewModel {
            query: row.query,
            hits: row.hits.to_string(),
            zero_result_hits: row.zero_result_hits.to_string(),
            clicks: row.clicks.to_string(),
            click_through_rate: format_percent_fraction(row.click_through_rate),
            abandonment_rate: format_percent_fraction(row.abandonment_rate),
            avg_took_ms: format_milliseconds(row.avg_took_ms),
            avg_results: format_decimal_1(row.avg_results),
            last_seen_at: row.last_seen_at,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchAnalyticsInsightRowViewModel {
    pub query: String,
    pub hits: String,
    pub zero_result_hits: String,
    pub clicks: String,
    pub click_through_rate: String,
    pub recommendation: String,
}

pub fn build_search_analytics_insight_row_view_models(
    rows: Vec<SearchAnalyticsInsightRowPayload>,
) -> Vec<SearchAnalyticsInsightRowViewModel> {
    rows.into_iter()
        .map(|row| SearchAnalyticsInsightRowViewModel {
            query: row.query,
            hits: row.hits.to_string(),
            zero_result_hits: row.zero_result_hits.to_string(),
            clicks: row.clicks.to_string(),
            click_through_rate: format_percent_fraction(row.click_through_rate),
            recommendation: row.recommendation,
        })
        .collect()
}

pub fn document_source_path(document_id: &str, source_module: &str, entity_type: &str) -> String {
    format!("{} / {} / {}", document_id, source_module, entity_type)
}

pub fn render_preview_summary(
    template: &str,
    total: u64,
    took_ms: u64,
    engine: &str,
    ranking_profile: &str,
) -> String {
    template
        .replace("{total}", total.to_string().as_str())
        .replace("{took_ms}", took_ms.to_string().as_str())
        .replace("{engine}", engine)
        .replace("{ranking_profile}", ranking_profile)
}

pub fn render_preview_preset(template: &str, preset_key: Option<&str>, none_label: &str) -> String {
    template.replace("{preset}", preset_key.unwrap_or(none_label))
}

pub fn value_or_fallback(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn label_value_summary(label: &str, value: &str) -> String {
    format!("{}: {}", label, value)
}

pub fn diagnostics_state_badge_class(state: &str) -> &'static str {
    match state {
        "healthy" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "inconsistent" => "border-rose-200 bg-rose-50 text-rose-700",
        "lagging" => "border-amber-200 bg-amber-50 text-amber-700",
        _ => "border-slate-200 bg-slate-50 text-slate-700",
    }
}

pub fn consistency_issue_badge_class(issue_kind: &str) -> &'static str {
    if issue_kind == "missing" {
        "border-rose-200 bg-rose-50 text-rose-700"
    } else {
        "border-orange-200 bg-orange-50 text-orange-700"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDiagnosticsLabels {
    pub healthy: String,
    pub inconsistent: String,
    pub lagging: String,
    pub not_indexed_yet: String,
    pub newest_indexed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDiagnosticsCardViewModel {
    pub badge_class: &'static str,
    pub state_label: String,
    pub newest_indexed_summary: String,
}

pub fn build_search_diagnostics_card_view_model(
    diagnostics: SearchDiagnosticsPayload,
    labels: &SearchDiagnosticsLabels,
) -> SearchDiagnosticsCardViewModel {
    let state_label = match diagnostics.state.as_str() {
        "healthy" => labels.healthy.clone(),
        "inconsistent" => labels.inconsistent.clone(),
        "lagging" => labels.lagging.clone(),
        other => other.to_string(),
    };
    let newest_indexed = value_or_fallback(
        diagnostics.newest_indexed_at,
        labels.not_indexed_yet.as_str(),
    );

    SearchDiagnosticsCardViewModel {
        badge_class: diagnostics_state_badge_class(diagnostics.state.as_str()),
        state_label,
        newest_indexed_summary: label_value_summary(
            labels.newest_indexed.as_str(),
            newest_indexed.as_str(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaggingSearchDocumentRowViewModel {
    pub title: String,
    pub document_key: String,
    pub source_status_label: String,
    pub locale: String,
    pub lag: String,
    pub indexed_at: String,
    pub updated_at: String,
}

pub fn build_lagging_search_document_row_view_models(
    rows: Vec<LaggingSearchDocumentPayload>,
) -> Vec<LaggingSearchDocumentRowViewModel> {
    rows.into_iter()
        .map(|row| LaggingSearchDocumentRowViewModel {
            title: row.title,
            document_key: row.document_key,
            source_status_label: source_entity_status_label(
                &row.source_module,
                &row.entity_type,
                &row.status,
            ),
            locale: row.locale,
            lag: format_seconds(row.lag_seconds),
            indexed_at: row.indexed_at,
            updated_at: row.updated_at,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchConsistencyIssueLabels {
    pub missing: String,
    pub orphaned: String,
    pub not_indexed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchConsistencyIssueRowViewModel {
    pub issue_badge_class: &'static str,
    pub issue_label: String,
    pub title: String,
    pub document_key: String,
    pub source_status_label: String,
    pub locale: String,
    pub updated_at: String,
    pub indexed_at: String,
}

pub fn build_search_consistency_issue_row_view_models(
    rows: Vec<SearchConsistencyIssuePayload>,
    labels: &SearchConsistencyIssueLabels,
) -> Vec<SearchConsistencyIssueRowViewModel> {
    rows.into_iter()
        .map(|row| {
            let issue_label = if row.issue_kind == "missing" {
                labels.missing.clone()
            } else {
                labels.orphaned.clone()
            };

            SearchConsistencyIssueRowViewModel {
                issue_badge_class: consistency_issue_badge_class(&row.issue_kind),
                issue_label,
                title: row.title,
                document_key: row.document_key,
                source_status_label: source_entity_status_label(
                    &row.source_module,
                    &row.entity_type,
                    &row.status,
                ),
                locale: row.locale,
                updated_at: row.updated_at,
                indexed_at: value_or_fallback(row.indexed_at, labels.not_indexed.as_str()),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSynonymRowViewModel {
    pub id: String,
    pub term: String,
    pub synonyms_summary: String,
    pub updated_at: String,
}

pub fn build_search_synonym_row_view_models(
    rows: Vec<SearchSynonymPayload>,
) -> Vec<SearchSynonymRowViewModel> {
    rows.into_iter()
        .map(|row| SearchSynonymRowViewModel {
            id: row.id,
            term: row.term,
            synonyms_summary: row.synonyms.join(", "),
            updated_at: row.updated_at,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStopWordRowViewModel {
    pub id: String,
    pub value: String,
    pub updated_at: String,
}

pub fn build_search_stop_word_row_view_models(
    rows: Vec<SearchStopWordPayload>,
) -> Vec<SearchStopWordRowViewModel> {
    rows.into_iter()
        .map(|row| SearchStopWordRowViewModel {
            id: row.id,
            value: row.value,
            updated_at: row.updated_at,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQueryRuleRowViewModel {
    pub id: String,
    pub query_text: String,
    pub query_normalized: String,
    pub title: String,
    pub target_source_path: String,
    pub pinned_position: String,
    pub updated_at: String,
}

pub fn build_search_query_rule_row_view_models(
    rows: Vec<SearchQueryRulePayload>,
) -> Vec<SearchQueryRuleRowViewModel> {
    rows.into_iter()
        .map(|row| SearchQueryRuleRowViewModel {
            id: row.id,
            query_text: row.query_text,
            query_normalized: row.query_normalized,
            title: row.title,
            target_source_path: document_source_path(
                &row.document_id,
                &row.source_module,
                &row.entity_type,
            ),
            pinned_position: row.pinned_position.to_string(),
            updated_at: row.updated_at,
        })
        .collect()
}

pub fn tab_class(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90"
    } else {
        "inline-flex items-center gap-2 rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground"
    }
}

pub fn module_overview_href(route_segment: &str) -> String {
    format!("/modules/{route_segment}")
}

pub fn module_section_href(route_segment: &str, section: &str) -> String {
    format!("/modules/{route_segment}/{section}")
}

pub fn engine_option_label(label: &str, kind: &str) -> String {
    format!("{} ({})", label, kind)
}

pub fn rebuild_target_suffix(target_id: Option<&str>) -> String {
    target_id
        .map(|id| format!(" for target {id}"))
        .unwrap_or_default()
}

pub fn render_rebuild_feedback(template: &str, scope: &str, target_id: Option<&str>) -> String {
    template
        .replace("{scope}", scope)
        .replace("{suffix}", rebuild_target_suffix(target_id).as_str())
}

pub struct RelevanceEditorConfigInput<'a> {
    pub config_text: &'a str,
    pub ranking_default: &'a str,
    pub ranking_preview: &'a str,
    pub ranking_storefront: &'a str,
    pub ranking_admin_global: &'a str,
    pub preview_presets: &'a str,
    pub storefront_presets: &'a str,
}

pub struct RelevanceEditorMessages<'a> {
    pub invalid_settings_json: &'a str,
    pub settings_root_object: &'a str,
    pub preview_presets_label: &'a str,
    pub storefront_presets_label: &'a str,
    pub editor_array_json: &'a str,
    pub editor_array_type: &'a str,
    pub serialize_merged_settings: &'a str,
}

pub fn merge_relevance_editor_config(
    input: RelevanceEditorConfigInput<'_>,
    messages: RelevanceEditorMessages<'_>,
) -> Result<String, String> {
    let mut config = parse_json_for_editor(input.config_text)
        .ok_or_else(|| messages.invalid_settings_json.to_string())?;
    let object = config
        .as_object_mut()
        .ok_or_else(|| messages.settings_root_object.to_string())?;

    object.insert(
        "ranking_profiles".to_string(),
        serde_json::json!({
            "default": input.ranking_default,
            "search_preview": input.ranking_preview,
            "storefront_search": input.ranking_storefront,
            "admin_global_search": input.ranking_admin_global,
        }),
    );
    object.insert(
        "filter_presets".to_string(),
        serde_json::json!({
            "search_preview": parse_json_array_for_editor(
                messages.preview_presets_label,
                input.preview_presets,
                messages.editor_array_json,
                messages.editor_array_type,
            )?,
            "storefront_search": parse_json_array_for_editor(
                messages.storefront_presets_label,
                input.storefront_presets,
                messages.editor_array_json,
                messages.editor_array_type,
            )?,
        }),
    );

    serde_json::to_string_pretty(&config)
        .map_err(|err| error_with_context(messages.serialize_merged_settings, &err.to_string()))
}

pub fn parse_json_array_for_editor(
    label: &str,
    value: &str,
    invalid_json_template: &str,
    array_type_template: &str,
) -> Result<serde_json::Value, String> {
    let parsed: serde_json::Value = serde_json::from_str(value).map_err(|err| {
        invalid_json_template
            .replace("{label}", label)
            .replace("{err}", err.to_string().as_str())
    })?;
    if !parsed.is_array() {
        return Err(array_type_template.replace("{label}", label));
    }
    Ok(parsed)
}

// ============================================================================
// DataGrid Column Definitions and Filtering
// ============================================================================

pub fn search_analytics_query_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("query", if is_ru { "Запрос" } else { "Query" })
            .min_width(200)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр запроса..."
                    } else {
                        "Filter query..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new("hits", if is_ru { "Попадания" } else { "Hits" })
            .width(100)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "zero_result_hits",
            if is_ru { "Нулевые" } else { "Zero hits" },
        )
        .width(100)
        .align(ColumnAlign::Right),
        GridColumnDef::new("clicks", if is_ru { "Клики" } else { "Clicks" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new("ctr", if is_ru { "CTR" } else { "CTR" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "abandonment",
            if is_ru {
                "Без клика"
            } else {
                "Abandonment"
            },
        )
        .width(110)
        .align(ColumnAlign::Right),
        GridColumnDef::new(
            "avg_latency",
            if is_ru {
                "Задержка"
            } else {
                "Avg latency"
            },
        )
        .width(110)
        .align(ColumnAlign::Right),
        GridColumnDef::new(
            "avg_results",
            if is_ru {
                "Ср. результаты"
            } else {
                "Avg results"
            },
        )
        .width(110)
        .align(ColumnAlign::Right),
        GridColumnDef::new(
            "last_seen",
            if is_ru {
                "Последний раз"
            } else {
                "Last seen"
            },
        )
        .min_width(140)
        .align(ColumnAlign::Right),
    ]
}

pub fn matches_search_analytics_query_filter(
    row: &SearchAnalyticsQueryRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("query", FilterValue::Text(q)) => {
                if !row
                    .query
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_analytics_queries(
    rows: &[SearchAnalyticsQueryRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchAnalyticsQueryRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty() && !row.query.to_ascii_lowercase().contains(&term) {
                    return false;
                }
            }
            matches_search_analytics_query_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn search_analytics_insight_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("query", if is_ru { "Запрос" } else { "Query" })
            .min_width(180)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр запроса..."
                    } else {
                        "Filter query..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new("hits", if is_ru { "Попадания" } else { "Hits" })
            .width(100)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "zero_result_hits",
            if is_ru { "Нулевые" } else { "Zero hits" },
        )
        .width(100)
        .align(ColumnAlign::Right),
        GridColumnDef::new("clicks", if is_ru { "Клики" } else { "Clicks" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new("ctr", if is_ru { "CTR" } else { "CTR" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "recommendation",
            if is_ru {
                "Рекомендация"
            } else {
                "Recommendation"
            },
        )
        .min_width(200)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр рекомендации..."
                } else {
                    "Filter recommendation..."
                }
                .into(),
            ),
        }),
    ]
}

pub fn matches_search_analytics_insight_filter(
    row: &SearchAnalyticsInsightRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("query", FilterValue::Text(q)) => {
                if !row
                    .query
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("recommendation", FilterValue::Text(q)) => {
                if !row
                    .recommendation
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_analytics_insights(
    rows: &[SearchAnalyticsInsightRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchAnalyticsInsightRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.query.to_ascii_lowercase().contains(&term)
                    && !row.recommendation.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_search_analytics_insight_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn lagging_search_document_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new(
            "title",
            if is_ru {
                "Заголовок / Ключ"
            } else {
                "Title / Key"
            },
        )
        .min_width(220)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр документа..."
                } else {
                    "Filter document..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new(
            "source",
            if is_ru {
                "Источник / Статус"
            } else {
                "Source / Status"
            },
        )
        .min_width(180)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр источника..."
                } else {
                    "Filter source..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new("locale", if is_ru { "Локаль" } else { "Locale" })
            .width(90)
            .align(ColumnAlign::Center),
        GridColumnDef::new("lag", if is_ru { "Лаг" } else { "Lag" })
            .width(100)
            .align(ColumnAlign::Center),
        GridColumnDef::new(
            "indexed_at",
            if is_ru {
                "Индексировано"
            } else {
                "Indexed"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
    ]
}

pub fn matches_lagging_search_document_filter(
    row: &LaggingSearchDocumentRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("title", FilterValue::Text(q)) => {
                let term = q.to_ascii_lowercase();
                if !row.title.to_ascii_lowercase().contains(&term)
                    && !row.document_key.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("source", FilterValue::Text(q)) => {
                if !row
                    .source_status_label
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("locale", FilterValue::Text(q)) => {
                if !row
                    .locale
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_lagging_search_documents(
    rows: &[LaggingSearchDocumentRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<LaggingSearchDocumentRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.title.to_ascii_lowercase().contains(&term)
                    && !row.document_key.to_ascii_lowercase().contains(&term)
                    && !row.source_status_label.to_ascii_lowercase().contains(&term)
                    && !row.locale.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_lagging_search_document_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn search_consistency_issue_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("issue", if is_ru { "Проблема" } else { "Issue" })
            .width(130)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр проблемы..."
                    } else {
                        "Filter issue..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new(
            "title",
            if is_ru {
                "Заголовок / Ключ"
            } else {
                "Title / Key"
            },
        )
        .min_width(220)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр документа..."
                } else {
                    "Filter document..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new(
            "source",
            if is_ru {
                "Источник / Статус"
            } else {
                "Source / Status"
            },
        )
        .min_width(180)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр источника..."
                } else {
                    "Filter source..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new("locale", if is_ru { "Локаль" } else { "Locale" })
            .width(90)
            .align(ColumnAlign::Center),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
        GridColumnDef::new(
            "indexed_at",
            if is_ru {
                "Индексировано"
            } else {
                "Indexed"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
    ]
}

pub fn matches_search_consistency_issue_filter(
    row: &SearchConsistencyIssueRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("issue", FilterValue::Text(q)) => {
                if !row
                    .issue_label
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("title", FilterValue::Text(q)) => {
                let term = q.to_ascii_lowercase();
                if !row.title.to_ascii_lowercase().contains(&term)
                    && !row.document_key.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("source", FilterValue::Text(q)) => {
                if !row
                    .source_status_label
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("locale", FilterValue::Text(q)) => {
                if !row
                    .locale
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_consistency_issues(
    rows: &[SearchConsistencyIssueRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchConsistencyIssueRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.issue_label.to_ascii_lowercase().contains(&term)
                    && !row.title.to_ascii_lowercase().contains(&term)
                    && !row.document_key.to_ascii_lowercase().contains(&term)
                    && !row.source_status_label.to_ascii_lowercase().contains(&term)
                    && !row.locale.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_search_consistency_issue_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn search_synonym_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("term", if is_ru { "Термин" } else { "Term" })
            .min_width(180)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр термина..."
                    } else {
                        "Filter term..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new(
            "synonyms",
            if is_ru {
                "Синонимы"
            } else {
                "Synonyms"
            },
        )
        .min_width(240)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр синонимов..."
                } else {
                    "Filter synonyms..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(110)
            .not_sortable()
            .align(ColumnAlign::Right),
    ]
}

pub fn matches_search_synonym_filter(
    row: &SearchSynonymRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("term", FilterValue::Text(q)) => {
                if !row
                    .term
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("synonyms", FilterValue::Text(q)) => {
                if !row
                    .synonyms_summary
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_synonyms(
    rows: &[SearchSynonymRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchSynonymRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.term.to_ascii_lowercase().contains(&term)
                    && !row.synonyms_summary.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_search_synonym_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn search_stop_word_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("value", if is_ru { "Значение" } else { "Value" })
            .min_width(200)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр значения..."
                    } else {
                        "Filter value..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(110)
            .not_sortable()
            .align(ColumnAlign::Right),
    ]
}

pub fn matches_search_stop_word_filter(
    row: &SearchStopWordRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("value", FilterValue::Text(q)) => {
                if !row
                    .value
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_stop_words(
    rows: &[SearchStopWordRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchStopWordRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty() && !row.value.to_ascii_lowercase().contains(&term) {
                    return false;
                }
            }
            matches_search_stop_word_filter(row, filters)
        })
        .cloned()
        .collect()
}

pub fn search_query_rule_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("query", if is_ru { "Запрос" } else { "Query" })
            .min_width(200)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(
                    if is_ru {
                        "Фильтр запроса..."
                    } else {
                        "Filter query..."
                    }
                    .into(),
                ),
            }),
        GridColumnDef::new(
            "target",
            if is_ru {
                "Целевой документ"
            } else {
                "Target Document"
            },
        )
        .min_width(220)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(
                if is_ru {
                    "Фильтр цели..."
                } else {
                    "Filter target..."
                }
                .into(),
            ),
        }),
        GridColumnDef::new("position", if is_ru { "Позиция" } else { "Position" })
            .width(100)
            .align(ColumnAlign::Center),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .min_width(150)
        .align(ColumnAlign::Left),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(110)
            .not_sortable()
            .align(ColumnAlign::Right),
    ]
}

pub fn matches_search_query_rule_filter(
    row: &SearchQueryRuleRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("query", FilterValue::Text(q)) => {
                let term = q.to_ascii_lowercase();
                if !row.query_text.to_ascii_lowercase().contains(&term)
                    && !row.query_normalized.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("target", FilterValue::Text(q)) => {
                let term = q.to_ascii_lowercase();
                if !row.title.to_ascii_lowercase().contains(&term)
                    && !row.target_source_path.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_search_query_rules(
    rows: &[SearchQueryRuleRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<SearchQueryRuleRowViewModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.query_text.to_ascii_lowercase().contains(&term)
                    && !row.query_normalized.to_ascii_lowercase().contains(&term)
                    && !row.title.to_ascii_lowercase().contains(&term)
                    && !row.target_source_path.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_search_query_rule_filter(row, filters)
        })
        .cloned()
        .collect()
}

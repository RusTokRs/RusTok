#[cfg(feature = "ssr")]
fn map_search_preview_payload(
    value: rustok_search::SearchResult,
    preset_key: Option<String>,
    query_log_id: Option<i64>,
) -> SearchPreviewPayload {
    SearchPreviewPayload {
        query_log_id: query_log_id.map(|value| value.to_string()),
        preset_key,
        items: value
            .items
            .into_iter()
            .map(|item| {
                let url = rustok_search::canonical_search_result_url(&item);
                crate::model::SearchPreviewResultItem {
                    id: item.id.to_string(),
                    entity_type: item.entity_type,
                    source_module: item.source_module,
                    title: item.title,
                    snippet: item.snippet,
                    score: item.score,
                    locale: item.locale,
                    url,
                    payload: item.payload.to_string(),
                }
            })
            .collect(),
        total: value.total,
        took_ms: value.took_ms,
        engine: value.engine.as_str().to_string(),
        ranking_profile: value.ranking_profile.as_str().to_string(),
        facets: value
            .facets
            .into_iter()
            .map(|facet| crate::model::SearchFacetGroup {
                name: facet.name,
                buckets: facet
                    .buckets
                    .into_iter()
                    .map(|bucket| crate::model::SearchFacetBucket {
                        value: bucket.value,
                        label: bucket.label,
                        count: bucket.count,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_diagnostics_payload(
    value: rustok_search::SearchDiagnosticsSnapshot,
) -> crate::model::SearchDiagnosticsPayload {
    crate::model::SearchDiagnosticsPayload {
        tenant_id: value.tenant_id.to_string(),
        total_documents: value.total_documents,
        public_documents: value.public_documents,
        content_documents: value.content_documents,
        product_documents: value.product_documents,
        stale_documents: value.stale_documents,
        missing_documents: value.missing_documents,
        orphaned_documents: value.orphaned_documents,
        newest_indexed_at: value.newest_indexed_at.map(|value| value.to_rfc3339()),
        oldest_indexed_at: value.oldest_indexed_at.map(|value| value.to_rfc3339()),
        max_lag_seconds: value.max_lag_seconds,
        state: value.state,
    }
}

#[cfg(feature = "ssr")]
fn map_lagging_documents(
    rows: Vec<rustok_search::LaggingSearchDocument>,
) -> Vec<LaggingSearchDocumentPayload> {
    rows.into_iter()
        .map(|value| LaggingSearchDocumentPayload {
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            is_public: value.is_public,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.to_rfc3339(),
            lag_seconds: value.lag_seconds,
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_consistency_issues(
    rows: Vec<rustok_search::SearchConsistencyIssue>,
) -> Vec<SearchConsistencyIssuePayload> {
    rows.into_iter()
        .map(|value| SearchConsistencyIssuePayload {
            issue_kind: value.issue_kind,
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.map(|value| value.to_rfc3339()),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_analytics_payload(value: rustok_search::SearchAnalyticsSnapshot) -> SearchAnalyticsPayload {
    SearchAnalyticsPayload {
        summary: crate::model::SearchAnalyticsSummaryPayload {
            window_days: value.summary.window_days,
            total_queries: value.summary.total_queries,
            successful_queries: value.summary.successful_queries,
            zero_result_queries: value.summary.zero_result_queries,
            zero_result_rate: value.summary.zero_result_rate,
            slow_queries: value.summary.slow_queries,
            slow_query_rate: value.summary.slow_query_rate,
            avg_took_ms: value.summary.avg_took_ms,
            avg_results_per_query: value.summary.avg_results_per_query,
            unique_queries: value.summary.unique_queries,
            clicked_queries: value.summary.clicked_queries,
            total_clicks: value.summary.total_clicks,
            click_through_rate: value.summary.click_through_rate,
            abandonment_queries: value.summary.abandonment_queries,
            abandonment_rate: value.summary.abandonment_rate,
            last_query_at: value.summary.last_query_at.map(|value| value.to_rfc3339()),
        },
        top_queries: map_analytics_rows(value.top_queries),
        zero_result_queries: map_analytics_rows(value.zero_result_queries),
        slow_queries: map_analytics_rows(value.slow_queries),
        low_ctr_queries: map_analytics_rows(value.low_ctr_queries),
        abandonment_queries: map_analytics_rows(value.abandonment_queries),
        intelligence_candidates: value
            .intelligence_candidates
            .into_iter()
            .map(|value| crate::model::SearchAnalyticsInsightRowPayload {
                query: value.query,
                hits: value.hits,
                zero_result_hits: value.zero_result_hits,
                clicks: value.clicks,
                click_through_rate: value.click_through_rate,
                abandonment_rate: value.abandonment_rate,
                recommendation: value.recommendation,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_analytics_rows(
    rows: Vec<rustok_search::SearchAnalyticsQueryRow>,
) -> Vec<crate::model::SearchAnalyticsQueryRowPayload> {
    rows.into_iter()
        .map(|value| crate::model::SearchAnalyticsQueryRowPayload {
            query: value.query,
            hits: value.hits,
            zero_result_hits: value.zero_result_hits,
            clicks: value.clicks,
            avg_took_ms: value.avg_took_ms,
            avg_results: value.avg_results,
            click_through_rate: value.click_through_rate,
            abandonment_rate: value.abandonment_rate,
            last_seen_at: value.last_seen_at.to_rfc3339(),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_dictionary_snapshot(
    value: rustok_search::SearchDictionarySnapshot,
) -> SearchDictionarySnapshotPayload {
    SearchDictionarySnapshotPayload {
        synonyms: value
            .synonyms
            .into_iter()
            .map(|value| crate::model::SearchSynonymPayload {
                id: value.id.to_string(),
                term: value.term,
                synonyms: value.synonyms,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        stop_words: value
            .stop_words
            .into_iter()
            .map(|value| crate::model::SearchStopWordPayload {
                id: value.id.to_string(),
                value: value.value,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        query_rules: value
            .query_rules
            .into_iter()
            .map(|value| crate::model::SearchQueryRulePayload {
                id: value.id.to_string(),
                query_text: value.query_text,
                query_normalized: value.query_normalized,
                rule_kind: value.rule_kind,
                document_id: value.document_id.to_string(),
                entity_type: value.entity_type,
                source_module: value.source_module,
                title: value.title,
                pinned_position: value.pinned_position,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_dictionary_mutation_payload(success: bool) -> SearchDictionaryMutationPayload {
    SearchDictionaryMutationPayload { success }
}

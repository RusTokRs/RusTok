from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text()


def write(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old}")
    write(path, text.replace(old, new, 1))


def replace_after(path: str, anchor: str, old: str, new: str) -> None:
    text = read(path)
    index = text.find(anchor)
    if index < 0:
        raise SystemExit(f"{path}: anchor not found: {anchor}")
    before = text[:index]
    after = text[index:]
    if old not in after:
        raise SystemExit(f"{path}: replacement not found after anchor\n{old}")
    write(path, before + after.replace(old, new, 1))


def replace_span(path: str, start: str, end: str, replacement: str) -> None:
    text = read(path)
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{path}: span start not found: {start}")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"{path}: span end not found: {end}")
    write(path, text[:start_index] + replacement + text[end_index:])


# Register the Search-owned storefront product channel predicate.
replace_once(
    "crates/rustok-search/src/lib.rs",
    "pub mod storefront_channel_authority;\npub mod storefront_result_eligibility;\n",
    "pub mod storefront_channel_authority;\nmod storefront_product_channel_visibility;\npub mod storefront_result_eligibility;\n",
)

# Backfill old product documents at Search bootstrap before they can become visible.
replace_once(
    "crates/rustok-search/src/projector.rs",
    """const CORE_SCOPE_COUNT_SQL: &str = r#\"\nSELECT COUNT(*) AS total\nFROM search_documents\nWHERE tenant_id = $1\n  AND entity_type IN ('node', 'product')\n\"#;\n""",
    """const CORE_SCOPE_COUNT_SQL: &str = r#\"\nSELECT COUNT(*) AS total\nFROM search_documents\nWHERE tenant_id = $1\n  AND entity_type IN ('node', 'product')\n\"#;\n\nconst PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL: &str = r#\"\nSELECT COUNT(*) AS total\nFROM search_documents\nWHERE tenant_id = $1\n  AND entity_type = 'product'\n  AND jsonb_typeof(payload #> '{channel_visibility,allowed_channel_slugs}')\n      IS DISTINCT FROM 'array'\n\"#;\n""",
)
replace_once(
    "crates/rustok-search/src/projector.rs",
    """        if total == 0 {\n            self.rebuild_tenant(tenant_id).await?;\n        }\n        Ok(())\n""",
    """        if total == 0 {\n            self.rebuild_tenant(tenant_id).await?;\n            return Ok(());\n        }\n\n        let drift_statement = Statement::from_sql_and_values(\n            DbBackend::Postgres,\n            PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL,\n            vec![tenant_id.into()],\n        );\n        let drift_total = self\n            .db\n            .query_one(drift_statement)\n            .await\n            .map_err(Error::Database)?\n            .and_then(|row| row.try_get::<i64>(\"\", \"total\").ok())\n            .unwrap_or(0);\n        if drift_total > 0 {\n            self.rebuild_product_scope(tenant_id).await?;\n        }\n\n        Ok(())\n""",
)
replace_once(
    "crates/rustok-search/src/projector.rs",
    """    use super::CORE_SCOPE_COUNT_SQL;\n""",
    """    use super::{CORE_SCOPE_COUNT_SQL, PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL};\n""",
)
replace_once(
    "crates/rustok-search/src/projector.rs",
    """        assert!(!CORE_SCOPE_COUNT_SQL.contains(\"forum_topic\"));\n    }\n}\n""",
    """        assert!(!CORE_SCOPE_COUNT_SQL.contains(\"forum_topic\"));\n    }\n\n    #[test]\n    fn product_channel_visibility_drift_is_fail_closed() {\n        assert!(PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL.contains(\"entity_type = 'product'\"));\n        assert!(PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL.contains(\"allowed_channel_slugs\"));\n        assert!(PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL.contains(\"IS DISTINCT FROM 'array'\"));\n    }\n}\n""",
)

# Project the canonical Product allowlist into Search-owned payloads.
replace_once(
    "crates/rustok-search/src/projector_legacy.rs",
    """                    'variant_count', COALESCE(agg.variant_count, 0),\n                    'published_at', p.published_at\n""",
    """                    'variant_count', COALESCE(agg.variant_count, 0),\n                    'channel_visibility', jsonb_build_object(\n                        'allowed_channel_slugs',\n                        CASE\n                            WHEN NOT (p.metadata ? 'channel_visibility') THEN '[]'::jsonb\n                            WHEN jsonb_typeof(\n                                p.metadata #> '{channel_visibility,allowed_channel_slugs}'\n                            ) = 'array' THEN\n                                p.metadata #> '{channel_visibility,allowed_channel_slugs}'\n                            ELSE 'null'::jsonb\n                        END\n                    ),\n                    'published_at', p.published_at\n""",
)

# PostgreSQL Search uses one storefront-only base predicate for FTS, typo, totals and facets.
replace_once(
    "crates/rustok-search/src/pg_engine.rs",
    "use crate::ranking::SearchRankingProfile;\n",
    "use crate::ranking::SearchRankingProfile;\nuse crate::storefront_product_channel_visibility::product_channel_visibility_sql;\nuse crate::TrustedStorefrontChannel;\n",
)
replace_once(
    "crates/rustok-search/src/pg_engine.rs",
    """    pub(crate) fn connection(&self) -> &DatabaseConnection {\n        &self.db\n    }\n}\n""",
    """    pub(crate) fn connection(&self) -> &DatabaseConnection {\n        &self.db\n    }\n\n    pub async fn search_storefront(\n        &self,\n        query: SearchQuery,\n        channel: &TrustedStorefrontChannel,\n    ) -> Result<SearchResult> {\n        self.search_with_storefront_channel(query, Some(channel)).await\n    }\n\n    async fn search_with_storefront_channel(\n        &self,\n        query: SearchQuery,\n        storefront_channel: Option<&TrustedStorefrontChannel>,\n    ) -> Result<SearchResult> {\n        if self.db.get_database_backend() != DbBackend::Postgres {\n            return Err(Error::External(\n                \"PgSearchEngine requires PostgreSQL backend\".to_string(),\n            ));\n        }\n\n        let trimmed_query = query.query.trim().to_string();\n        if trimmed_query.is_empty() {\n            return Ok(SearchResult {\n                items: Vec::new(),\n                total: 0,\n                took_ms: 0,\n                engine: SearchEngineKind::Postgres,\n                ranking_profile: query.ranking_profile,\n                facets: empty_facets(),\n            });\n        }\n\n        let tenant_id = query.tenant_id.ok_or_else(|| {\n            Error::Validation(\"search preview currently requires tenant_id\".to_string())\n        })?;\n        let locale = query.locale.clone().unwrap_or_default();\n        let limit = query.limit.clamp(1, 50) as i64;\n        let offset = query.offset as i64;\n        let started_at = std::time::Instant::now();\n        let mut result = run_fts_search(\n            &self.db,\n            tenant_id,\n            &locale,\n            &trimmed_query,\n            &query,\n            storefront_channel,\n            offset,\n            limit,\n        )\n        .await?;\n\n        if result.total == 0 && should_run_typo_fallback(&trimmed_query) {\n            result = run_typo_tolerant_search(\n                &self.db,\n                tenant_id,\n                &locale,\n                &trimmed_query,\n                &query,\n                storefront_channel,\n                offset,\n                limit,\n            )\n            .await?;\n        }\n\n        result.took_ms = started_at.elapsed().as_millis() as u64;\n        Ok(result)\n    }\n}\n""",
)
replace_span(
    "crates/rustok-search/src/pg_engine.rs",
    "    async fn search(&self, query: SearchQuery) -> Result<SearchResult> {\n",
    "\n    }\n}\n\nstruct FilterClause",
    """    async fn search(&self, query: SearchQuery) -> Result<SearchResult> {\n        self.search_with_storefront_channel(query, None).await\n""",
)
replace_once(
    "crates/rustok-search/src/pg_engine.rs",
    "fn build_filter_clause(query: &SearchQuery, starting_param: usize) -> FilterClause {\n",
    "fn build_filter_clause(\n    query: &SearchQuery,\n    starting_param: usize,\n    storefront_channel: Option<&TrustedStorefrontChannel>,\n) -> FilterClause {\n",
)
replace_once(
    "crates/rustok-search/src/pg_engine.rs",
    """    if query.published_only {\n        clauses.push(\"is_public = TRUE\".to_string());\n    }\n\n""",
    """    if query.published_only {\n        clauses.push(\"is_public = TRUE\".to_string());\n    }\n\n    if let Some(channel) = storefront_channel {\n        clauses.push(product_channel_visibility_sql(\n            \"entity_type\",\n            \"payload\",\n            channel,\n            &mut values,\n            &mut next_param,\n        ));\n    }\n\n""",
)
replace_once(
    "crates/rustok-search/src/pg_engine.rs",
    """    query: &SearchQuery,\n    offset: i64,\n    limit: i64,\n) -> Result<SearchResult> {\n    let filters = build_filter_clause(query, 4);\n""",
    """    query: &SearchQuery,\n    storefront_channel: Option<&TrustedStorefrontChannel>,\n    offset: i64,\n    limit: i64,\n) -> Result<SearchResult> {\n    let filters = build_filter_clause(query, 4, storefront_channel);\n""",
)
replace_after(
    "crates/rustok-search/src/pg_engine.rs",
    "async fn run_typo_tolerant_search(\n",
    """    query: &SearchQuery,\n    offset: i64,\n    limit: i64,\n) -> Result<SearchResult> {\n    let filters = build_filter_clause(query, 4);\n""",
    """    query: &SearchQuery,\n    storefront_channel: Option<&TrustedStorefrontChannel>,\n    offset: i64,\n    limit: i64,\n) -> Result<SearchResult> {\n    let filters = build_filter_clause(query, 4, storefront_channel);\n""",
)

# Query-rule pins cannot reinsert a restricted Product after the base search.
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    "use crate::engine::{SearchQuery, SearchResult, SearchResultItem};\n",
    "use crate::engine::{SearchQuery, SearchResult, SearchResultItem};\nuse crate::storefront_product_channel_visibility::product_payload_visible_for_storefront;\nuse crate::TrustedStorefrontChannel;\n",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    """    pub async fn apply_query_rules(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        mut result: SearchResult,\n    ) -> Result<SearchResult> {\n        ensure_postgres(db)?;\n""",
    """    pub async fn apply_query_rules(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        result: SearchResult,\n    ) -> Result<SearchResult> {\n        Self::apply_query_rules_with_storefront_channel(db, query, result, None).await\n    }\n\n    pub async fn apply_storefront_query_rules(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        result: SearchResult,\n        channel: &TrustedStorefrontChannel,\n    ) -> Result<SearchResult> {\n        Self::apply_query_rules_with_storefront_channel(db, query, result, Some(channel)).await\n    }\n\n    async fn apply_query_rules_with_storefront_channel(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        mut result: SearchResult,\n        storefront_channel: Option<&TrustedStorefrontChannel>,\n    ) -> Result<SearchResult> {\n        ensure_postgres(db)?;\n""",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    "Self::load_pinned_item(db, query, rule.document_id).await?",
    "Self::load_pinned_item(db, query, rule.document_id, storefront_channel).await?",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    """    async fn load_pinned_item(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        document_id: Uuid,\n    ) -> Result<Option<SearchResultItem>> {\n""",
    """    async fn load_pinned_item(\n        db: &DatabaseConnection,\n        query: &SearchQuery,\n        document_id: Uuid,\n        storefront_channel: Option<&TrustedStorefrontChannel>,\n    ) -> Result<Option<SearchResultItem>> {\n""",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    """        row.filter(|row| pinned_item_matches_query(query, row))\n            .map(map_pinned_item_row)\n""",
    """        row.filter(|row| pinned_item_matches_query(query, row, storefront_channel))\n            .map(map_pinned_item_row)\n""",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    "fn pinned_item_matches_query(query: &SearchQuery, row: &QueryResult) -> bool {\n",
    "fn pinned_item_matches_query(\n    query: &SearchQuery,\n    row: &QueryResult,\n    storefront_channel: Option<&TrustedStorefrontChannel>,\n) -> bool {\n",
)
replace_once(
    "crates/rustok-search/src/dictionaries.rs",
    """    if !query.statuses.is_empty() && !query.statuses.contains(&status) {\n        return false;\n    }\n\n    true\n}\n""",
    """    if !query.statuses.is_empty() && !query.statuses.contains(&status) {\n        return false;\n    }\n    if entity_type == \"product\" {\n        if let Some(channel) = storefront_channel {\n            let payload = match row.try_get::<serde_json::Value>(\"\", \"payload\") {\n                Ok(value) => value,\n                Err(_) => return false,\n            };\n            if !product_payload_visible_for_storefront(&payload, channel) {\n                return false;\n            }\n        }\n    }\n\n    true\n}\n""",
)

# Storefront document suggestions share the exact Product predicate; admin suggestions remain unchanged.
replace_once(
    "crates/rustok-search/src/suggestions.rs",
    "use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};\n",
    "use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};\n",
)
replace_once(
    "crates/rustok-search/src/suggestions.rs",
    "use rustok_core::{Error, Result};\n",
    "use rustok_core::{Error, Result};\n\nuse crate::storefront_product_channel_visibility::product_channel_visibility_sql;\nuse crate::TrustedStorefrontChannel;\n",
)
replace_once(
    "crates/rustok-search/src/suggestions.rs",
    """    pub async fn suggestions(\n        db: &DatabaseConnection,\n        query: SearchSuggestionQuery,\n    ) -> Result<Vec<SearchSuggestion>> {\n        ensure_postgres(db)?;\n""",
    """    pub async fn suggestions(\n        db: &DatabaseConnection,\n        query: SearchSuggestionQuery,\n    ) -> Result<Vec<SearchSuggestion>> {\n        Self::suggestions_with_storefront_channel(db, query, None).await\n    }\n\n    pub async fn storefront_suggestions(\n        db: &DatabaseConnection,\n        query: SearchSuggestionQuery,\n        channel: &TrustedStorefrontChannel,\n    ) -> Result<Vec<SearchSuggestion>> {\n        Self::suggestions_with_storefront_channel(db, query, Some(channel)).await\n    }\n\n    async fn suggestions_with_storefront_channel(\n        db: &DatabaseConnection,\n        query: SearchSuggestionQuery,\n        storefront_channel: Option<&TrustedStorefrontChannel>,\n    ) -> Result<Vec<SearchSuggestion>> {\n        ensure_postgres(db)?;\n""",
)
replace_once(
    "crates/rustok-search/src/suggestions.rs",
    """            query.published_only,\n            limit,\n        )\n""",
    """            query.published_only,\n            limit,\n            storefront_channel,\n        )\n""",
)
replace_once(
    "crates/rustok-search/src/suggestions.rs",
    """    published_only: bool,\n    limit: usize,\n) -> Result<Vec<SearchSuggestion>> {\n    let stmt = Statement::from_sql_and_values(\n        DbBackend::Postgres,\n        r#\"\n        SELECT\n            document_id,\n            entity_type,\n            source_module,\n            title,\n            locale,\n            CASE\n                WHEN lower(title) = $2 THEN 500.0\n                WHEN lower(title) LIKE $3 THEN 320.0\n                WHEN lower(COALESCE(slug, '')) LIKE $3 THEN 250.0\n                WHEN lower(COALESCE(handle, '')) LIKE $3 THEN 220.0\n                WHEN lower(title) LIKE $4 THEN 120.0\n                ELSE 80.0\n            END AS suggestion_score\n        FROM search_documents\n        WHERE tenant_id = $1\n          AND ($5 = '' OR locale = $5)\n          AND ($6 = FALSE OR is_public = TRUE)\n          AND (\n              lower(title) LIKE $4\n              OR lower(COALESCE(slug, '')) LIKE $4\n              OR lower(COALESCE(handle, '')) LIKE $4\n          )\n        ORDER BY suggestion_score DESC, updated_at DESC, title ASC\n        LIMIT $7\n        \"#,\n        vec![\n            tenant_id.into(),\n            normalized_query.to_string().into(),\n            format!(\"{normalized_query}%\").into(),\n            format!(\"%{normalized_query}%\").into(),\n            locale.unwrap_or(\"\").to_string().into(),\n            published_only.into(),\n            (limit as i64).into(),\n        ],\n    );\n""",
    """    published_only: bool,\n    limit: usize,\n    storefront_channel: Option<&TrustedStorefrontChannel>,\n) -> Result<Vec<SearchSuggestion>> {\n    let mut values = vec![\n        tenant_id.into(),\n        normalized_query.to_string().into(),\n        format!(\"{normalized_query}%\").into(),\n        format!(\"%{normalized_query}%\").into(),\n        locale.unwrap_or(\"\").to_string().into(),\n        published_only.into(),\n    ];\n    let mut next_param = 7;\n    let product_scope = storefront_channel\n        .map(|channel| {\n            product_channel_visibility_sql(\n                \"entity_type\",\n                \"payload\",\n                channel,\n                &mut values,\n                &mut next_param,\n            )\n        })\n        .unwrap_or_else(|| \"TRUE\".to_string());\n    let limit_param = next_param;\n    values.push(Value::from(limit as i64));\n\n    let stmt = Statement::from_sql_and_values(\n        DbBackend::Postgres,\n        format!(\n            r#\"\n        SELECT\n            document_id,\n            entity_type,\n            source_module,\n            title,\n            locale,\n            CASE\n                WHEN lower(title) = $2 THEN 500.0\n                WHEN lower(title) LIKE $3 THEN 320.0\n                WHEN lower(COALESCE(slug, '')) LIKE $3 THEN 250.0\n                WHEN lower(COALESCE(handle, '')) LIKE $3 THEN 220.0\n                WHEN lower(title) LIKE $4 THEN 120.0\n                ELSE 80.0\n            END AS suggestion_score\n        FROM search_documents\n        WHERE tenant_id = $1\n          AND ($5 = '' OR locale = $5)\n          AND ($6 = FALSE OR is_public = TRUE)\n          AND {product_scope}\n          AND (\n              lower(title) LIKE $4\n              OR lower(COALESCE(slug, '')) LIKE $4\n              OR lower(COALESCE(handle, '')) LIKE $4\n          )\n        ORDER BY suggestion_score DESC, updated_at DESC, title ASC\n        LIMIT ${limit_param}\n        \"#\n        ),\n        values,\n    );\n""",
)

# GraphQL storefront Search and suggestions use the trusted execution variants.
replace_once(
    "crates/rustok-search/src/graphql/query.rs",
    """    SearchModule, SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n    SearchSuggestionQuery, SearchSuggestionService, resolve_trusted_storefront_channel,\n""",
    """    SearchModule, SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n    SearchSuggestionQuery, SearchSuggestionService, TrustedStorefrontChannel,\n    resolve_trusted_storefront_channel,\n""",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search(\n",
    "let result = run_search_with_dictionaries(db, &engine, search_query.clone()).await;",
    "let result = run_storefront_search_with_dictionaries(\n            db,\n            &engine,\n            search_query.clone(),\n            &trusted_channel,\n        )\n        .await;",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search_suggestions(\n",
    """        let input = normalize_search_suggestions_input(input)?;\n        enforce_storefront_rate_limit(ctx, STOREFRONT_SUGGESTIONS_SURFACE).await?;\n""",
    """        let input = normalize_search_suggestions_input(input)?;\n        let request_context = ctx.data::<RequestContext>()?;\n        let trusted_channel =\n            resolve_trusted_storefront_channel(request_context, tenant.id, None)\n                .map_err(|error| FieldError::new(error.to_string()))?;\n        enforce_storefront_rate_limit(ctx, STOREFRONT_SUGGESTIONS_SURFACE).await?;\n""",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search_suggestions(\n",
    "SearchSuggestionService::suggestions(\n",
    "SearchSuggestionService::storefront_suggestions(\n",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search_suggestions(\n",
    """                published_only: true,\n            },\n        )\n""",
    """                published_only: true,\n            },\n            &trusted_channel,\n        )\n""",
)
replace_once(
    "crates/rustok-search/src/graphql/query.rs",
    """async fn run_search_with_dictionaries(\n    db: &sea_orm::DatabaseConnection,\n    engine: &PgSearchEngine,\n    search_query: SearchQuery,\n) -> rustok_core::Result<crate::SearchResult> {\n    let result = engine.search(search_query.clone()).await?;\n    SearchDictionaryService::apply_query_rules(db, &search_query, result).await\n}\n""",
    """async fn run_search_with_dictionaries(\n    db: &sea_orm::DatabaseConnection,\n    engine: &PgSearchEngine,\n    search_query: SearchQuery,\n) -> rustok_core::Result<crate::SearchResult> {\n    let result = engine.search(search_query.clone()).await?;\n    SearchDictionaryService::apply_query_rules(db, &search_query, result).await\n}\n\nasync fn run_storefront_search_with_dictionaries(\n    db: &sea_orm::DatabaseConnection,\n    engine: &PgSearchEngine,\n    search_query: SearchQuery,\n    channel: &TrustedStorefrontChannel,\n) -> rustok_core::Result<crate::SearchResult> {\n    let result = engine\n        .search_storefront(search_query.clone(), channel)\n        .await?;\n    SearchDictionaryService::apply_storefront_query_rules(\n        db,\n        &search_query,\n        result,\n        channel,\n    )\n    .await\n}\n""",
)

# Native storefront Search and suggestions use the same trusted execution variants.
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_native(\n",
    """            PgSearchEngine, SearchDictionaryService, SearchEngine, SearchFilterPresetService,\n""",
    """            PgSearchEngine, SearchDictionaryService, SearchFilterPresetService,\n""",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_native(\n",
    """        let result = PgSearchEngine::new(db.clone())\n            .search(search_query.clone())\n            .await\n            .map_err(server_error)?;\n        let result = SearchDictionaryService::apply_query_rules(&db, &search_query, result)\n            .await\n            .map_err(server_error)?;\n""",
    """        let result = PgSearchEngine::new(db.clone())\n            .search_storefront(search_query.clone(), &trusted_channel)\n            .await\n            .map_err(server_error)?;\n        let result = SearchDictionaryService::apply_storefront_query_rules(\n            &db,\n            &search_query,\n            result,\n            &trusted_channel,\n        )\n        .await\n        .map_err(server_error)?;\n""",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_suggestions_native(\n",
    "use rustok_api::{HostRuntimeContext, TenantContext};",
    "use rustok_api::{HostRuntimeContext, RequestContext, TenantContext};",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_suggestions_native(\n",
    "use rustok_search::{SearchSuggestionQuery, SearchSuggestionService};",
    "use rustok_search::{\n            SearchSuggestionQuery, SearchSuggestionService, resolve_trusted_storefront_channel,\n        };",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_suggestions_native(\n",
    """        let tenant = leptos_axum::extract::<TenantContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let query = normalize_query(&query)?;\n""",
    """        let tenant = leptos_axum::extract::<TenantContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let request_context = leptos_axum::extract::<RequestContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let trusted_channel =\n            resolve_trusted_storefront_channel(&request_context, tenant.id, None)\n                .map_err(|error| ServerFnError::new(error.to_string()))?;\n        let query = normalize_query(&query)?;\n""",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_suggestions_native(\n",
    "SearchSuggestionService::suggestions(\n",
    "SearchSuggestionService::storefront_suggestions(\n",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_suggestions_native(\n",
    """                published_only: true,\n            },\n        )\n""",
    """                published_only: true,\n            },\n            &trusted_channel,\n        )\n""",
)

# Forum-only execution keeps the same trusted channel across bounded pages and query rules.
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    SearchFilterPresetService, SearchQuery, SearchQueryLogRecord, SearchRankingProfile,\n    SearchResult, SearchResultItem, SearchSettingsService, SharedStorefrontSearchCategoryScopePort,\n""",
    """    SearchFilterPresetService, SearchQuery, SearchQueryLogRecord, SearchRankingProfile,\n    SearchResult, SearchResultItem, SearchSettingsService, SharedStorefrontSearchCategoryScopePort,\n    TrustedStorefrontChannel,\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    request_context: Option<RequestContext>,\n    transport: StorefrontSearchTransport,\n}\n""",
    """    request_context: Option<RequestContext>,\n    trusted_channel: TrustedStorefrontChannel,\n    transport: StorefrontSearchTransport,\n}\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    let input = normalize_request(request)?;\n    let started_at = Instant::now();\n""",
    """    let input = normalize_request(request)?;\n    let trusted_channel = input.trusted_channel.clone();\n    let started_at = Instant::now();\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """        request_context,\n        input.transport,\n    )\n""",
    """        request_context,\n        &trusted_channel,\n        input.transport,\n    )\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    let result = SearchDictionaryService::apply_query_rules(db, &search_query, result).await?;\n""",
    """    let result = SearchDictionaryService::apply_storefront_query_rules(\n        db,\n        &search_query,\n        result,\n        &trusted_channel,\n    )\n    .await?;\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    request_context: Option<RequestContext>,\n    transport: StorefrontSearchTransport,\n) -> Result<SearchResult, ForumStorefrontSearchExecutionError> {\n""",
    """    request_context: Option<RequestContext>,\n    trusted_channel: &TrustedStorefrontChannel,\n    transport: StorefrontSearchTransport,\n) -> Result<SearchResult, ForumStorefrontSearchExecutionError> {\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "let first_page = engine.search(scan_query.clone()).await?;",
    "let first_page = engine\n        .search_storefront(scan_query.clone(), trusted_channel)\n        .await?;",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "let page = engine.search(scan_query.clone()).await?;",
    "let page = engine\n            .search_storefront(scan_query.clone(), trusted_channel)\n            .await?;",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """        request_context: Some(request_context),\n        transport: request.transport,\n""",
    """        request_context: Some(request_context),\n        trusted_channel,\n        transport: request.transport,\n""",
)

print("FORUM-23B2E2 product channel patch applied")

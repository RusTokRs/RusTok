use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};

use rustok_core::{Error, Result};

use crate::engine::{
    SearchConnectorDescriptor, SearchEngine, SearchEngineKind, SearchFacetBucket, SearchFacetGroup,
    SearchQuery, SearchResult, SearchResultItem,
};
use crate::ranking::SearchRankingProfile;
pub struct PgSearchEngine {
    db: DatabaseConnection,
}

impl PgSearchEngine {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn connection(&self) -> &DatabaseConnection {
        &self.db
    }
}

#[async_trait]
impl SearchEngine for PgSearchEngine {
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::Postgres
    }

    fn descriptor(&self) -> SearchConnectorDescriptor {
        SearchConnectorDescriptor::postgres_default()
    }

    async fn search(&self, query: SearchQuery) -> Result<SearchResult> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "PgSearchEngine requires PostgreSQL backend".to_string(),
            ));
        }

        let trimmed_query = query.query.trim().to_string();
        if trimmed_query.is_empty() {
            return Ok(SearchResult {
                items: Vec::new(),
                total: 0,
                took_ms: 0,
                engine: SearchEngineKind::Postgres,
                ranking_profile: query.ranking_profile,
                facets: empty_facets(),
            });
        }

        let tenant_id = query.tenant_id.ok_or_else(|| {
            Error::Validation("search preview currently requires tenant_id".to_string())
        })?;
        let locale = query.locale.clone().unwrap_or_default();
        let limit = query.limit.clamp(1, 50) as i64;
        let offset = query.offset as i64;
        let started_at = std::time::Instant::now();
        let mut result = run_fts_search(
            &self.db,
            tenant_id,
            &locale,
            &trimmed_query,
            &query,
            offset,
            limit,
        )
        .await?;

        if result.total == 0 && should_run_typo_fallback(&trimmed_query) {
            result = run_typo_tolerant_search(
                &self.db,
                tenant_id,
                &locale,
                &trimmed_query,
                &query,
                offset,
                limit,
            )
            .await?;
        }

        result.took_ms = started_at.elapsed().as_millis() as u64;
        Ok(result)
    }
}

struct FilterClause {
    clause: String,
    values: Vec<Value>,
}

fn build_filter_clause(query: &SearchQuery, starting_param: usize) -> FilterClause {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    let mut next_param = starting_param;

    if query.published_only {
        clauses.push("is_public = TRUE".to_string());
    }

    if !query.entity_types.is_empty() {
        clauses.push(format!(
            "entity_type IN ({})",
            bind_list(&query.entity_types, &mut values, &mut next_param)
        ));
    }
    if !query.source_modules.is_empty() {
        clauses.push(format!(
            "source_module IN ({})",
            bind_list(&query.source_modules, &mut values, &mut next_param)
        ));
    }
    if !query.statuses.is_empty() {
        clauses.push(format!(
            "status IN ({})",
            bind_list(&query.statuses, &mut values, &mut next_param)
        ));
    }
    if !query.category_ids.is_empty() {
        clauses.push(format!(
            "entity_type = 'product' AND EXISTS (
                SELECT 1
                FROM index_product_categories ipc
                WHERE ipc.tenant_id = $1
                  AND ipc.product_id = id
                  AND ($2 = '' OR ipc.locale = $2)
                  AND ipc.category_id IN ({})
            )",
            bind_uuid_list(&query.category_ids, &mut values, &mut next_param)
        ));
    }
    for filter in &query.attribute_filters {
        let attribute_param = next_param;
        values.push(filter.attribute_code.clone().into());
        next_param += 1;

        let mut attribute_clauses = vec![
            "iav.tenant_id = $1".to_string(),
            "iav.product_id = id".to_string(),
            "($2 = '' OR iav.locale = $2)".to_string(),
            format!("iav.attribute_code = ${attribute_param}"),
            "iav.is_detached = FALSE".to_string(),
            channel_scope_clause("iav", query, &mut values, &mut next_param),
        ];

        if !filter.values.is_empty() {
            attribute_clauses.push(format!(
                "iav.facet_bucket_key IN ({})",
                bind_list(&filter.values, &mut values, &mut next_param)
            ));
        }
        if let Some(min) = filter.min.as_ref() {
            let min_param = next_param;
            values.push(min.clone().into());
            next_param += 1;
            attribute_clauses.push(format!(
                "CASE
                    WHEN iav.value_number IS NOT NULL AND ${min_param} ~ '^-?[0-9]+(\\.[0-9]+)?$'
                    THEN iav.value_number >= ${min_param}::numeric
                    ELSE iav.sort_value >= ${min_param}
                END"
            ));
        }
        if let Some(max) = filter.max.as_ref() {
            let max_param = next_param;
            values.push(max.clone().into());
            next_param += 1;
            attribute_clauses.push(format!(
                "CASE
                    WHEN iav.value_number IS NOT NULL AND ${max_param} ~ '^-?[0-9]+(\\.[0-9]+)?$'
                    THEN iav.value_number <= ${max_param}::numeric
                    ELSE iav.sort_value <= ${max_param}
                END"
            ));
        }

        clauses.push(format!(
            "entity_type = 'product' AND EXISTS (
                SELECT 1
                FROM index_product_attribute_values iav
                WHERE {}
            )",
            attribute_clauses.join(" AND ")
        ));
    }

    let clause = if clauses.is_empty() {
        "TRUE".to_string()
    } else {
        clauses.join(" AND ")
    };

    FilterClause { clause, values }
}

async fn run_fts_search(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    locale: &str,
    trimmed_query: &str,
    query: &SearchQuery,
    offset: i64,
    limit: i64,
) -> Result<SearchResult> {
    let filters = build_filter_clause(query, 4);
    let cte = build_fts_cte(query.ranking_profile);

    finalize_ranked_search(
        db,
        RankedSearchPlan {
            cte: &cte,
            base_values: build_base_values(tenant_id, locale, trimmed_query, &filters.values),
            filters: &filters,
            query,
            ranking_profile: query.ranking_profile,
            offset,
            limit,
        },
    )
    .await
}

async fn run_typo_tolerant_search(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    locale: &str,
    trimmed_query: &str,
    query: &SearchQuery,
    offset: i64,
    limit: i64,
) -> Result<SearchResult> {
    let filters = build_filter_clause(query, 4);
    let cte = build_typo_cte(query.ranking_profile);

    finalize_ranked_search(
        db,
        RankedSearchPlan {
            cte: &cte,
            base_values: build_base_values(
                tenant_id,
                locale,
                &trimmed_query.to_ascii_lowercase(),
                &filters.values,
            ),
            filters: &filters,
            query,
            ranking_profile: query.ranking_profile,
            offset,
            limit,
        },
    )
    .await
}

struct RankedSearchPlan<'a> {
    cte: &'a str,
    base_values: Vec<Value>,
    filters: &'a FilterClause,
    query: &'a SearchQuery,
    ranking_profile: SearchRankingProfile,
    offset: i64,
    limit: i64,
}

async fn finalize_ranked_search(
    db: &DatabaseConnection,
    plan: RankedSearchPlan<'_>,
) -> Result<SearchResult> {
    let RankedSearchPlan {
        cte,
        base_values,
        filters,
        query,
        ranking_profile,
        offset,
        limit,
    } = plan;

    let total_statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "{cte} SELECT COUNT(*) AS total FROM ranked WHERE {}",
            filters.clause
        ),
        base_values.clone(),
    );
    let total = db
        .query_one(total_statement)
        .await
        .map_err(Error::Database)?
        .and_then(|row| row.try_get::<i64>("", "total").ok())
        .unwrap_or(0)
        .max(0) as u64;

    let offset_param = 4 + filters.values.len();
    let limit_param = offset_param + 1;
    let sort_attribute_param = limit_param + 1;
    let sort_channel_param = sort_attribute_param + 1;
    let order_by = build_order_by_clause(query, sort_attribute_param, sort_channel_param);
    let mut paged_values = build_paged_values_from_base(base_values.clone(), offset, limit);
    if let Some(attribute_code) = normalized_sort_attribute_code(query) {
        paged_values.push(attribute_code.into());
        if let Some(channel_id) = query.channel_id {
            paged_values.push(channel_id.into());
        }
    }
    let items_statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "{cte}
             SELECT id, entity_type, source_module, locale, title, snippet, score, payload
             FROM ranked
             WHERE {}
             {order_by}
             OFFSET ${offset_param}
             LIMIT ${limit_param}",
            filters.clause
        ),
        paged_values,
    );
    let items = db
        .query_all(items_statement)
        .await
        .map_err(Error::Database)?
        .into_iter()
        .map(map_row_to_result_item)
        .collect::<Result<Vec<_>>>()?;

    let facets_statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "{cte}
             SELECT 'entity_type'::text AS facet_name, entity_type AS facet_value, COUNT(*)::bigint AS facet_count
             FROM ranked
             WHERE {}
             GROUP BY entity_type

             UNION ALL

             SELECT 'source_module'::text AS facet_name, source_module AS facet_value, COUNT(*)::bigint AS facet_count
             FROM ranked
             WHERE {}
             GROUP BY source_module

             UNION ALL

             SELECT 'status'::text AS facet_name, status AS facet_value, COUNT(*)::bigint AS facet_count
             FROM ranked
             WHERE {}
             GROUP BY status
             ORDER BY facet_name, facet_count DESC, facet_value ASC",
            filters.clause, filters.clause, filters.clause
        ),
        base_values.clone(),
    );
    let mut facets = build_facets(
        db.query_all(facets_statement)
            .await
            .map_err(Error::Database)?,
    )?;

    let mut attribute_facet_values = base_values;
    let mut next_param = 4 + filters.values.len();
    let attribute_facet_scope =
        channel_scope_clause("iav", query, &mut attribute_facet_values, &mut next_param);
    let attribute_facets_statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "{cte}
             SELECT
                ('attr:' || iav.attribute_code)::text AS facet_name,
                COALESCE(iav.facet_bucket_key, iav.value_key)::text AS facet_value,
                NULLIF(iav.value_label, '')::text AS facet_label,
                COUNT(DISTINCT ranked.id)::bigint AS facet_count
             FROM ranked
             INNER JOIN index_product_attribute_values iav
                ON iav.tenant_id = $1
               AND iav.product_id = ranked.id
               AND ($2 = '' OR iav.locale = $2)
               AND {attribute_facet_scope}
               AND iav.is_filterable = TRUE
               AND iav.is_detached = FALSE
               AND iav.facet_bucket_key IS NOT NULL
               AND iav.facet_bucket_key <> ''
             WHERE {}
               AND ranked.entity_type = 'product'
             GROUP BY iav.attribute_code, COALESCE(iav.facet_bucket_key, iav.value_key), NULLIF(iav.value_label, '')
             ORDER BY facet_name, facet_count DESC, facet_value ASC",
            filters.clause
        ),
        attribute_facet_values,
    );
    facets.extend(build_dynamic_facets(
        db.query_all(attribute_facets_statement)
            .await
            .map_err(Error::Database)?,
    )?);

    Ok(SearchResult {
        items,
        total,
        took_ms: 0,
        engine: SearchEngineKind::Postgres,
        ranking_profile,
        facets,
    })
}

fn build_fts_cte(profile: SearchRankingProfile) -> String {
    let score_sql = profile.fts_score_sql();
    r#"
        WITH q AS (
            SELECT websearch_to_tsquery('simple', $3) AS ts_query
        ),
        ranked AS (
            SELECT
                sd.document_id AS id,
                sd.entity_type AS entity_type,
                sd.source_module AS source_module,
                sd.status AS status,
                sd.locale AS locale,
                sd.title AS title,
                ts_headline('simple', sd.body, q.ts_query) AS snippet,
                __SCORE_SQL__ AS score,
                sd.payload AS payload,
                sd.is_public AS is_public,
                sd.updated_at AS updated_at
            FROM search_documents sd
            CROSS JOIN q
            WHERE sd.tenant_id = $1
              AND ($2 = '' OR sd.locale = $2)
              AND sd.search_vector @@ q.ts_query
        )
    "#
    .replace("__SCORE_SQL__", score_sql)
}

fn build_typo_cte(profile: SearchRankingProfile) -> String {
    let score_sql = profile.typo_score_sql();
    r#"
        WITH candidate_keys AS (
            SELECT document_key
            FROM search_documents
            WHERE tenant_id = $1
              AND ($2 = '' OR locale = $2)
              AND lower(title) % $3
            UNION
            SELECT document_key
            FROM search_documents
            WHERE tenant_id = $1
              AND ($2 = '' OR locale = $2)
              AND lower(COALESCE(slug, '')) % $3
            UNION
            SELECT document_key
            FROM search_documents
            WHERE tenant_id = $1
              AND ($2 = '' OR locale = $2)
              AND lower(COALESCE(handle, '')) % $3
            UNION
            SELECT document_key
            FROM search_documents
            WHERE tenant_id = $1
              AND ($2 = '' OR locale = $2)
              AND lower(COALESCE(keywords_text, '')) % $3
        ),
        ranked AS (
            SELECT
                sd.document_id AS id,
                sd.entity_type AS entity_type,
                sd.source_module AS source_module,
                sd.status AS status,
                sd.locale AS locale,
                sd.title AS title,
                NULLIF(
                    COALESCE(sd.subtitle, sd.slug, sd.handle, ''),
                    ''
                ) AS snippet,
                __SCORE_SQL__ AS score,
                sd.payload AS payload,
                sd.is_public AS is_public,
                sd.updated_at AS updated_at
            FROM search_documents sd
            INNER JOIN candidate_keys candidates
                ON candidates.document_key = sd.document_key
            WHERE sd.tenant_id = $1
              AND ($2 = '' OR sd.locale = $2)
        )
    "#
    .replace("__SCORE_SQL__", score_sql)
}

fn bind_list(values: &[String], bound_values: &mut Vec<Value>, next_param: &mut usize) -> String {
    values
        .iter()
        .map(|value| {
            let placeholder = format!("${}", *next_param);
            bound_values.push(value.clone().into());
            *next_param += 1;
            placeholder
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn bind_uuid_list(
    values: &[uuid::Uuid],
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> String {
    values
        .iter()
        .map(|value| {
            let placeholder = format!("${}", *next_param);
            bound_values.push((*value).into());
            *next_param += 1;
            placeholder
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn channel_scope_clause(
    alias: &str,
    query: &SearchQuery,
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> String {
    match query.channel_id {
        Some(channel_id) => {
            let placeholder = format!("${}", *next_param);
            bound_values.push(channel_id.into());
            *next_param += 1;
            format!("{alias}.channel_id = {placeholder}")
        }
        None => format!("{alias}.channel_id IS NULL"),
    }
}

fn normalized_sort_attribute_code(query: &SearchQuery) -> Option<String> {
    query
        .sort_attribute_code
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn build_order_by_clause(
    query: &SearchQuery,
    sort_attribute_param: usize,
    sort_channel_param: usize,
) -> String {
    if normalized_sort_attribute_code(query).is_none() {
        return "ORDER BY score DESC, updated_at DESC".to_string();
    }

    let direction = if query.sort_desc { "DESC" } else { "ASC" };
    let null_direction = "NULLS LAST";
    let channel_scope = if query.channel_id.is_some() {
        format!("iav.channel_id = ${sort_channel_param}")
    } else {
        "iav.channel_id IS NULL".to_string()
    };

    format!(
        "ORDER BY (
            SELECT iav.sort_value
            FROM index_product_attribute_values iav
            WHERE iav.tenant_id = $1
              AND iav.product_id = ranked.id
              AND ($2 = '' OR iav.locale = $2)
              AND iav.attribute_code = ${sort_attribute_param}
              AND {channel_scope}
              AND iav.is_sortable = TRUE
              AND iav.is_detached = FALSE
            ORDER BY iav.sort_value {direction}
            LIMIT 1
        ) {direction} {null_direction}, score DESC, updated_at DESC"
    )
}

fn build_base_values(
    tenant_id: uuid::Uuid,
    locale: &str,
    trimmed_query: &str,
    filter_values: &[Value],
) -> Vec<Value> {
    let mut values = vec![
        tenant_id.into(),
        locale.to_string().into(),
        trimmed_query.to_string().into(),
    ];
    values.extend(filter_values.iter().cloned());
    values
}

fn build_paged_values_from_base(mut values: Vec<Value>, offset: i64, limit: i64) -> Vec<Value> {
    values.push(offset.into());
    values.push(limit.into());
    values
}

fn should_run_typo_fallback(query: &str) -> bool {
    let normalized = query.trim();
    normalized.len() >= 4 && normalized.split_whitespace().all(|token| token.len() >= 3)
}

fn map_row_to_result_item(row: QueryResult) -> Result<SearchResultItem> {
    let id = row.try_get("", "id").map_err(Error::Database)?;
    let entity_type = row
        .try_get::<String>("", "entity_type")
        .map_err(Error::Database)?;
    let source_module = row
        .try_get::<String>("", "source_module")
        .map_err(Error::Database)?;
    let title = row
        .try_get::<String>("", "title")
        .map_err(Error::Database)?;
    let snippet = row
        .try_get::<Option<String>>("", "snippet")
        .map_err(Error::Database)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let score = row
        .try_get::<f64>("", "score")
        .or_else(|_| row.try_get::<f32>("", "score").map(|value| value as f64))
        .map_err(Error::Database)?;
    let locale = row
        .try_get::<String>("", "locale")
        .map(Some)
        .map_err(Error::Database)?;
    let payload = row
        .try_get::<serde_json::Value>("", "payload")
        .map_err(Error::Database)?;

    Ok(SearchResultItem {
        id,
        entity_type,
        source_module,
        title,
        snippet,
        score,
        locale,
        payload,
    })
}

fn build_facets(rows: Vec<QueryResult>) -> Result<Vec<SearchFacetGroup>> {
    let mut entity_type = Vec::new();
    let mut source_module = Vec::new();
    let mut status = Vec::new();

    for row in rows {
        let facet_name = row
            .try_get::<String>("", "facet_name")
            .map_err(Error::Database)?;
        let bucket = SearchFacetBucket {
            value: row
                .try_get::<String>("", "facet_value")
                .map_err(Error::Database)?,
            label: None,
            count: row
                .try_get::<i64>("", "facet_count")
                .map_err(Error::Database)?
                .max(0) as u64,
        };

        match facet_name.as_str() {
            "entity_type" => entity_type.push(bucket),
            "source_module" => source_module.push(bucket),
            "status" => status.push(bucket),
            _ => {}
        }
    }

    Ok(vec![
        SearchFacetGroup {
            name: "entity_type".to_string(),
            buckets: entity_type,
        },
        SearchFacetGroup {
            name: "source_module".to_string(),
            buckets: source_module,
        },
        SearchFacetGroup {
            name: "status".to_string(),
            buckets: status,
        },
    ])
}

fn build_dynamic_facets(rows: Vec<QueryResult>) -> Result<Vec<SearchFacetGroup>> {
    let mut groups = std::collections::BTreeMap::<String, Vec<SearchFacetBucket>>::new();

    for row in rows {
        let facet_name = row
            .try_get::<String>("", "facet_name")
            .map_err(Error::Database)?;
        let bucket = SearchFacetBucket {
            value: row
                .try_get::<String>("", "facet_value")
                .map_err(Error::Database)?,
            label: row
                .try_get::<Option<String>>("", "facet_label")
                .map_err(Error::Database)?,
            count: row
                .try_get::<i64>("", "facet_count")
                .map_err(Error::Database)?
                .max(0) as u64,
        };
        groups.entry(facet_name).or_default().push(bucket);
    }

    Ok(groups
        .into_iter()
        .map(|(name, buckets)| SearchFacetGroup { name, buckets })
        .collect())
}

fn empty_facets() -> Vec<SearchFacetGroup> {
    vec![
        SearchFacetGroup {
            name: "entity_type".to_string(),
            buckets: Vec::new(),
        },
        SearchFacetGroup {
            name: "source_module".to_string(),
            buckets: Vec::new(),
        },
        SearchFacetGroup {
            name: "status".to_string(),
            buckets: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{build_filter_clause, should_run_typo_fallback};
    use crate::{SearchRankingProfile, engine::SearchQuery};

    #[test]
    fn filter_clause_uses_bound_parameters() {
        let filters = build_filter_clause(
            &SearchQuery {
                tenant_id: None,
                locale: None,
                channel_id: None,
                original_query: "phone".to_string(),
                query: "phone".to_string(),
                ranking_profile: SearchRankingProfile::Balanced,
                preset_key: None,
                limit: 10,
                offset: 0,
                published_only: true,
                entity_types: vec!["product".to_string()],
                source_modules: vec!["commerce".to_string()],
                statuses: vec!["active".to_string()],
                category_ids: Vec::new(),
                attribute_filters: Vec::new(),
                sort_attribute_code: None,
                sort_desc: false,
            },
            4,
        );

        assert_eq!(
            filters.clause,
            "is_public = TRUE AND entity_type IN ($4) AND source_module IN ($5) AND status IN ($6)"
        );
        assert_eq!(filters.values.len(), 3);
    }

    #[test]
    fn typo_fallback_requires_meaningful_query_length() {
        assert!(!should_run_typo_fallback("tv"));
        assert!(!should_run_typo_fallback("red tv"));
        assert!(should_run_typo_fallback("iphnoe"));
        assert!(should_run_typo_fallback("samsnug phone"));
    }
}

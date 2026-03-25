use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};

use rustok_core::{Error, Result};

use crate::engine::{
    SearchConnectorDescriptor, SearchEngine, SearchEngineKind, SearchFacetBucket, SearchFacetGroup,
    SearchQuery, SearchResult, SearchResultItem,
};
pub struct PgSearchEngine {
    db: DatabaseConnection,
}

impl PgSearchEngine {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
                facets: empty_facets(),
            });
        }

        let started_at = std::time::Instant::now();
        let tenant_id = query.tenant_id.ok_or_else(|| {
            Error::Validation("search preview currently requires tenant_id".to_string())
        })?;
        let locale = query.locale.clone().unwrap_or_default();
        let limit = query.limit.clamp(1, 50) as i64;
        let offset = query.offset as i64;
        let filters = build_filter_clause(&query, 4);
        let cte = r#"
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
                    ts_rank_cd(sd.search_vector, q.ts_query) AS score,
                    sd.payload AS payload,
                    sd.is_public AS is_public,
                    sd.updated_at AS updated_at
                FROM search_documents sd
                CROSS JOIN q
                WHERE sd.tenant_id = $1
                  AND ($2 = '' OR sd.locale = $2)
                  AND sd.search_vector @@ q.ts_query
            )
        "#;

        let total_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "{cte} SELECT COUNT(*) AS total FROM ranked WHERE {}",
                filters.clause
            ),
            build_base_values(tenant_id, &locale, &trimmed_query, &filters.values),
        );
        let total = self
            .db
            .query_one(total_statement)
            .await
            .map_err(Error::Database)?
            .and_then(|row| row.try_get::<i64>("", "total").ok())
            .unwrap_or(0)
            .max(0) as u64;

        let offset_param = 4 + filters.values.len();
        let limit_param = offset_param + 1;
        let items_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "{cte}
                 SELECT id, entity_type, source_module, locale, title, snippet, score, payload
                 FROM ranked
                 WHERE {}
                 ORDER BY score DESC, updated_at DESC
                 OFFSET ${offset_param}
                 LIMIT ${limit_param}",
                filters.clause
            ),
            build_paged_values(
                tenant_id,
                &locale,
                &trimmed_query,
                &filters.values,
                offset,
                limit,
            ),
        );
        let items = self
            .db
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
            build_base_values(tenant_id, &locale, &trimmed_query, &filters.values),
        );
        let facets = build_facets(
            self.db
                .query_all(facets_statement)
                .await
                .map_err(Error::Database)?,
        )?;

        Ok(SearchResult {
            items,
            total,
            took_ms: started_at.elapsed().as_millis() as u64,
            engine: SearchEngineKind::Postgres,
            facets,
        })
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

    let clause = if clauses.is_empty() {
        "TRUE".to_string()
    } else {
        clauses.join(" AND ")
    };

    FilterClause { clause, values }
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

fn build_paged_values(
    tenant_id: uuid::Uuid,
    locale: &str,
    trimmed_query: &str,
    filter_values: &[Value],
    offset: i64,
    limit: i64,
) -> Vec<Value> {
    let mut values = build_base_values(tenant_id, locale, trimmed_query, filter_values);
    values.push(offset.into());
    values.push(limit.into());
    values
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
    use super::build_filter_clause;
    use crate::engine::SearchQuery;

    #[test]
    fn filter_clause_uses_bound_parameters() {
        let filters = build_filter_clause(
            &SearchQuery {
                tenant_id: None,
                locale: None,
                original_query: "phone".to_string(),
                query: "phone".to_string(),
                limit: 10,
                offset: 0,
                published_only: true,
                entity_types: vec!["product".to_string()],
                source_modules: vec!["commerce".to_string()],
                statuses: vec!["active".to_string()],
            },
            4,
        );

        assert_eq!(
            filters.clause,
            "is_public = TRUE AND entity_type IN ($4) AND source_module IN ($5) AND status IN ($6)"
        );
        assert_eq!(filters.values.len(), 3);
    }
}

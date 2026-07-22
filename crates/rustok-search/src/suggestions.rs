use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use std::collections::HashSet;
use uuid::Uuid;

use rustok_core::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSuggestionKind {
    Query,
    Document,
}

impl SearchSuggestionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Document => "document",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchSuggestionQuery {
    pub tenant_id: Uuid,
    pub query: String,
    pub locale: Option<String>,
    pub limit: usize,
    pub published_only: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchSuggestion {
    pub text: String,
    pub kind: SearchSuggestionKind,
    pub document_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub source_module: Option<String>,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub score: f64,
}

pub struct SearchSuggestionService;

impl SearchSuggestionService {
    pub async fn suggestions(
        db: &DatabaseConnection,
        query: SearchSuggestionQuery,
    ) -> Result<Vec<SearchSuggestion>> {
        ensure_postgres(db)?;

        let normalized_query = normalize_suggestion_query(&query.query);
        if normalized_query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = query.limit.clamp(1, 10);
        let query_rows = fetch_query_suggestions(
            db,
            query.tenant_id,
            &normalized_query,
            query.locale.as_deref(),
            query.published_only,
            limit,
        )
        .await?;
        let document_rows = fetch_document_suggestions(
            db,
            query.tenant_id,
            &normalized_query,
            query.locale.as_deref(),
            query.published_only,
            limit,
        )
        .await?;

        Ok(merge_suggestions(query_rows, document_rows, limit))
    }
}

async fn fetch_query_suggestions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    normalized_query: &str,
    locale: Option<&str>,
    published_only: bool,
    limit: usize,
) -> Result<Vec<SearchSuggestion>> {
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT
            query_text,
            locale,
            COUNT(*)::bigint AS hits,
            MAX(created_at) AS last_seen_at
        FROM search_query_logs
        WHERE tenant_id = $1
          AND status = 'success'
          AND ($3 = FALSE OR surface = 'storefront_search')
          AND query_normalized LIKE $2
          AND ($4 = '' OR locale IS NULL OR locale = $4)
        GROUP BY query_text, locale
        ORDER BY hits DESC, last_seen_at DESC, query_text ASC
        LIMIT $5
        "#,
        vec![
            tenant_id.into(),
            format!("{normalized_query}%").into(),
            published_only.into(),
            locale.unwrap_or("").to_string().into(),
            (limit as i64).into(),
        ],
    );

    db.query_all(stmt)
        .await
        .map_err(Error::Database)?
        .into_iter()
        .map(|row| {
            let text = row
                .try_get::<String>("", "query_text")
                .map_err(Error::Database)?;
            let locale = row
                .try_get::<Option<String>>("", "locale")
                .map_err(Error::Database)?;
            let hits = row.try_get::<i64>("", "hits").map_err(Error::Database)?;

            Ok(SearchSuggestion {
                text,
                kind: SearchSuggestionKind::Query,
                document_id: None,
                entity_type: None,
                source_module: None,
                locale,
                url: None,
                score: hits.max(0) as f64,
            })
        })
        .collect()
}

async fn fetch_document_suggestions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    normalized_query: &str,
    locale: Option<&str>,
    published_only: bool,
    limit: usize,
) -> Result<Vec<SearchSuggestion>> {
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT
            document_id,
            entity_type,
            source_module,
            title,
            locale,
            CASE
                WHEN lower(title) = $2 THEN 500.0
                WHEN lower(title) LIKE $3 THEN 320.0
                WHEN lower(COALESCE(slug, '')) LIKE $3 THEN 250.0
                WHEN lower(COALESCE(handle, '')) LIKE $3 THEN 220.0
                WHEN lower(title) LIKE $4 THEN 120.0
                ELSE 80.0
            END AS suggestion_score
        FROM search_documents
        WHERE tenant_id = $1
          AND ($5 = '' OR locale = $5)
          AND ($6 = FALSE OR is_public = TRUE)
          AND (
              lower(title) LIKE $4
              OR lower(COALESCE(slug, '')) LIKE $4
              OR lower(COALESCE(handle, '')) LIKE $4
          )
        ORDER BY suggestion_score DESC, updated_at DESC, title ASC
        LIMIT $7
        "#,
        vec![
            tenant_id.into(),
            normalized_query.to_string().into(),
            format!("{normalized_query}%").into(),
            format!("%{normalized_query}%").into(),
            locale.unwrap_or("").to_string().into(),
            published_only.into(),
            (limit as i64).into(),
        ],
    );

    db.query_all(stmt)
        .await
        .map_err(Error::Database)?
        .into_iter()
        .map(map_document_suggestion)
        .collect()
}

fn map_document_suggestion(row: QueryResult) -> Result<SearchSuggestion> {
    let document_id = row
        .try_get::<Uuid>("", "document_id")
        .map_err(Error::Database)?;
    let entity_type = row
        .try_get::<String>("", "entity_type")
        .map_err(Error::Database)?;
    let source_module = row
        .try_get::<String>("", "source_module")
        .map_err(Error::Database)?;
    let title = row
        .try_get::<String>("", "title")
        .map_err(Error::Database)?;
    let locale = row
        .try_get::<String>("", "locale")
        .map(Some)
        .map_err(Error::Database)?;
    let score = row
        .try_get::<f64>("", "suggestion_score")
        .or_else(|_| {
            row.try_get::<f32>("", "suggestion_score")
                .map(|value| value as f64)
        })
        .map_err(Error::Database)?;

    Ok(SearchSuggestion {
        text: title,
        kind: SearchSuggestionKind::Document,
        document_id: Some(document_id),
        entity_type: Some(entity_type.clone()),
        source_module: Some(source_module.clone()),
        locale,
        url: derive_document_url(document_id, &entity_type, &source_module),
        score,
    })
}

fn merge_suggestions(
    query_rows: Vec<SearchSuggestion>,
    document_rows: Vec<SearchSuggestion>,
    limit: usize,
) -> Vec<SearchSuggestion> {
    let mut seen = HashSet::new();
    let mut merged = Vec::with_capacity(limit);

    for suggestion in query_rows.into_iter().chain(document_rows) {
        let dedupe_key = suggestion.text.trim().to_ascii_lowercase();
        if dedupe_key.is_empty() || !seen.insert(dedupe_key) {
            continue;
        }

        merged.push(suggestion);
        if merged.len() >= limit {
            break;
        }
    }

    merged
}

fn derive_document_url(
    document_id: Uuid,
    entity_type: &str,
    source_module: &str,
) -> Option<String> {
    match entity_type {
        "product" => Some(format!("/store/products/{document_id}")),
        "node" => Some(format!(
            "/modules/content?id={document_id}{}",
            if source_module.is_empty() || source_module == "content" {
                String::new()
            } else {
                format!("&kind={source_module}")
            }
        )),
        _ => None,
    }
}

fn normalize_suggestion_query(value: &str) -> String {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(Error::External(
            "SearchSuggestionService requires PostgreSQL backend".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SearchSuggestion, SearchSuggestionKind, merge_suggestions, normalize_suggestion_query,
    };

    #[test]
    fn normalize_suggestion_query_collapses_whitespace() {
        assert_eq!(
            normalize_suggestion_query("  Summer   Shoes "),
            "summer shoes".to_string()
        );
    }

    #[test]
    fn merge_suggestions_deduplicates_by_text() {
        let merged = merge_suggestions(
            vec![SearchSuggestion {
                text: "Summer Shoes".to_string(),
                kind: SearchSuggestionKind::Query,
                document_id: None,
                entity_type: None,
                source_module: None,
                locale: None,
                url: None,
                score: 10.0,
            }],
            vec![SearchSuggestion {
                text: "summer shoes".to_string(),
                kind: SearchSuggestionKind::Document,
                document_id: None,
                entity_type: Some("product".to_string()),
                source_module: Some("commerce".to_string()),
                locale: None,
                url: Some("/store/products/1".to_string()),
                score: 1.0,
            }],
            10,
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind, SearchSuggestionKind::Query);
    }
}

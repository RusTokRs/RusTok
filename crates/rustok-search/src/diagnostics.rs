use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_core::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchDiagnosticsSnapshot {
    pub tenant_id: Uuid,
    pub total_documents: u64,
    pub public_documents: u64,
    pub content_documents: u64,
    pub product_documents: u64,
    pub stale_documents: u64,
    pub newest_indexed_at: Option<DateTime<Utc>>,
    pub oldest_indexed_at: Option<DateTime<Utc>>,
    pub max_lag_seconds: u64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaggingSearchDocument {
    pub document_key: String,
    pub document_id: Uuid,
    pub source_module: String,
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub is_public: bool,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub indexed_at: DateTime<Utc>,
    pub lag_seconds: u64,
}

pub struct SearchDiagnosticsService;

impl SearchDiagnosticsService {
    pub async fn snapshot(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<SearchDiagnosticsSnapshot> {
        if db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "SearchDiagnosticsService requires PostgreSQL backend".to_string(),
            ));
        }

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                COUNT(*)::bigint AS total_documents,
                COUNT(*) FILTER (WHERE is_public)::bigint AS public_documents,
                COUNT(*) FILTER (WHERE entity_type = 'node')::bigint AS content_documents,
                COUNT(*) FILTER (WHERE entity_type = 'product')::bigint AS product_documents,
                COUNT(*) FILTER (WHERE indexed_at < updated_at)::bigint AS stale_documents,
                MAX(indexed_at) AS newest_indexed_at,
                MIN(indexed_at) AS oldest_indexed_at,
                COALESCE(MAX(GREATEST(EXTRACT(EPOCH FROM (updated_at - indexed_at)), 0)), 0)::bigint AS max_lag_seconds,
                EXISTS(
                    SELECT 1
                    FROM nodes n
                    JOIN node_translations nt ON nt.node_id = n.id
                    WHERE n.tenant_id = $1
                      AND n.deleted_at IS NULL
                ) AS has_content_sources,
                EXISTS(
                    SELECT 1
                    FROM products p
                    JOIN product_translations pt ON pt.product_id = p.id
                    WHERE p.tenant_id = $1
                ) AS has_product_sources
            FROM search_documents
            WHERE tenant_id = $1
            "#,
            vec![tenant_id.into()],
        );

        let row = db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search diagnostics row".to_string()))?;

        let total_documents = row
            .try_get::<i64>("", "total_documents")
            .map_err(Error::Database)?
            .max(0) as u64;
        let stale_documents = row
            .try_get::<i64>("", "stale_documents")
            .map_err(Error::Database)?
            .max(0) as u64;
        let max_lag_seconds = row
            .try_get::<i64>("", "max_lag_seconds")
            .map_err(Error::Database)?
            .max(0) as u64;
        let has_indexable_sources = row
            .try_get::<bool>("", "has_content_sources")
            .map_err(Error::Database)?
            || row
                .try_get::<bool>("", "has_product_sources")
                .map_err(Error::Database)?;

        let state = compute_diagnostics_state(
            total_documents,
            stale_documents,
            max_lag_seconds,
            has_indexable_sources,
        )
        .to_string();

        Ok(SearchDiagnosticsSnapshot {
            tenant_id,
            total_documents,
            public_documents: row
                .try_get::<i64>("", "public_documents")
                .map_err(Error::Database)?
                .max(0) as u64,
            content_documents: row
                .try_get::<i64>("", "content_documents")
                .map_err(Error::Database)?
                .max(0) as u64,
            product_documents: row
                .try_get::<i64>("", "product_documents")
                .map_err(Error::Database)?
                .max(0) as u64,
            stale_documents,
            newest_indexed_at: row
                .try_get::<Option<DateTime<Utc>>>("", "newest_indexed_at")
                .map_err(Error::Database)?,
            oldest_indexed_at: row
                .try_get::<Option<DateTime<Utc>>>("", "oldest_indexed_at")
                .map_err(Error::Database)?,
            max_lag_seconds,
            state,
        })
    }

    pub async fn lagging_documents(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: usize,
    ) -> Result<Vec<LaggingSearchDocument>> {
        if db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "SearchDiagnosticsService requires PostgreSQL backend".to_string(),
            ));
        }

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                document_key,
                document_id,
                source_module,
                entity_type,
                locale,
                status,
                is_public,
                title,
                updated_at,
                indexed_at,
                GREATEST(EXTRACT(EPOCH FROM (updated_at - indexed_at)), 0)::bigint AS lag_seconds
            FROM search_documents
            WHERE tenant_id = $1
              AND indexed_at < updated_at
            ORDER BY lag_seconds DESC, updated_at DESC
            LIMIT $2
            "#,
            vec![tenant_id.into(), (limit.clamp(1, 100) as i64).into()],
        );

        let rows = db.query_all(stmt).await.map_err(Error::Database)?;
        rows.into_iter()
            .map(|row| {
                Ok(LaggingSearchDocument {
                    document_key: row
                        .try_get::<String>("", "document_key")
                        .map_err(Error::Database)?,
                    document_id: row.try_get("", "document_id").map_err(Error::Database)?,
                    source_module: row
                        .try_get::<String>("", "source_module")
                        .map_err(Error::Database)?,
                    entity_type: row
                        .try_get::<String>("", "entity_type")
                        .map_err(Error::Database)?,
                    locale: row
                        .try_get::<String>("", "locale")
                        .map_err(Error::Database)?,
                    status: row
                        .try_get::<String>("", "status")
                        .map_err(Error::Database)?,
                    is_public: row
                        .try_get::<bool>("", "is_public")
                        .map_err(Error::Database)?,
                    title: row
                        .try_get::<String>("", "title")
                        .map_err(Error::Database)?,
                    updated_at: row
                        .try_get::<DateTime<Utc>>("", "updated_at")
                        .map_err(Error::Database)?,
                    indexed_at: row
                        .try_get::<DateTime<Utc>>("", "indexed_at")
                        .map_err(Error::Database)?,
                    lag_seconds: row
                        .try_get::<i64>("", "lag_seconds")
                        .map_err(Error::Database)?
                        .max(0) as u64,
                })
            })
            .collect()
    }
}

fn compute_diagnostics_state(
    total_documents: u64,
    stale_documents: u64,
    max_lag_seconds: u64,
    has_indexable_sources: bool,
) -> &'static str {
    if total_documents == 0 && has_indexable_sources {
        "bootstrap_pending"
    } else if stale_documents > 0 || max_lag_seconds > 300 {
        "lagging"
    } else {
        "healthy"
    }
}

#[cfg(test)]
mod tests {
    use super::compute_diagnostics_state;

    #[test]
    fn empty_tenant_without_sources_is_healthy() {
        assert_eq!(compute_diagnostics_state(0, 0, 0, false), "healthy");
    }

    #[test]
    fn empty_tenant_with_sources_is_bootstrap_pending() {
        assert_eq!(
            compute_diagnostics_state(0, 0, 0, true),
            "bootstrap_pending"
        );
    }
}

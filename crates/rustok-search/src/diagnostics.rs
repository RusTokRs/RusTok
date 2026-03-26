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
    pub missing_documents: u64,
    pub orphaned_documents: u64,
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

#[derive(Debug, Clone, PartialEq)]
pub struct SearchConsistencyIssue {
    pub issue_kind: String,
    pub document_key: String,
    pub document_id: Uuid,
    pub source_module: String,
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub indexed_at: Option<DateTime<Utc>>,
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
                (
                    SELECT COUNT(*)::bigint
                    FROM nodes n
                    JOIN node_translations nt ON nt.node_id = n.id
                    LEFT JOIN search_documents sd
                        ON sd.tenant_id = n.tenant_id
                       AND sd.entity_type = 'node'
                       AND sd.document_id = n.id
                       AND sd.locale = nt.locale
                    WHERE n.tenant_id = $1
                      AND n.deleted_at IS NULL
                      AND sd.document_key IS NULL
                ) AS missing_content_documents,
                (
                    SELECT COUNT(*)::bigint
                    FROM products p
                    JOIN product_translations pt ON pt.product_id = p.id
                    LEFT JOIN search_documents sd
                        ON sd.tenant_id = p.tenant_id
                       AND sd.entity_type = 'product'
                       AND sd.document_id = p.id
                       AND sd.locale = pt.locale
                    WHERE p.tenant_id = $1
                      AND sd.document_key IS NULL
                ) AS missing_product_documents,
                (
                    SELECT COUNT(*)::bigint
                    FROM search_documents sd
                    LEFT JOIN nodes n
                        ON n.id = sd.document_id
                       AND n.tenant_id = sd.tenant_id
                       AND n.deleted_at IS NULL
                    LEFT JOIN node_translations nt
                        ON nt.node_id = sd.document_id
                       AND nt.locale = sd.locale
                    WHERE sd.tenant_id = $1
                      AND sd.entity_type = 'node'
                      AND (n.id IS NULL OR nt.id IS NULL)
                ) AS orphaned_content_documents,
                (
                    SELECT COUNT(*)::bigint
                    FROM search_documents sd
                    LEFT JOIN products p
                        ON p.id = sd.document_id
                       AND p.tenant_id = sd.tenant_id
                    LEFT JOIN product_translations pt
                        ON pt.product_id = sd.document_id
                       AND pt.locale = sd.locale
                    WHERE sd.tenant_id = $1
                      AND sd.entity_type = 'product'
                      AND (p.id IS NULL OR pt.id IS NULL)
                ) AS orphaned_product_documents,
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
        let missing_documents = row
            .try_get::<i64>("", "missing_content_documents")
            .map_err(Error::Database)?
            .max(0) as u64
            + row
                .try_get::<i64>("", "missing_product_documents")
                .map_err(Error::Database)?
                .max(0) as u64;
        let orphaned_documents = row
            .try_get::<i64>("", "orphaned_content_documents")
            .map_err(Error::Database)?
            .max(0) as u64
            + row
                .try_get::<i64>("", "orphaned_product_documents")
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
            missing_documents,
            orphaned_documents,
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
            missing_documents,
            orphaned_documents,
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

    pub async fn consistency_issues(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: usize,
    ) -> Result<Vec<SearchConsistencyIssue>> {
        if db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "SearchDiagnosticsService requires PostgreSQL backend".to_string(),
            ));
        }

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                issue_kind,
                document_key,
                document_id,
                source_module,
                entity_type,
                locale,
                status,
                title,
                updated_at,
                indexed_at
            FROM (
                SELECT
                    'missing'::text AS issue_kind,
                    CONCAT('node:', n.id::text, ':', nt.locale) AS document_key,
                    n.id AS document_id,
                    COALESCE(NULLIF(n.kind::text, ''), 'content') AS source_module,
                    'node'::text AS entity_type,
                    nt.locale,
                    n.status::text AS status,
                    COALESCE(nt.title, '') AS title,
                    n.updated_at,
                    NULL::timestamptz AS indexed_at
                FROM nodes n
                JOIN node_translations nt ON nt.node_id = n.id
                LEFT JOIN search_documents sd
                    ON sd.tenant_id = n.tenant_id
                   AND sd.entity_type = 'node'
                   AND sd.document_id = n.id
                   AND sd.locale = nt.locale
                WHERE n.tenant_id = $1
                  AND n.deleted_at IS NULL
                  AND sd.document_key IS NULL

                UNION ALL

                SELECT
                    'missing'::text AS issue_kind,
                    CONCAT('product:', p.id::text, ':', pt.locale) AS document_key,
                    p.id AS document_id,
                    'commerce'::text AS source_module,
                    'product'::text AS entity_type,
                    pt.locale,
                    p.status::text AS status,
                    pt.title,
                    p.updated_at,
                    NULL::timestamptz AS indexed_at
                FROM products p
                JOIN product_translations pt ON pt.product_id = p.id
                LEFT JOIN search_documents sd
                    ON sd.tenant_id = p.tenant_id
                   AND sd.entity_type = 'product'
                   AND sd.document_id = p.id
                   AND sd.locale = pt.locale
                WHERE p.tenant_id = $1
                  AND sd.document_key IS NULL

                UNION ALL

                SELECT
                    'orphaned'::text AS issue_kind,
                    sd.document_key,
                    sd.document_id,
                    sd.source_module,
                    sd.entity_type,
                    sd.locale,
                    sd.status,
                    sd.title,
                    sd.updated_at,
                    sd.indexed_at
                FROM search_documents sd
                LEFT JOIN nodes n
                    ON n.id = sd.document_id
                   AND n.tenant_id = sd.tenant_id
                   AND n.deleted_at IS NULL
                LEFT JOIN node_translations nt
                    ON nt.node_id = sd.document_id
                   AND nt.locale = sd.locale
                WHERE sd.tenant_id = $1
                  AND sd.entity_type = 'node'
                  AND (n.id IS NULL OR nt.id IS NULL)

                UNION ALL

                SELECT
                    'orphaned'::text AS issue_kind,
                    sd.document_key,
                    sd.document_id,
                    sd.source_module,
                    sd.entity_type,
                    sd.locale,
                    sd.status,
                    sd.title,
                    sd.updated_at,
                    sd.indexed_at
                FROM search_documents sd
                LEFT JOIN products p
                    ON p.id = sd.document_id
                   AND p.tenant_id = sd.tenant_id
                LEFT JOIN product_translations pt
                    ON pt.product_id = sd.document_id
                   AND pt.locale = sd.locale
                WHERE sd.tenant_id = $1
                  AND sd.entity_type = 'product'
                  AND (p.id IS NULL OR pt.id IS NULL)
            ) issues
            ORDER BY updated_at DESC, issue_kind ASC, document_key ASC
            LIMIT $2
            "#,
            vec![tenant_id.into(), (limit.clamp(1, 100) as i64).into()],
        );

        let rows = db.query_all(stmt).await.map_err(Error::Database)?;
        rows.into_iter()
            .map(|row| {
                Ok(SearchConsistencyIssue {
                    issue_kind: row
                        .try_get::<String>("", "issue_kind")
                        .map_err(Error::Database)?,
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
                    title: row
                        .try_get::<String>("", "title")
                        .map_err(Error::Database)?,
                    updated_at: row
                        .try_get::<DateTime<Utc>>("", "updated_at")
                        .map_err(Error::Database)?,
                    indexed_at: row
                        .try_get::<Option<DateTime<Utc>>>("", "indexed_at")
                        .map_err(Error::Database)?,
                })
            })
            .collect()
    }
}

fn compute_diagnostics_state(
    total_documents: u64,
    stale_documents: u64,
    missing_documents: u64,
    orphaned_documents: u64,
    max_lag_seconds: u64,
    has_indexable_sources: bool,
) -> &'static str {
    if total_documents == 0 && has_indexable_sources {
        "bootstrap_pending"
    } else if missing_documents > 0 || orphaned_documents > 0 {
        "inconsistent"
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
        assert_eq!(compute_diagnostics_state(0, 0, 0, 0, 0, false), "healthy");
    }

    #[test]
    fn empty_tenant_with_sources_is_bootstrap_pending() {
        assert_eq!(
            compute_diagnostics_state(0, 0, 3, 0, 0, true),
            "bootstrap_pending"
        );
    }

    #[test]
    fn missing_or_orphaned_documents_are_inconsistent() {
        assert_eq!(
            compute_diagnostics_state(4, 0, 1, 0, 0, true),
            "inconsistent"
        );
        assert_eq!(
            compute_diagnostics_state(4, 0, 0, 2, 0, true),
            "inconsistent"
        );
    }

    #[test]
    fn inconsistent_takes_precedence_over_lagging() {
        assert_eq!(
            compute_diagnostics_state(4, 2, 1, 0, 900, true),
            "inconsistent"
        );
    }

    #[test]
    fn lagging_state_covers_stale_documents_without_consistency_issues() {
        assert_eq!(compute_diagnostics_state(4, 2, 0, 0, 45, true), "lagging");
    }
}

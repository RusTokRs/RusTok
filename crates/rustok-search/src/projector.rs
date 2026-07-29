use std::time::Instant;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

use crate::projector_legacy;

const CORE_SCOPE_COUNT_SQL: &str = r#"
SELECT COUNT(*) AS total
FROM search_documents
WHERE tenant_id = $1
  AND entity_type IN ('node', 'product')
"#;

/// Search-owned projector facade.
///
/// Tenant rebuilds replace only the direct `node` and `product` scopes owned by
/// this projector. External Blog, Forum and future registered projection scopes
/// remain untouched until their own projectors complete an atomic replacement.
/// This prevents an early core rebuild commit from deleting the previous value
/// of a later source when that source subsequently fails.
#[derive(Clone)]
pub struct SearchProjector {
    db: DatabaseConnection,
    legacy: projector_legacy::SearchProjector,
}

impl SearchProjector {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: projector_legacy::SearchProjector::new(db.clone()),
            db,
        }
    }

    /// Bootstraps the scopes owned by this projector even when an external source
    /// has already populated the shared Search document store.
    pub async fn ensure_bootstrap(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            CORE_SCOPE_COUNT_SQL,
            vec![tenant_id.into()],
        );
        let total = self
            .db
            .query_one(statement)
            .await
            .map_err(Error::Database)?
            .and_then(|row| row.try_get::<i64>("", "total").ok())
            .unwrap_or(0);
        if total == 0 {
            self.rebuild_tenant(tenant_id).await?;
        }
        Ok(())
    }

    /// Replaces the direct Search-owned scopes without deleting documents owned
    /// by later Blog, Forum or future projection-source stages.
    ///
    /// Content and product replacements keep their existing per-scope database
    /// transactions. A later scope failure therefore leaves every external scope
    /// at its previous committed value and leaves the failed scope rolled back.
    pub async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        let started_at = Instant::now();
        let result = async {
            self.legacy.rebuild_content_scope(tenant_id).await?;
            self.legacy.rebuild_product_scope(tenant_id).await
        }
        .await;
        record_scope_preserving_rebuild(tenant_id, &result, started_at);
        result
    }

    pub async fn rebuild_content_scope(&self, tenant_id: Uuid) -> Result<()> {
        self.legacy.rebuild_content_scope(tenant_id).await
    }

    pub async fn rebuild_product_scope(&self, tenant_id: Uuid) -> Result<()> {
        self.legacy.rebuild_product_scope(tenant_id).await
    }

    pub async fn upsert_node(&self, tenant_id: Uuid, node_id: Uuid) -> Result<()> {
        self.legacy.upsert_node(tenant_id, node_id).await
    }

    pub async fn upsert_node_locale(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        locale: &str,
    ) -> Result<()> {
        self.legacy
            .upsert_node_locale(tenant_id, node_id, locale)
            .await
    }

    pub async fn delete_node(&self, tenant_id: Uuid, node_id: Uuid) -> Result<()> {
        self.legacy.delete_node(tenant_id, node_id).await
    }

    pub async fn delete_node_locale(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        locale: &str,
    ) -> Result<()> {
        self.legacy
            .delete_node_locale(tenant_id, node_id, locale)
            .await
    }

    pub async fn reindex_category(&self, tenant_id: Uuid, category_id: Uuid) -> Result<()> {
        self.legacy.reindex_category(tenant_id, category_id).await
    }

    pub async fn upsert_product(&self, tenant_id: Uuid, product_id: Uuid) -> Result<()> {
        self.legacy.upsert_product(tenant_id, product_id).await
    }

    pub async fn delete_product(&self, tenant_id: Uuid, product_id: Uuid) -> Result<()> {
        self.legacy.delete_product(tenant_id, product_id).await
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "SearchProjector requires PostgreSQL backend".to_string(),
            ));
        }
        Ok(())
    }
}

fn record_scope_preserving_rebuild(tenant_id: Uuid, result: &Result<()>, started_at: Instant) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::record_search_indexing_operation(
        "rebuild_tenant_scoped",
        "tenant",
        status,
        started_at.elapsed().as_secs_f64(),
    );
    match result {
        Ok(()) => tracing::info!(
            tenant_id = %tenant_id,
            duration_ms = started_at.elapsed().as_millis() as u64,
            "Search-owned tenant scopes rebuilt without deleting external projections"
        ),
        Err(error) => tracing::error!(
            tenant_id = %tenant_id,
            error = %error,
            duration_ms = started_at.elapsed().as_millis() as u64,
            "Search-owned tenant scope rebuild failed"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::CORE_SCOPE_COUNT_SQL;

    #[test]
    fn bootstrap_count_is_limited_to_direct_search_scopes() {
        assert!(CORE_SCOPE_COUNT_SQL.contains("entity_type IN ('node', 'product')"));
        assert!(!CORE_SCOPE_COUNT_SQL.contains("blog_post"));
        assert!(!CORE_SCOPE_COUNT_SQL.contains("forum_category"));
        assert!(!CORE_SCOPE_COUNT_SQL.contains("forum_topic"));
    }
}

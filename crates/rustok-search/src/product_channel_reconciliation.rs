use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_core::{Error, Result};

use crate::SearchProjector;

pub const DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT: usize = 32;
const MAX_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT: usize = 256;

const LEGACY_PRODUCT_CHANNEL_TENANTS_SQL: &str = r#"
SELECT DISTINCT tenant_id
FROM search_documents
WHERE entity_type = 'product'
  AND payload #> '{channel_visibility,allowed_channel_slugs}' IS NULL
ORDER BY tenant_id
LIMIT $1
"#;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductChannelProjectionSweepReport {
    pub due_tenants: usize,
    pub rebuilt_tenants: usize,
}

#[derive(Clone)]
pub struct ProductChannelProjectionReconciler {
    db: DatabaseConnection,
    projector: SearchProjector,
}

impl ProductChannelProjectionReconciler {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            projector: SearchProjector::new(db.clone()),
            db,
        }
    }

    pub fn supports_background_reconciliation(&self) -> bool {
        self.db.get_database_backend() == DbBackend::Postgres
    }

    pub async fn sweep_due(
        &self,
        tenant_limit: usize,
    ) -> Result<ProductChannelProjectionSweepReport> {
        if !self.supports_background_reconciliation() {
            return Err(Error::External(
                "Product channel projection reconciliation requires PostgreSQL".to_string(),
            ));
        }

        let tenant_limit = tenant_limit.clamp(1, MAX_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT);
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            LEGACY_PRODUCT_CHANNEL_TENANTS_SQL,
            vec![(tenant_limit as i64).into()],
        );
        let tenant_ids = self
            .db
            .query_all_raw(statement)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(|row| {
                row.try_get::<Uuid>("", "tenant_id")
                    .map_err(Error::Database)
            })
            .collect::<Result<Vec<_>>>()?;

        let mut report = ProductChannelProjectionSweepReport {
            due_tenants: tenant_ids.len(),
            rebuilt_tenants: 0,
        };
        for tenant_id in tenant_ids {
            self.projector.rebuild_product_scope(tenant_id).await?;
            report.rebuilt_tenants += 1;
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::{LEGACY_PRODUCT_CHANNEL_TENANTS_SQL, MAX_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT};

    #[test]
    fn reconciliation_selects_only_missing_legacy_projection() {
        assert!(LEGACY_PRODUCT_CHANNEL_TENANTS_SQL.contains("entity_type = 'product'"));
        assert!(LEGACY_PRODUCT_CHANNEL_TENANTS_SQL.contains("allowed_channel_slugs"));
        assert!(LEGACY_PRODUCT_CHANNEL_TENANTS_SQL.contains("IS NULL"));
        assert!(!LEGACY_PRODUCT_CHANNEL_TENANTS_SQL.contains("IS DISTINCT FROM"));
    }

    #[test]
    fn tenant_batch_is_bounded() {
        assert_eq!(MAX_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT, 256);
    }
}

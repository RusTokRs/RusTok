//! Host-owned active-tenant enumeration for artifact durable delivery work.
//!
//! Artifact queues are tenant-RLS scoped and must never discover tenants through
//! unscoped queue queries. The server adapts the Tenant owner's active-ID
//! projection to module work registrations.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const TENANT_PAGE_SIZE: u64 = 100;

pub struct ServerArtifactDeliveryTenantSource {
    tenants: rustok_tenant::TenantService,
}

impl ServerArtifactDeliveryTenantSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tenants: rustok_tenant::TenantService::new(db),
        }
    }
}

#[async_trait]
impl rustok_modules::ArtifactDeliveryTenantSource for ServerArtifactDeliveryTenantSource {
    async fn tenant_ids(
        &self,
    ) -> Result<Vec<Uuid>, rustok_modules::ArtifactDeliveryTenantSourceError> {
        let mut after = None;
        let mut tenant_ids = Vec::new();
        loop {
            let batch = self
                .tenants
                .list_active_tenant_ids_after(after, TENANT_PAGE_SIZE)
                .await
                .map_err(|error| {
                    rustok_modules::ArtifactDeliveryTenantSourceError::Store(error.to_string())
                })?;
            let Some(last_id) = batch.last().copied() else {
                return Ok(tenant_ids);
            };
            tenant_ids.extend(batch);
            after = Some(last_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_modules::ArtifactDeliveryTenantSource;
    use rustok_tenant::entities::tenant;
    use sea_orm::{ActiveModelTrait, Set};

    #[tokio::test]
    async fn enumerator_crosses_page_boundary_and_excludes_inactive_tenants() {
        let db = rustok_test_utils::db::setup_test_db_with_migrations::<
            rustok_migrations::SqliteTestMigrator,
        >()
        .await;
        let mut expected = Vec::new();
        for i in 1..=102_u128 {
            let id = Uuid::from_u128(i);
            let mut model = tenant::ActiveModel::new(
                &format!("Artifact Tenant {i}"),
                &format!("artifact-tenant-{i}"),
            );
            model.id = Set(id);
            model.is_active = Set(i != 50);
            model.insert(&db).await.expect("insert tenant");
            if i != 50 {
                expected.push(id);
            }
        }

        let source = ServerArtifactDeliveryTenantSource::new(db);
        assert_eq!(source.tenant_ids().await.expect("enumerate"), expected);
    }
}

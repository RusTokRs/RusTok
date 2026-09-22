//! Host-owned active-tenant enumeration for artifact durable delivery work.
//!
//! Artifact queues are tenant-RLS scoped and must never discover tenants through
//! unscoped queue queries. The server owns the tenant projection and supplies
//! this handle to module work registrations.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const TENANT_PAGE_SIZE: u64 = 250;

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
    async fn tenant_ids(&self) -> Result<Vec<Uuid>, String> {
        let mut page = 1_u64;
        let mut scanned = 0_u64;
        let mut tenant_ids = Vec::new();
        loop {
            let (tenants, total) = self
                .tenants
                .list_tenants(page, TENANT_PAGE_SIZE)
                .await
                .map_err(|error| error.to_string())?;
            if tenants.is_empty() {
                return Ok(tenant_ids);
            }
            scanned = scanned.saturating_add(tenants.len() as u64);
            tenant_ids.extend(
                tenants
                    .into_iter()
                    .filter(|tenant| tenant.is_active)
                    .map(|tenant| tenant.id),
            );
            if scanned >= total {
                return Ok(tenant_ids);
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| "artifact delivery tenant pagination overflowed".to_string())?;
        }
    }
}

use sea_orm::DatabaseConnection;

use crate::error::Result;

use super::rbac_persistence::replace_user_role_via_store;
use super::rbac_service::RbacService;

impl RbacService {
    /// Replace a role outside an enclosing transaction and invalidate the
    /// process-local authorization snapshot only after persistence succeeds.
    pub async fn replace_user_role_committed(
        db: &DatabaseConnection,
        user_id: &uuid::Uuid,
        tenant_id: &uuid::Uuid,
        role: rustok_core::UserRole,
    ) -> Result<()> {
        Self::record_committed_mutation_entrypoint();
        replace_user_role_via_store(db, user_id, tenant_id, role).await?;
        Self::invalidate_user_rbac_caches(tenant_id, user_id).await;
        Ok(())
    }

    fn record_committed_mutation_entrypoint() {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "rbac",
            "replace_user_role_committed",
            "library",
        );
    }
}
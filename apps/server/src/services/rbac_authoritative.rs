use crate::error::Result;
use rustok_api::Permission;
use rustok_rbac::PermissionResolver;
use sea_orm::DatabaseConnection;

use super::rbac_runtime::resolver as rbac_runtime_resolver;
use super::rbac_service::RbacService;

impl RbacService {
    /// Resolve the canonical database-backed permission snapshot.
    ///
    /// Authentication uses this method before deriving the principal's
    /// canonical role. Authorization entry points must use
    /// `get_user_permissions`, which honors the immutable request scope and
    /// therefore cannot restore permissions removed by OAuth scopes.
    pub async fn get_user_permissions_authoritative(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>> {
        let resolver = rbac_runtime_resolver(db);
        let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;
        Ok(resolved.permissions)
    }
}
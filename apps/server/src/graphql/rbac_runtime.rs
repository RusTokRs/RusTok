use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::UserRole;
use rustok_rbac::graphql::{RbacGraphqlRoleWriter, RbacGraphqlRoleWriterHandle};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::users;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

struct ServerRbacGraphqlRoleWriter {
    db: DatabaseConnection,
}

#[async_trait]
impl RbacGraphqlRoleWriter for ServerRbacGraphqlRoleWriter {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), String> {
        let target_exists = users::Entity::find_by_id(*user_id)
            .filter(users::Column::TenantId.eq(*tenant_id))
            .one(&self.db)
            .await
            .map_err(|err| err.to_string())?
            .is_some();

        if !target_exists {
            return Err("target user not found in tenant".to_string());
        }

        RbacService::replace_user_role_committed(&self.db, user_id, tenant_id, role)
            .await
            .map_err(|err| err.to_string())
    }
}

pub fn rbac_graphql_role_writer_from_context(
    ctx: &ServerRuntimeContext,
) -> RbacGraphqlRoleWriterHandle {
    RbacGraphqlRoleWriterHandle(Arc::new(ServerRbacGraphqlRoleWriter { db: ctx.db_clone() }))
}

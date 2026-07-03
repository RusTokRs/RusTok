use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::UserRole;
use rustok_rbac::graphql::{RbacGraphqlRoleWriter, RbacGraphqlRoleWriterHandle};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

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
        RbacService::replace_user_role(&self.db, user_id, tenant_id, role)
            .await
            .map_err(|err| err.to_string())
    }
}

pub fn rbac_graphql_role_writer_from_context(
    ctx: &ServerRuntimeContext,
) -> RbacGraphqlRoleWriterHandle {
    RbacGraphqlRoleWriterHandle(Arc::new(ServerRbacGraphqlRoleWriter { db: ctx.db_clone() }))
}

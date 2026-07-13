use async_trait::async_trait;
use rustok_core::UserRole;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait RbacGraphqlRoleWriter: Send + Sync {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), String>;
}

#[derive(Clone)]
pub struct RbacGraphqlRoleWriterHandle(pub Arc<dyn RbacGraphqlRoleWriter>);

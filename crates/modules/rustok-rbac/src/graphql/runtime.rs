use async_trait::async_trait;
use rustok_core::UserRole;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RbacGraphqlRoleWriteError {
    #[error("permission denied: {0}")]
    Forbidden(String),
    #[error("role assignment target not found: {0}")]
    NotFound(String),
    #[error("role assignment conflict: {0}")]
    Conflict(String),
    #[error("role assignment failed: {0}")]
    Internal(String),
}

#[async_trait]
pub trait RbacGraphqlRoleWriter: Send + Sync {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), RbacGraphqlRoleWriteError>;
}

#[derive(Clone)]
pub struct RbacGraphqlRoleWriterHandle(pub Arc<dyn RbacGraphqlRoleWriter>);

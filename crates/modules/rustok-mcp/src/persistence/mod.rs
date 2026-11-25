//! Persisted MCP state and the host capabilities it uses.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::Permission;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{AlloyScaffoldError, ApplyModuleScaffoldResponse, StagedModuleScaffold};

mod authority;
mod management;
mod models;
mod runtime;
mod service;

#[derive(Debug, thiserror::Error)]
pub enum McpPersistenceError {
    #[error(transparent)]
    Authority(#[from] crate::McpManagementAuthorityError),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("Resource not found")]
    NotFound,
    #[error("Internal server error")]
    InternalServerError,
    #[error("{0}")]
    Message(String),
}

type Result<T> = std::result::Result<T, McpPersistenceError>;

pub enum McpDelegatedPrincipal {
    Missing,
    Inactive,
    Active(Vec<Permission>),
}

/// Dependencies resolved by the composition root for a persisted MCP owner.
#[async_trait]
pub trait McpHostServices: Send + Sync {
    fn request_permissions(&self, tenant_id: Uuid, actor_id: Uuid) -> Option<Vec<Permission>>;

    async fn delegated_principal(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<McpDelegatedPrincipal>;

    async fn active_permissions(
        &self,
        tenant_id: Uuid,
        permissions: Vec<Permission>,
    ) -> Result<Vec<Permission>>;

    /// Authorize and lock the workspace before a draft can be claimed.
    async fn prepare_scaffold_workspace(
        &self,
        requested_root: &str,
    ) -> Result<Box<dyn McpScaffoldPublication>>;
}

/// An authorized workspace and its held publication locks.
#[async_trait]
pub trait McpScaffoldPublication: Send + Sync {
    fn root(&self) -> &str;
    fn actor_id(&self) -> Uuid;
    async fn validate_target(&self, draft: &StagedModuleScaffold) -> Result<()>;
    fn publish(
        self: Box<Self>,
        draft: &StagedModuleScaffold,
    ) -> std::result::Result<ApplyModuleScaffoldResponse, AlloyScaffoldError>;
}

pub struct McpPersistence {
    db: DatabaseConnection,
    host: Arc<dyn McpHostServices>,
}

impl McpPersistence {
    pub fn new(db: DatabaseConnection, host: Arc<dyn McpHostServices>) -> Self {
        Self { db, host }
    }
}

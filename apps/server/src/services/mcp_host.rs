use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_mcp::persistence::{
    McpDelegatedPrincipal, McpHostServices, McpPersistence, McpPersistenceError,
    McpScaffoldPublication,
};
use rustok_mcp::{
    AlloyScaffoldError, ApplyModuleScaffoldResponse, McpManagementRuntime, StagedModuleScaffold,
};
use rustok_modules::ModuleEffectivePolicyCache;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::mcp_scaffold_workspace::{
    McpScaffoldWorkspaceLock, apply_mcp_scaffold_atomically, authorize_mcp_scaffold_workspace,
    lock_mcp_scaffold_workspace, validate_mcp_scaffold_target_path,
};
use super::rbac_module_availability::{permission_ownership, resolve_permission_availability};
use super::rbac_service::RbacService;
use super::server_runtime_context::ServerRuntimeContext;
use crate::error::{Error, Result};
use crate::models::users;

type OwnerResult<T> = std::result::Result<T, McpPersistenceError>;

pub(super) fn compose(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
) -> Result<Arc<McpPersistence>> {
    let db = ctx.db_clone();
    let host = Arc::new(ServerMcpHost {
        db: db.clone(),
        registry: registry.clone(),
        policy_cache: ctx.effective_policy_cache(),
        permission_owners: permission_ownership(ctx, registry)?,
    });
    Ok(Arc::new(McpPersistence::new(db, host)))
}

fn runtime_extensions(ctx: &ServerRuntimeContext) -> Result<Arc<ModuleRuntimeExtensions>> {
    ctx.shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("MCP runtime extensions are not registered".to_string()))
}

pub(crate) fn persistence(ctx: &ServerRuntimeContext) -> Result<Arc<McpPersistence>> {
    runtime_extensions(ctx)?
        .get::<Arc<McpPersistence>>()
        .cloned()
        .ok_or_else(|| Error::Message("MCP persistence is not registered".to_string()))
}

pub(crate) fn management(ctx: &ServerRuntimeContext) -> Result<McpManagementRuntime> {
    runtime_extensions(ctx)?
        .get::<McpManagementRuntime>()
        .cloned()
        .ok_or_else(|| Error::Message("MCP management is not registered".to_string()))
}

struct ServerMcpHost {
    db: DatabaseConnection,
    registry: ModuleRegistry,
    policy_cache: ModuleEffectivePolicyCache,
    permission_owners: Arc<BTreeMap<String, String>>,
}

#[async_trait]
impl McpHostServices for ServerMcpHost {
    fn request_permissions(&self, tenant_id: Uuid, actor_id: Uuid) -> Option<Vec<Permission>> {
        super::rbac_request_scope::permissions_for(&tenant_id, &actor_id)
    }

    async fn delegated_principal(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> OwnerResult<McpDelegatedPrincipal> {
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Failed to load the delegated MCP user");
                McpPersistenceError::InternalServerError
            })?;
        let Some(user) = user else {
            return Ok(McpDelegatedPrincipal::Missing);
        };
        if !user.is_active() {
            return Ok(McpDelegatedPrincipal::Inactive);
        }
        RbacService::get_user_permissions_authoritative(&self.db, &tenant_id, &user_id)
            .await
            .map(McpDelegatedPrincipal::Active)
            .map_err(|error| {
                tracing::error!(%error, "Failed to resolve delegated MCP user permissions");
                McpPersistenceError::InternalServerError
            })
    }

    async fn active_permissions(
        &self,
        tenant_id: Uuid,
        permissions: Vec<Permission>,
    ) -> OwnerResult<Vec<Permission>> {
        let policy = resolve_permission_availability(
            &self.db,
            &self.registry,
            &self.policy_cache,
            &self.permission_owners,
            tenant_id,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, %tenant_id, "MCP module permission availability failed");
            McpPersistenceError::InternalServerError
        })?;
        Ok(policy.active_permissions(&permissions))
    }

    async fn prepare_scaffold_workspace(
        &self,
        requested_root: &str,
    ) -> OwnerResult<Box<dyn McpScaffoldPublication>> {
        let root = authorize_mcp_scaffold_workspace(requested_root)
            .await
            .map_err(workspace_error)?;
        let authority = crate::host_authority::current_host_authority()
            .ok_or(McpPersistenceError::InternalServerError)?;
        let process_guard = super::mcp_scaffold_workspace::lock_process_scaffold_workspace().await;
        let lock = lock_mcp_scaffold_workspace(&root)
            .await
            .map_err(workspace_error)?;
        Ok(Box::new(WorkspacePublication {
            root,
            actor_id: authority.actor_id(),
            lock,
            _process_guard: process_guard,
        }))
    }
}

struct WorkspacePublication {
    root: String,
    actor_id: Uuid,
    lock: McpScaffoldWorkspaceLock,
    _process_guard: tokio::sync::MutexGuard<'static, ()>,
}

#[async_trait]
impl McpScaffoldPublication for WorkspacePublication {
    fn root(&self) -> &str {
        &self.root
    }

    fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    async fn validate_target(&self, draft: &StagedModuleScaffold) -> OwnerResult<()> {
        let file_paths = draft
            .preview
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        validate_mcp_scaffold_target_path(
            &self.root,
            &draft.preview.crate_name,
            &draft.preview.crate_path,
            &file_paths,
        )
        .await
        .map_err(workspace_error)
    }

    fn publish(
        self: Box<Self>,
        draft: &StagedModuleScaffold,
    ) -> std::result::Result<ApplyModuleScaffoldResponse, AlloyScaffoldError> {
        let Self {
            root,
            lock,
            _process_guard,
            ..
        } = *self;
        apply_mcp_scaffold_atomically(draft, &root, lock)
    }
}

fn workspace_error(error: Error) -> McpPersistenceError {
    match error {
        Error::BadRequest(message) => McpPersistenceError::BadRequest(message),
        Error::Forbidden(message) => McpPersistenceError::Forbidden(message),
        Error::Message(message) => McpPersistenceError::Message(message),
        error => {
            tracing::error!(%error, "MCP workspace operation failed");
            McpPersistenceError::InternalServerError
        }
    }
}

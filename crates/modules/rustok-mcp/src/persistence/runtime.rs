use async_trait::async_trait;
use chrono::Utc;
use rustok_api::Permission;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
    sea_query::Expr,
};
use uuid::Uuid;

use super::authority::Authority;
use super::models::{mcp_clients, mcp_policies, mcp_tokens};
use super::service::identity_for_client;
use super::service::{
    ApplyMcpScaffoldDraftInput, McpManagementService, RecordMcpAuditEventInput,
    StageMcpScaffoldDraftInput, hash_token,
};
use super::{McpPersistence, McpPersistenceError as Error, Result};
use crate::McpManagementAuthorityError;
use crate::{
    ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse, McpAccessContext, McpAccessPolicy,
    McpAccessResolver, McpActorType, McpAuditSink, McpIdentity, McpRemoteBootstrapResponse,
    McpRuntimeBinding, McpRuntimeError, McpRuntimeResult, McpScaffoldDraftRuntimeContext,
    McpScaffoldDraftStore, McpSessionContext, McpToolCallAuditEvent, McpToolCallOutcome,
    ReviewModuleScaffoldRequest, ReviewModuleScaffoldResponse, ScaffoldModuleRequest,
    StageModuleScaffoldResponse, TOOL_MCP_WHOAMI, scaffold_source_digest,
};

impl McpPersistence {
    pub async fn bootstrap_remote_session(
        &self,
        session_context: McpSessionContext,
    ) -> Result<McpRemoteBootstrapResponse> {
        if session_context.transport.trim().is_empty() || session_context.transport == "stdio" {
            return Err(Error::BadRequest(
                "Remote MCP bootstrap requires a non-stdio transport".to_string(),
            ));
        }

        let plaintext_token = session_context
            .plaintext_token
            .as_deref()
            .ok_or_else(|| Error::Unauthorized("MCP bearer token is required".into()))?;
        let binding = self.resolve_binding_for_token(plaintext_token).await?;

        McpManagementService::record_audit_event(
            &self.db,
            RecordMcpAuditEventInput {
                tenant_id: parse_uuid(binding.tenant_id.as_deref()).ok_or_else(|| {
                    Error::BadRequest("MCP runtime binding did not include tenant_id".to_string())
                })?,
                client_id: parse_uuid(binding.client_id.as_deref()),
                token_id: parse_uuid(binding.token_id.as_deref()),
                actor_id: binding
                    .access_context
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_id.clone()),
                actor_type: binding
                    .access_context
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_type.as_str().to_string()),
                action: "remote_session_bootstrapped".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason: None,
                correlation_id: session_context.correlation_id.clone(),
                metadata: serde_json::json!({
                    "transport": session_context.transport,
                    "session_metadata": session_context.metadata,
                }),
                created_by: actor_user_id_from_access_context(&binding.access_context),
            },
        )
        .await?;

        Ok(McpRemoteBootstrapResponse {
            transport: session_context.transport,
            correlation_id: session_context.correlation_id,
            tenant_id: binding.tenant_id,
            client_id: binding.client_id,
            token_id: binding.token_id,
            access_context: binding.access_context,
        })
    }

    pub async fn resolve_binding_for_token(
        &self,
        plaintext_token: &str,
    ) -> Result<McpRuntimeBinding> {
        let token = mcp_tokens::Entity::find_active_by_hash(&self.db, &hash_token(plaintext_token))
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| Error::Unauthorized("Invalid or expired MCP token".into()))?;

        let client = mcp_clients::Entity::find_by_id(token.client_id)
            .filter(mcp_clients::Column::TenantId.eq(token.tenant_id))
            .one(&self.db)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| Error::Unauthorized("Invalid or expired MCP token".into()))?;

        if !client.is_active() {
            return Err(Error::Unauthorized("Invalid or expired MCP token".into()));
        }

        let policy =
            mcp_policies::Entity::find_by_client_scoped(&self.db, client.tenant_id, client.id)
                .await
                .map_err(map_db_err)?;
        Authority::validate_runtime_policy(
            self.host.as_ref(),
            client.tenant_id,
            &client,
            policy.as_ref(),
        )
        .await
        .map_err(|error| map_runtime_policy_error(&client, error))?;

        touch_last_used(&self.db, &client, &token).await?;

        let access_context = access_context_for_client(&client, policy.as_ref())?;
        let access_context = self
            .filter_active_module_permissions(client.tenant_id, access_context)
            .await?;

        Ok(McpRuntimeBinding {
            access_context,
            tenant_id: Some(client.tenant_id.to_string()),
            client_id: Some(client.id.to_string()),
            token_id: Some(token.id.to_string()),
        })
    }

    async fn filter_active_module_permissions(
        &self,
        tenant_id: Uuid,
        mut access_context: McpAccessContext,
    ) -> Result<McpAccessContext> {
        let permissions = parse_granted_permissions(&access_context.granted_permissions);
        let mut active = self
            .host
            .active_permissions(tenant_id, permissions)
            .await
            .map_err(|_error| {
                tracing::error!(
                    tenant_id = %tenant_id,
                    error_code = "mcp_module_permission_availability_failed",
                    "MCP module permission availability failed"
                );
                Error::InternalServerError
            })?;
        active.sort_by_cached_key(ToString::to_string);
        active.dedup();
        access_context.granted_permissions = active
            .into_iter()
            .map(|permission| permission.to_string())
            .collect();
        Ok(access_context)
    }
}

#[async_trait]
impl McpAccessResolver for McpPersistence {
    async fn resolve_runtime_binding(
        &self,
        session: &McpSessionContext,
    ) -> McpRuntimeResult<McpRuntimeBinding> {
        let plaintext_token = session.plaintext_token.as_deref().ok_or_else(|| {
            McpRuntimeError::Message("MCP session plaintext token is required".to_string())
        })?;

        self.resolve_binding_for_token(plaintext_token)
            .await
            .map_err(|error| McpRuntimeError::Message(error.to_string()))
    }
}

#[async_trait]
impl McpAuditSink for McpPersistence {
    async fn record_tool_call(&self, event: McpToolCallAuditEvent) -> McpRuntimeResult<()> {
        let Some(tenant_id) = parse_uuid(event.tenant_id.as_deref()) else {
            tracing::warn!(
                tool_name_len = event.tool_name.len(),
                "Skipping MCP tool audit persistence because tenant_id is missing"
            );
            return Ok(());
        };

        McpManagementService::record_audit_event(
            &self.db,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: parse_uuid(event.client_id.as_deref()),
                token_id: parse_uuid(event.token_id.as_deref()),
                actor_id: event
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_id.clone()),
                actor_type: event
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_type.as_str().to_string()),
                action: "tool_call".to_string(),
                outcome: match event.outcome {
                    McpToolCallOutcome::Allowed => "allowed".to_string(),
                    McpToolCallOutcome::Denied => "denied".to_string(),
                },
                tool_name: Some(event.tool_name),
                reason: event.reason,
                correlation_id: event.correlation_id,
                metadata: serde_json::json!({
                    "transport": event.transport,
                    "session_metadata": event.metadata,
                }),
                created_by: None,
            },
        )
        .await
        .map_err(|error| McpRuntimeError::Message(error.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl McpScaffoldDraftStore for McpPersistence {
    async fn stage_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ScaffoldModuleRequest,
    ) -> McpRuntimeResult<StageModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let client_id = client_id_from_runtime_context(context)?;
        let created_by = actor_user_id_from_runtime_context(context);
        let (actor_id, actor_type) = actor_identity_from_runtime_context(context);

        let draft = McpManagementService::stage_scaffold_draft(
            &self.db,
            tenant_id,
            StageMcpScaffoldDraftInput {
                client_id: Some(client_id),
                request,
                created_by,
                actor_id,
                actor_type,
            },
        )
        .await
        .map_err(|error| McpRuntimeError::Message(error.to_string()))?;

        let preview = draft
            .preview()
            .map_err(|error| McpRuntimeError::Message(error.to_string()))?;
        let source_digest = scaffold_source_digest(&preview)
            .map_err(|error| McpRuntimeError::Message(error.to_string()))?;

        Ok(StageModuleScaffoldResponse {
            draft_id: draft.id.to_string(),
            preview,
            source_digest,
            status: draft.status_value().map_err(McpRuntimeError::Message)?,
            review_required: true,
            apply_tool: crate::TOOL_ALLOY_APPLY_MODULE_SCAFFOLD.to_string(),
            next_steps: vec![
                format!(
                    "Review the persisted draft with {} before applying it.",
                    crate::TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD
                ),
                format!(
                    "Apply the persisted draft with {} and confirm=true.",
                    crate::TOOL_ALLOY_APPLY_MODULE_SCAFFOLD
                ),
            ],
        })
    }

    async fn review_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ReviewModuleScaffoldRequest,
    ) -> McpRuntimeResult<ReviewModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let draft_id = Uuid::parse_str(&request.draft_id)
            .map_err(|error| McpRuntimeError::Message(format!("Invalid draft id: {error}")))?;
        let client_id = client_id_from_runtime_context(context)
            .map_err(|error| McpRuntimeError::Message(error.to_string()))?;
        let draft = McpManagementService::get_scaffold_draft_for_client(
            &self.db, tenant_id, client_id, draft_id,
        )
        .await
        .map_err(|error| McpRuntimeError::Message(error.to_string()))?
        .ok_or_else(|| {
            McpRuntimeError::Message(format!("Unknown scaffold draft: {}", request.draft_id))
        })?;

        ensure_scaffold_draft_client_binding(draft.client_id, client_id)?;
        let draft = draft
            .to_reviewable_draft()
            .map_err(McpRuntimeError::Message)?;
        let source_digest = scaffold_source_digest(&draft.preview)
            .map_err(|error| McpRuntimeError::Message(error.to_string()))?;
        Ok(ReviewModuleScaffoldResponse {
            draft,
            source_digest,
        })
    }

    async fn apply_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ApplyModuleScaffoldRequest,
    ) -> McpRuntimeResult<ApplyModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let applied_by = actor_user_id_from_runtime_context(context);
        let (actor_id, actor_type) = actor_identity_from_runtime_context(context);
        let draft_id = Uuid::parse_str(&request.draft_id)
            .map_err(|error| McpRuntimeError::Message(format!("Invalid draft id: {error}")))?;

        let client_id = client_id_from_runtime_context(context)
            .map_err(|error| McpRuntimeError::Message(error.to_string()))?;
        let (_, response) = McpManagementService::apply_scaffold_draft(
            &self.db,
            self.host.as_ref(),
            tenant_id,
            draft_id,
            ApplyMcpScaffoldDraftInput {
                workspace_root: request.workspace_root,
                confirm: request.confirm,
                expected_client_id: Some(client_id),
                applied_by,
                actor_id,
                actor_type,
            },
        )
        .await
        .map_err(|error| McpRuntimeError::Message(error.to_string()))?;

        Ok(response)
    }
}

fn parse_granted_permissions(granted_permissions: &[String]) -> Vec<Permission> {
    // Canonicalize persisted grants even when the host runs without a module registry.
    let mut permissions = Vec::with_capacity(granted_permissions.len());
    for value in granted_permissions {
        match value.parse::<Permission>() {
            Ok(permission) => permissions.push(permission),
            Err(_error) => {
                tracing::warn!(
                    error_code = "mcp_granted_permission_invalid",
                    "Ignoring unparseable granted permission"
                );
            }
        }
    }
    permissions
}

fn access_context_for_client(
    client: &mcp_clients::Model,
    policy: Option<&mcp_policies::Model>,
) -> Result<McpAccessContext> {
    match policy {
        Some(policy) => {
            let identity = identity_for_client(client, policy).map_err(|_error| {
                tracing::error!(
                    client_id = %client.id,
                    error_code = "mcp_persisted_policy_invalid",
                    "Persisted MCP policy is invalid"
                );
                Error::InternalServerError
            })?;
            policy
                .to_access_context(client, Some(identity))
                .map_err(|_error| {
                    tracing::error!(
                        client_id = %client.id,
                        error_code = "mcp_persisted_policy_invalid",
                        "Persisted MCP policy is invalid"
                    );
                    Error::InternalServerError
                })
        }
        None => Ok(McpAccessContext {
            identity: Some(McpIdentity {
                actor_id: client.id.to_string(),
                actor_type: client.actor_type(),
                tenant_id: Some(client.tenant_id.to_string()),
                delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
                display_name: Some(client.display_name.clone()),
                scopes: Vec::new(),
            }),
            granted_permissions: Vec::new(),
            policy: McpAccessPolicy {
                allowed_tools: Some(vec![TOOL_MCP_WHOAMI.to_string()]),
                denied_tools: Vec::new(),
            },
        }),
    }
}

fn tenant_id_from_runtime_context(
    context: &McpScaffoldDraftRuntimeContext,
) -> McpRuntimeResult<Uuid> {
    context
        .runtime_binding
        .as_ref()
        .and_then(|binding| binding.tenant_id.as_deref())
        .and_then(|value| parse_uuid(Some(value)))
        .ok_or_else(|| {
            McpRuntimeError::from("Persisted scaffold draft flow requires tenant-bound MCP runtime")
        })
}

fn client_id_from_runtime_context(
    context: &McpScaffoldDraftRuntimeContext,
) -> McpRuntimeResult<Uuid> {
    let value = context
        .runtime_binding
        .as_ref()
        .and_then(|binding| binding.client_id.as_deref())
        .ok_or_else(|| {
            McpRuntimeError::from(
                "Persisted scaffold draft flow requires a client-bound MCP runtime",
            )
        })?;
    Uuid::parse_str(value).map_err(|error| {
        McpRuntimeError::from(format!("Invalid MCP client id in runtime binding: {error}"))
    })
}

fn ensure_scaffold_draft_client_binding(
    draft_client_id: Option<Uuid>,
    expected_client_id: Uuid,
) -> McpRuntimeResult<()> {
    if draft_client_id == Some(expected_client_id) {
        Ok(())
    } else {
        Err(McpRuntimeError::Message(
            "Unknown scaffold draft".to_string(),
        ))
    }
}

fn actor_identity_from_runtime_context(
    context: &McpScaffoldDraftRuntimeContext,
) -> (Option<String>, Option<String>) {
    context
        .access_context
        .as_ref()
        .and_then(|access| access.identity.as_ref())
        .map(|identity| {
            (
                Some(identity.actor_id.clone()),
                Some(identity.actor_type.as_str().to_string()),
            )
        })
        .unwrap_or_default()
}

fn actor_user_id_from_runtime_context(context: &McpScaffoldDraftRuntimeContext) -> Option<Uuid> {
    context
        .access_context
        .as_ref()
        .and_then(actor_user_id_from_access_context)
}

fn actor_user_id_from_access_context(access: &McpAccessContext) -> Option<Uuid> {
    access.identity.as_ref().and_then(|identity| {
        identity
            .delegated_user_id
            .as_deref()
            .and_then(|value| parse_uuid(Some(value)))
            .or_else(|| match identity.actor_type {
                McpActorType::HumanUser => parse_uuid(Some(identity.actor_id.as_str())),
                _ => None,
            })
    })
}

async fn touch_last_used(
    db: &DatabaseConnection,
    client: &mcp_clients::Model,
    token: &mcp_tokens::Model,
) -> Result<()> {
    if token.tenant_id != client.tenant_id || token.client_id != client.id {
        return Err(Error::Unauthorized(
            "MCP token binding is invalid".to_string(),
        ));
    }

    let now = Utc::now();
    let tx = db.begin().await.map_err(map_db_err)?;

    // These are intentionally column-scoped conditional updates. Writing a
    // stale ActiveModel here could restore `is_active`/`revoked_at` values if
    // an administrator revoked the client or token after it was loaded.
    let client_touch = mcp_clients::Entity::update_many()
        .col_expr(mcp_clients::Column::LastUsedAt, Expr::value(Some(now)))
        .filter(mcp_clients::Column::Id.eq(client.id))
        .filter(mcp_clients::Column::TenantId.eq(client.tenant_id))
        .filter(mcp_clients::Column::IsActive.eq(true))
        .filter(mcp_clients::Column::RevokedAt.is_null())
        .exec(&tx)
        .await
        .map_err(map_db_err)?;
    if client_touch.rows_affected != 1 {
        return Err(Error::Unauthorized("MCP client is inactive".to_string()));
    }

    let token_touch = mcp_tokens::Entity::update_many()
        .col_expr(mcp_tokens::Column::LastUsedAt, Expr::value(Some(now)))
        .filter(mcp_tokens::Column::Id.eq(token.id))
        .filter(mcp_tokens::Column::TenantId.eq(client.tenant_id))
        .filter(mcp_tokens::Column::ClientId.eq(client.id))
        .filter(mcp_tokens::Column::TokenHash.eq(&token.token_hash))
        .filter(mcp_tokens::Column::RevokedAt.is_null())
        .filter(
            Condition::any()
                .add(mcp_tokens::Column::ExpiresAt.is_null())
                .add(mcp_tokens::Column::ExpiresAt.gt(now)),
        )
        .exec(&tx)
        .await
        .map_err(map_db_err)?;
    if token_touch.rows_affected != 1 {
        return Err(Error::Unauthorized(
            "MCP token is invalid or expired".to_string(),
        ));
    }

    tx.commit().await.map_err(map_db_err)?;
    Ok(())
}

fn parse_uuid(value: Option<&str>) -> Option<Uuid> {
    value
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| !value.is_nil())
}

fn map_db_err(_err: sea_orm::DbErr) -> Error {
    tracing::error!(
        error_code = "mcp_runtime_database_operation_failed",
        "MCP runtime database operation failed"
    );
    Error::InternalServerError
}

fn map_runtime_policy_error(
    client: &mcp_clients::Model,
    error: McpManagementAuthorityError,
) -> Error {
    let error_code = match &error {
        McpManagementAuthorityError::Internal(_) => "mcp_policy_internal_error",
        McpManagementAuthorityError::Invalid(_) => "mcp_policy_invalid",
        McpManagementAuthorityError::Forbidden(_) => "mcp_policy_forbidden",
        McpManagementAuthorityError::NotFound(_) => "mcp_policy_not_found",
    };
    tracing::warn!(
        tenant_id = %client.tenant_id,
        client_id = %client.id,
        error_code,
        "MCP runtime policy is no longer authorized"
    );
    match error {
        McpManagementAuthorityError::Internal(_) => Error::InternalServerError,
        McpManagementAuthorityError::Invalid(_)
        | McpManagementAuthorityError::Forbidden(_)
        | McpManagementAuthorityError::NotFound(_) => {
            Error::Unauthorized("Invalid or expired MCP token".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
        EntityTrait, Set, Statement,
        sea_query::{Alias, ColumnDef, Table},
    };
    use sea_orm_migration::SchemaManager;

    use super::super::{
        McpDelegatedPrincipal, McpHostServices, McpPersistenceError, McpScaffoldPublication,
        Result,
        models::{mcp_clients, mcp_policies, mcp_tokens},
    };
    use super::{
        McpPersistence, access_context_for_client, actor_user_id_from_runtime_context,
        ensure_scaffold_draft_client_binding, map_db_err, parse_uuid, touch_last_used,
    };
    use crate::{
        McpAccessContext, McpActorType, McpIdentity, McpScaffoldDraftRuntimeContext,
        McpSessionContext, TOOL_MCP_WHOAMI,
    };
    use rustok_api::Permission;
    use uuid::Uuid;

    struct TestHost {
        disabled_permission: Option<Permission>,
        availability_failed: bool,
    }

    #[async_trait::async_trait]
    impl McpHostServices for TestHost {
        fn request_permissions(&self, _: Uuid, _: Uuid) -> Option<Vec<Permission>> {
            None
        }

        async fn delegated_principal(
            &self,
            _tenant_id: Uuid,
            _user_id: Uuid,
        ) -> Result<McpDelegatedPrincipal> {
            Err(McpPersistenceError::InternalServerError)
        }

        async fn active_permissions(
            &self,
            _tenant_id: Uuid,
            mut permissions: Vec<Permission>,
        ) -> Result<Vec<Permission>> {
            if self.availability_failed {
                return Err(McpPersistenceError::InternalServerError);
            }
            if let Some(disabled_permission) = &self.disabled_permission {
                permissions.retain(|permission| permission != disabled_permission);
            }
            Ok(permissions)
        }

        async fn prepare_scaffold_workspace(
            &self,
            _requested_root: &str,
        ) -> Result<Box<dyn McpScaffoldPublication>> {
            Err(McpPersistenceError::InternalServerError)
        }
    }

    async fn permission_test_runtime(host: TestHost) -> McpPersistence {
        McpPersistence {
            db: Database::connect("sqlite::memory:")
                .await
                .expect("open runtime test database"),
            host: Arc::new(host),
        }
    }

    #[tokio::test]
    async fn granted_permissions_are_canonicalized_without_a_module_registry() {
        let runtime = permission_test_runtime(TestHost {
            disabled_permission: None,
            availability_failed: false,
        })
        .await;
        let access = McpAccessContext {
            granted_permissions: vec![
                format!(" {} ", Permission::MODULES_READ),
                Permission::MODULES_READ.to_string(),
                "not:a:permission".to_string(),
            ],
            ..Default::default()
        };
        let filtered = runtime
            .filter_active_module_permissions(Uuid::new_v4(), access)
            .await
            .expect("canonicalize permissions without a module registry");

        assert_eq!(
            filtered.granted_permissions,
            vec![Permission::MODULES_READ.to_string()]
        );
    }

    #[tokio::test]
    async fn permission_availability_failure_rejects_the_runtime_binding() {
        let runtime = permission_test_runtime(TestHost {
            disabled_permission: None,
            availability_failed: true,
        })
        .await;
        let denied = runtime
            .filter_active_module_permissions(Uuid::new_v4(), McpAccessContext::default())
            .await;
        assert!(matches!(
            denied,
            Err(McpPersistenceError::InternalServerError)
        ));
    }

    #[test]
    fn scaffold_draft_access_is_bound_to_the_current_mcp_client() {
        let client_id = Uuid::new_v4();
        assert!(ensure_scaffold_draft_client_binding(Some(client_id), client_id).is_ok());
        assert!(ensure_scaffold_draft_client_binding(None, client_id).is_err());
        assert!(ensure_scaffold_draft_client_binding(Some(Uuid::new_v4()), client_id).is_err());
    }

    #[test]
    fn mcp_runtime_uuid_parser_rejects_nil_identifiers() {
        let nil = Uuid::nil().to_string();
        assert_eq!(parse_uuid(Some(&nil)), None);
        assert_eq!(parse_uuid(Some("not-a-uuid")), None);

        let valid = Uuid::new_v4();
        let valid_text = valid.to_string();
        assert_eq!(parse_uuid(Some(&valid_text)), Some(valid));
        assert_eq!(parse_uuid(None), None);
    }

    #[tokio::test]
    async fn disabled_module_grants_are_removed_from_mcp_runtime_context() {
        let runtime = permission_test_runtime(TestHost {
            disabled_permission: Some(Permission::FORUM_TOPICS_READ),
            availability_failed: false,
        })
        .await;
        let access = McpAccessContext {
            granted_permissions: vec![
                Permission::FORUM_TOPICS_READ.to_string(),
                Permission::MODULES_LIST.to_string(),
            ],
            ..Default::default()
        };
        let access = runtime
            .filter_active_module_permissions(Uuid::new_v4(), access)
            .await
            .expect("filter disabled module permissions");

        assert_eq!(
            access.granted_permissions,
            vec![Permission::MODULES_LIST.to_string()]
        );
        let decision = access.authorize_tool(&crate::McpToolRequirement {
            tool_name: "forum.read_topic".to_string(),
            required_permissions: vec![Permission::FORUM_TOPICS_READ.to_string()],
            required_scopes: Vec::new(),
        });
        assert!(!decision.allowed);
        assert_eq!(decision.code.as_deref(), Some("missing_permissions"));
    }

    #[test]
    fn database_errors_map_to_internal_server_error() {
        assert!(matches!(
            map_db_err(sea_orm::DbErr::Custom("database failure".to_string())),
            McpPersistenceError::InternalServerError
        ));
    }

    async fn open_persistence_fixture() -> (DatabaseConnection, Uuid) {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("open MCP test database");
        let schema = SchemaManager::new(&db);
        // These tables stub only the external keys referenced by MCP migrations.
        for table in ["tenants", "users"] {
            schema
                .create_table(
                    Table::create()
                        .table(Alias::new(table))
                        .col(
                            ColumnDef::new(Alias::new("id"))
                                .uuid()
                                .not_null()
                                .primary_key(),
                        )
                        .to_owned(),
                )
                .await
                .expect("create referenced key table");
        }
        for migration in crate::migrations::migrations() {
            migration.up(&schema).await.expect("apply MCP migration");
        }

        let tenant_id = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .expect("insert tenant");
        (db, tenant_id)
    }

    #[tokio::test]
    async fn rotation_checks_current_policy_before_revoking_the_existing_token() {
        use crate::{
            CreateMcpClientCommand, McpManagementContext, McpManagementMutationError,
            McpManagementPort, RotateMcpTokenCommand, UpdateMcpPolicyCommand,
        };

        let (db, tenant_id) = open_persistence_fixture().await;
        let runtime = McpPersistence::new(
            db,
            Arc::new(TestHost {
                disabled_permission: None,
                availability_failed: false,
            }),
        );
        let operator_id = Uuid::new_v4();
        let operator = McpManagementContext::new(
            tenant_id,
            operator_id,
            McpActorType::ServiceClient,
            vec![Permission::MCP_MANAGE],
        );
        let administrator = McpManagementContext::new(
            tenant_id,
            Uuid::new_v4(),
            McpActorType::ServiceClient,
            vec![Permission::MCP_MANAGE, Permission::PAGES_MANAGE],
        );
        let issued = runtime
            .create_client(
                &operator,
                CreateMcpClientCommand {
                    slug: "rotation".to_string(),
                    display_name: "Rotation".to_string(),
                    description: None,
                    actor_type: McpActorType::ServiceClient,
                    delegated_user_id: None,
                    token_name: None,
                    token_expires_at: None,
                    allowed_tools: Vec::new(),
                    denied_tools: Vec::new(),
                    granted_permissions: Vec::new(),
                    granted_scopes: Vec::new(),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("create the MCP client and token");
        let client_id = issued.client.id;
        runtime
            .update_policy(
                &administrator,
                UpdateMcpPolicyCommand {
                    client_id,
                    allowed_tools: Vec::new(),
                    denied_tools: Vec::new(),
                    granted_permissions: vec![Permission::PAGES_READ.to_string()],
                    granted_scopes: Vec::new(),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("grant a permission only the administrator can delegate");

        let denied = runtime
            .rotate_token(
                &operator,
                RotateMcpTokenCommand {
                    client_id,
                    token_name: None,
                    expires_at: None,
                    revoke_existing_tokens: true,
                    metadata: serde_json::json!({}),
                },
            )
            .await;

        assert!(matches!(
            denied,
            Err(McpManagementMutationError::Authority(
                crate::McpManagementAuthorityError::Forbidden(_)
            ))
        ));
        let details = runtime
            .get_client(&operator, client_id)
            .await
            .expect("read current tokens")
            .expect("client persists");
        assert_eq!(details.tokens.len(), 1);
        assert_eq!(details.tokens[0].id, issued.token.id);
        assert!(details.tokens[0].is_active);
        let audit = runtime
            .list_audit_events(&operator, Some(client_id), None, None)
            .await
            .expect("read the audit log");
        assert!(audit.iter().all(|event| event.action != "token_rotated"));
        assert!(audit.iter().any(|event| {
            event.actor_id.as_deref() == Some(operator_id.to_string().as_str())
                && event.actor_type.as_deref() == Some("service_client")
                && event.created_by.is_none()
        }));
    }

    #[tokio::test]
    async fn last_used_touch_cannot_resurrect_revoked_mcp_credentials() {
        let (db, tenant_id) = open_persistence_fixture().await;
        let client = mcp_clients::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            client_key: Set(Uuid::new_v4()),
            slug: Set("touch-client".to_string()),
            display_name: Set("Touch client".to_string()),
            description: Set(None),
            actor_type: Set("service_client".to_string()),
            delegated_user_id: Set(None),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_by: Set(None),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("insert MCP client");
        let token = mcp_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            client_id: Set(client.id),
            token_name: Set("primary".to_string()),
            token_preview: Set("mcp_…".to_string()),
            token_hash: Set(format!("hash-{}", Uuid::new_v4())),
            created_by: Set(None),
            last_used_at: Set(None),
            expires_at: Set(None),
            revoked_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("insert MCP token");

        let stale_client = mcp_clients::Entity::find_by_id(client.id)
            .one(&db)
            .await
            .expect("load client snapshot")
            .expect("client exists");
        let stale_token = mcp_tokens::Entity::find_by_id(token.id)
            .one(&db)
            .await
            .expect("load token snapshot")
            .expect("token exists");
        let mut revoked: mcp_tokens::ActiveModel = token.into();
        revoked.revoked_at = Set(Some(chrono::Utc::now().into()));
        revoked.update(&db).await.expect("revoke token");

        let error = touch_last_used(&db, &stale_client, &stale_token)
            .await
            .expect_err("stale token snapshot must not touch or restore a revoked token");
        assert!(matches!(error, McpPersistenceError::Unauthorized(_)));
        let persisted = mcp_tokens::Entity::find_by_id(stale_token.id)
            .one(&db)
            .await
            .expect("reload token")
            .expect("token remains present");
        assert!(persisted.revoked_at.is_some());

        let mut reactivated_token: mcp_tokens::ActiveModel = persisted.into();
        reactivated_token.revoked_at = Set(None);
        let active_token = reactivated_token
            .update(&db)
            .await
            .expect("prepare independent client-revocation case");
        let stale_active_token = mcp_tokens::Entity::find_by_id(active_token.id)
            .one(&db)
            .await
            .expect("load active token snapshot")
            .expect("active token exists");
        let mut deactivated_client: mcp_clients::ActiveModel = client.into();
        deactivated_client.is_active = Set(false);
        deactivated_client.revoked_at = Set(Some(chrono::Utc::now().into()));
        deactivated_client
            .update(&db)
            .await
            .expect("deactivate client");

        let error = touch_last_used(&db, &stale_client, &stale_active_token)
            .await
            .expect_err("stale client snapshot must not restore a revoked client");
        assert!(matches!(error, McpPersistenceError::Unauthorized(_)));
        let persisted_client = mcp_clients::Entity::find_by_id(stale_client.id)
            .one(&db)
            .await
            .expect("reload client")
            .expect("client remains present");
        assert!(!persisted_client.is_active());
    }

    #[test]
    fn missing_policy_falls_back_to_whoami_only_access() {
        let client = mcp_clients::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            client_key: Uuid::new_v4(),
            slug: "writer-agent".to_string(),
            display_name: "Writer".to_string(),
            description: None,
            actor_type: "model_agent".to_string(),
            delegated_user_id: None,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };

        let access_context =
            access_context_for_client(&client, None).expect("default policy context");

        assert_eq!(
            access_context.policy.allowed_tools,
            Some(vec![TOOL_MCP_WHOAMI.to_string()])
        );
        assert!(access_context.granted_permissions.is_empty());
    }

    #[test]
    fn policy_driven_access_preserves_granted_scopes() {
        let client = mcp_clients::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            client_key: Uuid::new_v4(),
            slug: "svc-client".to_string(),
            display_name: "Service".to_string(),
            description: None,
            actor_type: "service_client".to_string(),
            delegated_user_id: None,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let policy = mcp_policies::Model {
            id: Uuid::new_v4(),
            tenant_id: client.tenant_id,
            client_id: client.id,
            allowed_tools: serde_json::json!(["list_modules"]),
            denied_tools: serde_json::json!([]),
            granted_permissions: serde_json::json!(["modules:list"]),
            granted_scopes: serde_json::json!(["modules.read"]),
            metadata: serde_json::json!({}),
            updated_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };

        let access_context =
            access_context_for_client(&client, Some(&policy)).expect("valid persisted policy");

        assert_eq!(
            access_context
                .identity
                .and_then(|identity| identity.scopes.into_iter().next()),
            Some("modules.read".to_string())
        );

        let mut corrupted_policy = policy;
        corrupted_policy.denied_tools = serde_json::json!({ "not": "a string list" });
        assert!(
            access_context_for_client(&client, Some(&corrupted_policy)).is_err(),
            "a corrupted deny-list must fail closed instead of becoming empty"
        );
    }

    #[test]
    fn draft_actor_user_prefers_delegated_user_id() {
        let delegated_user_id = Uuid::new_v4();
        let runtime_context = McpScaffoldDraftRuntimeContext {
            session: McpSessionContext::stdio(),
            runtime_binding: None,
            access_context: Some(McpAccessContext {
                identity: Some(McpIdentity {
                    actor_id: Uuid::new_v4().to_string(),
                    actor_type: McpActorType::ModelAgent,
                    tenant_id: None,
                    delegated_user_id: Some(delegated_user_id.to_string()),
                    display_name: None,
                    scopes: Vec::new(),
                }),
                granted_permissions: Vec::new(),
                policy: Default::default(),
            }),
        };

        assert_eq!(
            actor_user_id_from_runtime_context(&runtime_context),
            Some(delegated_user_id)
        );
    }

    #[test]
    fn draft_actor_user_uses_human_actor_id_when_it_is_uuid() {
        let actor_id = Uuid::new_v4();
        let runtime_context = McpScaffoldDraftRuntimeContext {
            session: McpSessionContext::stdio(),
            runtime_binding: None,
            access_context: Some(McpAccessContext {
                identity: Some(McpIdentity {
                    actor_id: actor_id.to_string(),
                    actor_type: McpActorType::HumanUser,
                    tenant_id: None,
                    delegated_user_id: None,
                    display_name: None,
                    scopes: Vec::new(),
                }),
                granted_permissions: Vec::new(),
                policy: Default::default(),
            }),
        };

        assert_eq!(
            actor_user_id_from_runtime_context(&runtime_context),
            Some(actor_id)
        );
    }
}

use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use super::authority::Authority;
use super::models::{mcp_audit_logs, mcp_clients, mcp_policies, mcp_scaffold_drafts, mcp_tokens};
use super::{McpHostServices, McpPersistenceError as Error, Result};
use crate::{
    ApplyModuleScaffoldResponse, McpAccessContext, McpActorType, McpIdentity,
    ScaffoldModuleRequest, generate_module_scaffold,
};
use rustok_api::Permission;

#[derive(Debug, Clone)]
pub struct CreateMcpClientInput {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorType,
    pub delegated_user_id: Option<Uuid>,
    pub token_name: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub audit_actor_id: Option<String>,
    pub audit_actor_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RotateMcpTokenInput {
    pub token_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub audit_actor_id: Option<String>,
    pub audit_actor_type: Option<String>,
    pub revoke_existing_tokens: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpPolicyInput {
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
    pub updated_by: Option<Uuid>,
    pub audit_actor_id: Option<String>,
    pub audit_actor_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StageMcpScaffoldDraftInput {
    pub client_id: Option<Uuid>,
    pub request: ScaffoldModuleRequest,
    pub created_by: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApplyMcpScaffoldDraftInput {
    pub workspace_root: String,
    pub confirm: bool,
    /// When supplied by remote MCP, the draft must belong to this exact client.
    /// Tenant-level admin management leaves this unset.
    pub expected_client_id: Option<Uuid>,
    pub applied_by: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct McpAuditFilters {
    pub client_id: Option<Uuid>,
    pub outcome: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug)]
pub struct CreateMcpClientResult {
    pub client: mcp_clients::Model,
    pub policy: mcp_policies::Model,
    pub token: mcp_tokens::Model,
    pub plaintext_token: String,
}

#[derive(Debug)]
pub struct RotateMcpTokenResult {
    pub client: mcp_clients::Model,
    pub token: mcp_tokens::Model,
    pub plaintext_token: String,
}

#[derive(Debug)]
pub struct McpClientDetails {
    pub client: mcp_clients::Model,
    pub policy: Option<mcp_policies::Model>,
    pub tokens: Vec<mcp_tokens::Model>,
    pub effective_access_context: Option<McpAccessContext>,
}

#[derive(Debug, Clone)]
pub struct RecordMcpAuditEventInput {
    pub tenant_id: Uuid,
    pub client_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub action: String,
    pub outcome: String,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
}

pub struct McpManagementService;

const MCP_SCAFFOLD_STATUS_STAGED: &str = "staged";
const MCP_SCAFFOLD_STATUS_APPLYING: &str = "applying";
const MCP_SCAFFOLD_STATUS_APPLIED: &str = "applied";
const MCP_SCAFFOLD_STATUS_RECOVERY_REQUIRED: &str = "recovery_required";

impl McpManagementService {
    #[tracing::instrument(
        skip(db, input),
        fields(tenant_id = %tenant_id, slug = %input.slug),
        err
    )]
    pub async fn create_client(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: CreateMcpClientInput,
    ) -> Result<CreateMcpClientResult> {
        validate_slug(&input.slug)?;
        validate_mcp_label(&input.display_name, "display name", 160)?;
        validate_token_expiry(input.token_expires_at.as_ref())?;
        let token_name = input
            .token_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "primary".to_string());
        validate_mcp_label(&token_name, "token name", 128)?;
        let actor_type = input.actor_type.as_str();
        let allowed_tools = dedupe(input.allowed_tools);
        let denied_tools = dedupe(input.denied_tools);
        let granted_permissions = dedupe(input.granted_permissions);
        let granted_scopes = dedupe(input.granted_scopes);
        let metadata = normalize_metadata(input.metadata);

        let txn = db.begin().await.map_err(map_db_err)?;

        let client = mcp_clients::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            client_key: Set(Uuid::new_v4()),
            slug: Set(input.slug.clone()),
            display_name: Set(input.display_name.clone()),
            description: Set(input.description.clone()),
            actor_type: Set(actor_type.to_string()),
            delegated_user_id: Set(input.delegated_user_id),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(metadata.clone()),
            created_by: Set(input.created_by),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(map_db_err)?;

        let policy = mcp_policies::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            client_id: Set(client.id),
            allowed_tools: Set(to_json_array(&allowed_tools)?),
            denied_tools: Set(to_json_array(&denied_tools)?),
            granted_permissions: Set(to_json_array(&granted_permissions)?),
            granted_scopes: Set(to_json_array(&granted_scopes)?),
            metadata: Set(metadata.clone()),
            updated_by: Set(input.created_by),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(map_db_err)?;

        let (token, plaintext_token) = insert_token(
            &txn,
            tenant_id,
            client.id,
            token_name.clone(),
            input.token_expires_at,
            metadata.clone(),
            input.created_by,
        )
        .await?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: Some(client.id),
                token_id: Some(token.id),
                actor_id: input
                    .audit_actor_id
                    .clone()
                    .or_else(|| input.created_by.map(|value| value.to_string())),
                actor_type: input
                    .audit_actor_type
                    .clone()
                    .or_else(|| input.created_by.map(|_| "human_user".to_string())),
                action: "client_created".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason: None,
                correlation_id: None,
                metadata: serde_json::json!({
                    "slug": client.slug,
                    "token_name": token_name,
                }),
                created_by: input.created_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;

        Ok(CreateMcpClientResult {
            client,
            policy,
            token,
            plaintext_token,
        })
    }

    pub async fn list_clients(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<mcp_clients::Model>> {
        let limit = clamp_limit(limit, 50, 100);
        mcp_clients::Entity::find()
            .filter(mcp_clients::Column::TenantId.eq(tenant_id))
            .order_by_desc(mcp_clients::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(map_db_err)
    }

    pub async fn get_client_details(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
    ) -> Result<Option<McpClientDetails>> {
        let client = mcp_clients::Entity::find_by_id(client_id)
            .filter(mcp_clients::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(map_db_err)?;

        let Some(client) = client else {
            return Ok(None);
        };

        let policy = mcp_policies::Entity::find_by_client_scoped(db, tenant_id, client.id)
            .await
            .map_err(map_db_err)?;
        let tokens = mcp_tokens::Entity::find_by_client_scoped(db, tenant_id, client.id)
            .await
            .map_err(map_db_err)?;
        let effective_access_context = policy
            .as_ref()
            .map(|policy| {
                identity_for_client(&client, policy)
                    .and_then(|identity| policy.to_access_context(&client, Some(identity)))
            })
            .transpose()
            .map_err(|_error| {
                tracing::error!(
                    client_id = %client.id,
                    error_code = "mcp_persisted_policy_invalid",
                    "Persisted MCP policy is invalid"
                );
                Error::InternalServerError
            })?;

        Ok(Some(McpClientDetails {
            client,
            policy,
            tokens,
            effective_access_context,
        }))
    }

    pub async fn rotate_token(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
        input: RotateMcpTokenInput,
        manager_permissions: &[Permission],
        delegated_permissions: Option<&[Permission]>,
    ) -> Result<RotateMcpTokenResult> {
        let token_name = input
            .token_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "rotated".to_string());
        validate_mcp_label(&token_name, "token name", 128)?;
        validate_token_expiry(input.expires_at.as_ref())?;
        let txn = db.begin().await.map_err(map_db_err)?;
        let client = lock_client_for_update_txn(&txn, tenant_id, client_id).await?;
        if !client.is_active() {
            return Err(Error::BadRequest(
                "MCP client is inactive and cannot issue a new token".to_string(),
            ));
        }

        Authority::validate_token_rotation(
            &txn,
            &client,
            manager_permissions,
            delegated_permissions,
        )
        .await?;

        if input.revoke_existing_tokens {
            revoke_active_tokens_txn(&txn, tenant_id, client.id).await?;
        }

        let (token, plaintext_token) = insert_token(
            &txn,
            tenant_id,
            client.id,
            token_name.clone(),
            input.expires_at,
            normalize_metadata(input.metadata),
            input.created_by,
        )
        .await?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: Some(client.id),
                token_id: Some(token.id),
                actor_id: input
                    .audit_actor_id
                    .clone()
                    .or_else(|| input.created_by.map(|value| value.to_string())),
                actor_type: input
                    .audit_actor_type
                    .clone()
                    .or_else(|| input.created_by.map(|_| "human_user".to_string())),
                action: "token_rotated".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason: None,
                correlation_id: None,
                metadata: serde_json::json!({
                    "token_name": token_name,
                    "revoke_existing_tokens": input.revoke_existing_tokens,
                }),
                created_by: input.created_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;

        Ok(RotateMcpTokenResult {
            client,
            token,
            plaintext_token,
        })
    }

    #[tracing::instrument(
        skip(db, input),
        fields(tenant_id = %tenant_id, client_id = %client_id),
        err
    )]
    pub async fn update_policy(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
        input: UpdateMcpPolicyInput,
    ) -> Result<mcp_policies::Model> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let client = lock_client_for_update_txn(&txn, tenant_id, client_id).await?;

        let allowed_tools = dedupe(input.allowed_tools);
        let denied_tools = dedupe(input.denied_tools);
        let granted_permissions = dedupe(input.granted_permissions);
        let granted_scopes = dedupe(input.granted_scopes);
        let metadata = normalize_metadata(input.metadata);

        let policy = if let Some(existing) = mcp_policies::Entity::find()
            .filter(mcp_policies::Column::TenantId.eq(tenant_id))
            .filter(mcp_policies::Column::ClientId.eq(client.id))
            .one(&txn)
            .await
            .map_err(map_db_err)?
        {
            let mut active: mcp_policies::ActiveModel = existing.into();
            active.allowed_tools = Set(to_json_array(&allowed_tools)?);
            active.denied_tools = Set(to_json_array(&denied_tools)?);
            active.granted_permissions = Set(to_json_array(&granted_permissions)?);
            active.granted_scopes = Set(to_json_array(&granted_scopes)?);
            active.metadata = Set(metadata.clone());
            active.updated_by = Set(input.updated_by);
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await.map_err(map_db_err)?
        } else {
            mcp_policies::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                client_id: Set(client.id),
                allowed_tools: Set(to_json_array(&allowed_tools)?),
                denied_tools: Set(to_json_array(&denied_tools)?),
                granted_permissions: Set(to_json_array(&granted_permissions)?),
                granted_scopes: Set(to_json_array(&granted_scopes)?),
                metadata: Set(metadata.clone()),
                updated_by: Set(input.updated_by),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(&txn)
            .await
            .map_err(map_db_err)?
        };

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: Some(client.id),
                token_id: None,
                actor_id: input
                    .audit_actor_id
                    .clone()
                    .or_else(|| input.updated_by.map(|value| value.to_string())),
                actor_type: input
                    .audit_actor_type
                    .clone()
                    .or_else(|| input.updated_by.map(|_| "human_user".to_string())),
                action: "policy_updated".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason: None,
                correlation_id: None,
                metadata: serde_json::json!({
                    "allowed_tools": allowed_tools,
                    "denied_tools": denied_tools,
                }),
                created_by: input.updated_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(policy)
    }

    pub async fn revoke_token(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        token_id: Uuid,
        revoked_by: Option<Uuid>,
        actor_id: Option<String>,
        actor_type: Option<String>,
        reason: Option<String>,
    ) -> Result<mcp_tokens::Model> {
        let token = mcp_tokens::Entity::find_by_id(token_id)
            .filter(mcp_tokens::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(map_db_err)?
            .ok_or(Error::NotFound)?;

        let txn = db.begin().await.map_err(map_db_err)?;
        let mut active: mcp_tokens::ActiveModel = token.clone().into();
        active.revoked_at = Set(Some(Utc::now().into()));
        let updated = active.update(&txn).await.map_err(map_db_err)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: Some(updated.client_id),
                token_id: Some(updated.id),
                actor_id: actor_id
                    .clone()
                    .or_else(|| revoked_by.map(|value| value.to_string())),
                actor_type: actor_type
                    .clone()
                    .or_else(|| revoked_by.map(|_| "human_user".to_string())),
                action: "token_revoked".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason,
                correlation_id: None,
                metadata: serde_json::json!({
                    "token_name": updated.token_name,
                }),
                created_by: revoked_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(updated)
    }

    pub async fn deactivate_client(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
        revoked_by: Option<Uuid>,
        actor_id: Option<String>,
        actor_type: Option<String>,
        reason: Option<String>,
    ) -> Result<mcp_clients::Model> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let client = lock_client_for_update_txn(&txn, tenant_id, client_id).await?;

        revoke_active_tokens_txn(&txn, tenant_id, client.id).await?;

        let mut active: mcp_clients::ActiveModel = client.clone().into();
        active.is_active = Set(false);
        active.revoked_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let updated = active.update(&txn).await.map_err(map_db_err)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: Some(updated.id),
                token_id: None,
                actor_id: actor_id
                    .clone()
                    .or_else(|| revoked_by.map(|value| value.to_string())),
                actor_type: actor_type
                    .clone()
                    .or_else(|| revoked_by.map(|_| "human_user".to_string())),
                action: "client_deactivated".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason,
                correlation_id: None,
                metadata: serde_json::json!({
                    "slug": updated.slug,
                }),
                created_by: revoked_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(updated)
    }

    pub async fn list_audit_events(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        filters: McpAuditFilters,
    ) -> Result<Vec<mcp_audit_logs::Model>> {
        let limit = clamp_limit(filters.limit, 100, 200);
        let mut query = mcp_audit_logs::Entity::find()
            .filter(mcp_audit_logs::Column::TenantId.eq(tenant_id))
            .order_by_desc(mcp_audit_logs::Column::CreatedAt)
            .limit(limit);

        if let Some(client_id) = filters.client_id {
            query = query.filter(mcp_audit_logs::Column::ClientId.eq(client_id));
        }
        if let Some(outcome) = filters.outcome {
            query = query.filter(mcp_audit_logs::Column::Outcome.eq(outcome));
        }

        query.all(db).await.map_err(map_db_err)
    }

    pub async fn list_scaffold_drafts(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<mcp_scaffold_drafts::Model>> {
        let limit = clamp_limit(limit, 50, 100);
        mcp_scaffold_drafts::Entity::find_by_tenant(db, tenant_id, limit)
            .await
            .map_err(map_db_err)
    }

    pub async fn get_scaffold_draft(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        draft_id: Uuid,
    ) -> Result<Option<mcp_scaffold_drafts::Model>> {
        mcp_scaffold_drafts::Entity::find_by_id(draft_id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(map_db_err)
    }

    pub async fn get_scaffold_draft_for_client(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
        draft_id: Uuid,
    ) -> Result<Option<mcp_scaffold_drafts::Model>> {
        mcp_scaffold_drafts::Entity::find_by_id(draft_id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .filter(mcp_scaffold_drafts::Column::ClientId.eq(Some(client_id)))
            .one(db)
            .await
            .map_err(map_db_err)
    }

    pub async fn stage_scaffold_draft(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: StageMcpScaffoldDraftInput,
    ) -> Result<mcp_scaffold_drafts::Model> {
        if let Some(client_id) = input.client_id {
            require_client(db, tenant_id, client_id).await?;
        }

        validate_scaffold_slug_for_rust_type(&input.request.slug)?;
        let mut request = input.request;
        request.write_files = false;
        let preview = generate_module_scaffold(&request)
            .map_err(|error| Error::BadRequest(error.to_string()))?;

        let txn = db.begin().await.map_err(map_db_err)?;
        let draft = mcp_scaffold_drafts::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            client_id: Set(input.client_id),
            slug: Set(request.slug.clone()),
            crate_name: Set(preview.crate_name.clone()),
            status: Set(MCP_SCAFFOLD_STATUS_STAGED.to_string()),
            request_payload: Set(serde_json::to_value(&request)
                .map_err(|error| Error::BadRequest(error.to_string()))?),
            preview_payload: Set(serde_json::to_value(&preview)
                .map_err(|error| Error::BadRequest(error.to_string()))?),
            workspace_root: Set(None),
            applied_at: Set(None),
            created_by: Set(input.created_by),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(map_db_err)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: input.client_id,
                token_id: None,
                actor_id: input
                    .actor_id
                    .clone()
                    .or_else(|| input.created_by.map(|value| value.to_string())),
                actor_type: input
                    .actor_type
                    .clone()
                    .or_else(|| input.created_by.map(|_| "human_user".to_string())),
                action: "scaffold_draft_staged".to_string(),
                outcome: "success".to_string(),
                tool_name: Some("alloy_scaffold_module".to_string()),
                reason: None,
                correlation_id: Some(draft.id.to_string()),
                metadata: serde_json::json!({
                    "draft_id": draft.id,
                    "slug": draft.slug,
                    "crate_name": draft.crate_name,
                }),
                created_by: input.created_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(draft)
    }

    pub async fn apply_scaffold_draft(
        db: &DatabaseConnection,
        host: &dyn McpHostServices,
        tenant_id: Uuid,
        draft_id: Uuid,
        input: ApplyMcpScaffoldDraftInput,
    ) -> Result<(mcp_scaffold_drafts::Model, ApplyModuleScaffoldResponse)> {
        if !input.confirm {
            return Err(Error::BadRequest(
                "Refusing to apply scaffold draft without confirm=true".to_string(),
            ));
        }

        // The host authorizes and locks its workspace before the owner claims the draft.
        let publication = host
            .prepare_scaffold_workspace(&input.workspace_root)
            .await?;
        let workspace_root = publication.root().to_owned();
        let host_authority_actor_id = Some(publication.actor_id());

        let draft = Self::claim_scaffold_draft_for_apply(
            db,
            tenant_id,
            draft_id,
            input.applied_by,
            input.actor_id.clone(),
            input.actor_type.clone(),
            input.expected_client_id,
            &workspace_root,
        )
        .await?;

        let staged_draft = match draft.to_staged_draft() {
            Ok(staged_draft) => staged_draft,
            Err(error) => {
                let reason = format!("Persisted scaffold draft is invalid: {error}");
                // The draft was already claimed as APPLYING. Restore it to
                // STAGED and record the failed attempt instead of stranding it
                // in a state that rejects every subsequent apply request.
                Self::mark_scaffold_apply_failed(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    input.actor_id.clone(),
                    input.actor_type.clone(),
                    &workspace_root,
                    &reason,
                )
                .await?;
                return Err(Error::BadRequest(reason));
            }
        };

        if let Err(error) = publication.validate_target(&staged_draft).await {
            let reason = error.to_string();
            Self::mark_scaffold_apply_failed(
                db,
                tenant_id,
                &draft,
                input.applied_by,
                input.actor_id.clone(),
                input.actor_type.clone(),
                &workspace_root,
                &reason,
            )
            .await?;
            return Err(error);
        }

        // Scaffold generation writes several files synchronously. Keep that filesystem
        // work off Tokio's async workers; a panic or partial write is quarantined below.
        let apply_result = match tokio::task::spawn_blocking(move || {
            publication.publish(&staged_draft)
        })
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let reason = error.to_string();
                if scaffold_apply_error_requires_recovery(&error) {
                    tracing::error!(
                        draft_id = %draft_id,
                        error = %error,
                        "MCP scaffold filesystem write failed; workspace may contain partial files"
                    );
                    let recovery_reason = "Scaffold filesystem write failed; the target workspace may contain partial files and requires manual reconciliation";
                    match Self::mark_scaffold_apply_recovery_required(
                        db,
                        tenant_id,
                        &draft,
                        input.applied_by,
                        input.actor_id.clone(),
                        input.actor_type.clone(),
                        &workspace_root,
                        host_authority_actor_id,
                        recovery_reason,
                    )
                    .await
                    {
                        Ok(()) => (),
                        Err(mark_error) => tracing::error!(
                            draft_id = %draft_id,
                            error = ?mark_error,
                            "Failed to persist MCP scaffold recovery-required state"
                        ),
                    }
                    return Err(Error::InternalServerError);
                }

                Self::mark_scaffold_apply_failed(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    input.actor_id.clone(),
                    input.actor_type.clone(),
                    &workspace_root,
                    &reason,
                )
                .await?;
                return Err(Error::BadRequest(reason));
            }
            Err(join_error) if join_error.is_cancelled() => {
                let reason = "Scaffold apply worker was cancelled before it completed";
                Self::mark_scaffold_apply_failed(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    input.actor_id.clone(),
                    input.actor_type.clone(),
                    &workspace_root,
                    reason,
                )
                .await?;
                return Err(Error::InternalServerError);
            }
            Err(join_error) => {
                tracing::error!(
                    draft_id = %draft_id,
                    error = %join_error,
                    "MCP scaffold apply worker panicked; workspace may contain partial files"
                );
                let recovery_reason = "Scaffold apply worker panicked; the target workspace may contain partial files and requires manual reconciliation";
                match Self::mark_scaffold_apply_recovery_required(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    input.actor_id.clone(),
                    input.actor_type.clone(),
                    &workspace_root,
                    host_authority_actor_id,
                    recovery_reason,
                )
                .await
                {
                    Ok(()) => (),
                    Err(mark_error) => tracing::error!(
                        draft_id = %draft_id,
                        error = ?mark_error,
                        "Failed to persist MCP scaffold recovery-required state"
                    ),
                }
                return Err(Error::InternalServerError);
            }
        };

        let finalize_result = async {
            let txn = db.begin().await.map_err(map_db_err)?;
            let now: sea_orm::entity::prelude::DateTimeWithTimeZone = Utc::now().into();
            let result = mcp_scaffold_drafts::Entity::update_many()
                .filter(mcp_scaffold_drafts::Column::Id.eq(draft.id))
                .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
                .filter(mcp_scaffold_drafts::Column::Status.eq(MCP_SCAFFOLD_STATUS_APPLYING))
                .col_expr(
                    mcp_scaffold_drafts::Column::Status,
                    Expr::value(MCP_SCAFFOLD_STATUS_APPLIED),
                )
                .col_expr(
                    mcp_scaffold_drafts::Column::WorkspaceRoot,
                    Expr::value(Some(workspace_root.clone())),
                )
                .col_expr(
                    mcp_scaffold_drafts::Column::AppliedAt,
                    Expr::value(Some(now.clone())),
                )
                .col_expr(mcp_scaffold_drafts::Column::UpdatedAt, Expr::value(now))
                .exec(&txn)
                .await
                .map_err(map_db_err)?;

            if result.rows_affected == 0 {
                return Err(Error::BadRequest(
                    "Scaffold draft state changed before apply finalization".to_string(),
                ));
            }

            let updated = mcp_scaffold_drafts::Entity::find_by_id(draft.id)
                .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
                .one(&txn)
                .await
                .map_err(map_db_err)?
                .ok_or(Error::NotFound)?;

            Self::record_audit_event_txn(
                &txn,
                RecordMcpAuditEventInput {
                    tenant_id,
                    client_id: updated.client_id,
                    token_id: None,
                    actor_id: input
                        .actor_id
                        .clone()
                        .or_else(|| input.applied_by.map(|value| value.to_string())),
                    actor_type: input
                        .actor_type
                        .clone()
                        .or_else(|| input.applied_by.map(|_| "human_user".to_string())),
                    action: "scaffold_draft_applied".to_string(),
                    outcome: "success".to_string(),
                    tool_name: Some("alloy_apply_module_scaffold".to_string()),
                    reason: None,
                    correlation_id: Some(updated.id.to_string()),
                    metadata: serde_json::json!({
                        "draft_id": updated.id,
                        "slug": updated.slug,
                        "crate_name": updated.crate_name,
                        "workspace_root": workspace_root.clone(),
                        "host_authority_actor_id": host_authority_actor_id,
                        "host_authority": "manage",
                    }),
                    created_by: input.applied_by,
                },
            )
            .await?;

            txn.commit().await.map_err(map_db_err)?;
            Ok::<mcp_scaffold_drafts::Model, Error>(updated)
        }
        .await;

        let updated = match finalize_result {
            Ok(updated) => updated,
            Err(error) => {
                let recovery_reason = "Scaffold files were written but database finalization failed; the target workspace requires manual reconciliation";
                match Self::mark_scaffold_apply_recovery_required(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    input.actor_id.clone(),
                    input.actor_type.clone(),
                    &workspace_root,
                    host_authority_actor_id,
                    recovery_reason,
                )
                .await
                {
                    Ok(()) => (),
                    Err(mark_error) => tracing::error!(
                        draft_id = %draft_id,
                        error = ?mark_error,
                        "Failed to persist MCP scaffold recovery-required state after finalization failure"
                    ),
                }
                return Err(error);
            }
        };
        Ok((updated, apply_result))
    }

    async fn claim_scaffold_draft_for_apply(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        draft_id: Uuid,
        applied_by: Option<Uuid>,
        actor_id: Option<String>,
        actor_type: Option<String>,
        expected_client_id: Option<Uuid>,
        workspace_root: &str,
    ) -> Result<mcp_scaffold_drafts::Model> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let mut claim = mcp_scaffold_drafts::Entity::update_many()
            .filter(mcp_scaffold_drafts::Column::Id.eq(draft_id))
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .filter(mcp_scaffold_drafts::Column::Status.eq(MCP_SCAFFOLD_STATUS_STAGED));
        if let Some(client_id) = expected_client_id {
            claim = claim.filter(mcp_scaffold_drafts::Column::ClientId.eq(Some(client_id)));
        }
        let result = claim
            .col_expr(
                mcp_scaffold_drafts::Column::Status,
                Expr::value(MCP_SCAFFOLD_STATUS_APPLYING),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::WorkspaceRoot,
                Expr::value(Some(workspace_root.to_string())),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .exec(&txn)
            .await
            .map_err(map_db_err)?;

        if result.rows_affected == 0 {
            let mut existing_query = mcp_scaffold_drafts::Entity::find_by_id(draft_id)
                .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id));
            if let Some(client_id) = expected_client_id {
                existing_query = existing_query
                    .filter(mcp_scaffold_drafts::Column::ClientId.eq(Some(client_id)));
            }
            let existing = existing_query
                .one(&txn)
                .await
                .map_err(map_db_err)?
                .ok_or(Error::NotFound)?;
            return Err(scaffold_status_error(draft_id, &existing.status));
        }

        let mut claimed_query = mcp_scaffold_drafts::Entity::find_by_id(draft_id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id));
        if let Some(client_id) = expected_client_id {
            claimed_query =
                claimed_query.filter(mcp_scaffold_drafts::Column::ClientId.eq(Some(client_id)));
        }
        let claimed = claimed_query
            .one(&txn)
            .await
            .map_err(map_db_err)?
            .ok_or(Error::NotFound)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: claimed.client_id,
                token_id: None,
                actor_id: actor_id
                    .clone()
                    .or_else(|| applied_by.map(|value| value.to_string())),
                actor_type: actor_type
                    .clone()
                    .or_else(|| applied_by.map(|_| "human_user".to_string())),
                action: "scaffold_draft_apply_started".to_string(),
                outcome: "pending".to_string(),
                tool_name: Some("alloy_apply_module_scaffold".to_string()),
                reason: None,
                correlation_id: Some(claimed.id.to_string()),
                metadata: serde_json::json!({
                    "draft_id": claimed.id,
                    "slug": claimed.slug,
                    "crate_name": claimed.crate_name,
                    "workspace_root": workspace_root,
                }),
                created_by: applied_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(claimed)
    }

    async fn mark_scaffold_apply_failed(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        draft: &mcp_scaffold_drafts::Model,
        applied_by: Option<Uuid>,
        actor_id: Option<String>,
        actor_type: Option<String>,
        workspace_root: &str,
        reason: &str,
    ) -> Result<()> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let result = mcp_scaffold_drafts::Entity::update_many()
            .filter(mcp_scaffold_drafts::Column::Id.eq(draft.id))
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .filter(mcp_scaffold_drafts::Column::Status.eq(MCP_SCAFFOLD_STATUS_APPLYING))
            .col_expr(
                mcp_scaffold_drafts::Column::Status,
                Expr::value(MCP_SCAFFOLD_STATUS_STAGED),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::WorkspaceRoot,
                Expr::value(Some(workspace_root.to_string())),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .exec(&txn)
            .await
            .map_err(map_db_err)?;

        if result.rows_affected == 0 {
            txn.rollback().await.map_err(map_db_err)?;
            return Ok(());
        }

        let updated = mcp_scaffold_drafts::Entity::find_by_id(draft.id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await
            .map_err(map_db_err)?
            .ok_or(Error::NotFound)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: updated.client_id,
                token_id: None,
                actor_id: actor_id
                    .clone()
                    .or_else(|| applied_by.map(|value| value.to_string())),
                actor_type: actor_type
                    .clone()
                    .or_else(|| applied_by.map(|_| "human_user".to_string())),
                action: "scaffold_draft_apply_failed".to_string(),
                outcome: "failure".to_string(),
                tool_name: Some("alloy_apply_module_scaffold".to_string()),
                reason: Some(reason.to_string()),
                correlation_id: Some(updated.id.to_string()),
                metadata: serde_json::json!({
                    "draft_id": updated.id,
                    "slug": updated.slug,
                    "crate_name": updated.crate_name,
                    "workspace_root": workspace_root,
                }),
                created_by: applied_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(())
    }

    async fn mark_scaffold_apply_recovery_required(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        draft: &mcp_scaffold_drafts::Model,
        applied_by: Option<Uuid>,
        actor_id: Option<String>,
        actor_type: Option<String>,
        workspace_root: &str,
        host_authority_actor_id: Option<Uuid>,
        reason: &str,
    ) -> Result<()> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let result = mcp_scaffold_drafts::Entity::update_many()
            .filter(mcp_scaffold_drafts::Column::Id.eq(draft.id))
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .filter(mcp_scaffold_drafts::Column::Status.eq(MCP_SCAFFOLD_STATUS_APPLYING))
            .col_expr(
                mcp_scaffold_drafts::Column::Status,
                Expr::value(MCP_SCAFFOLD_STATUS_RECOVERY_REQUIRED),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::WorkspaceRoot,
                Expr::value(Some(workspace_root.to_string())),
            )
            .col_expr(
                mcp_scaffold_drafts::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .exec(&txn)
            .await
            .map_err(map_db_err)?;

        if result.rows_affected == 0 {
            txn.rollback().await.map_err(map_db_err)?;
            return Ok(());
        }

        let updated = mcp_scaffold_drafts::Entity::find_by_id(draft.id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await
            .map_err(map_db_err)?
            .ok_or(Error::NotFound)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: updated.client_id,
                token_id: None,
                actor_id: actor_id
                    .clone()
                    .or_else(|| applied_by.map(|value| value.to_string())),
                actor_type: actor_type
                    .clone()
                    .or_else(|| applied_by.map(|_| "human_user".to_string())),
                action: "scaffold_draft_apply_recovery_required".to_string(),
                outcome: "recovery_required".to_string(),
                tool_name: Some("alloy_apply_module_scaffold".to_string()),
                reason: Some(reason.to_string()),
                correlation_id: Some(updated.id.to_string()),
                metadata: serde_json::json!({
                    "draft_id": updated.id,
                    "slug": updated.slug,
                    "crate_name": updated.crate_name,
                    "workspace_root": workspace_root,
                    "host_authority_actor_id": host_authority_actor_id,
                    "host_authority": "manage",
                }),
                created_by: applied_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok(())
    }

    pub async fn record_audit_event(
        db: &DatabaseConnection,
        input: RecordMcpAuditEventInput,
    ) -> Result<mcp_audit_logs::Model> {
        Self::record_audit_event_txn(db, input).await
    }

    async fn record_audit_event_txn<C>(
        db: &C,
        input: RecordMcpAuditEventInput,
    ) -> Result<mcp_audit_logs::Model>
    where
        C: ConnectionTrait,
    {
        mcp_audit_logs::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(input.tenant_id),
            client_id: Set(input.client_id),
            token_id: Set(input.token_id),
            actor_id: Set(input.actor_id),
            actor_type: Set(input.actor_type),
            action: Set(input.action),
            outcome: Set(input.outcome),
            tool_name: Set(input.tool_name),
            reason: Set(input.reason),
            correlation_id: Set(input.correlation_id),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(input.created_by),
            created_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(map_db_err)
    }
}

async fn lock_client_for_update_txn(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    client_id: Uuid,
) -> Result<mcp_clients::Model> {
    let query = mcp_clients::Entity::find_by_id(client_id)
        .filter(mcp_clients::Column::TenantId.eq(tenant_id));

    let client = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await.map_err(map_db_err)?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE mcp_clients SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), client_id.into()],
            );
            txn.execute_raw(statement).await.map_err(map_db_err)?;
            query.one(txn).await.map_err(map_db_err)?
        }
        _ => query.one(txn).await.map_err(map_db_err)?,
    };

    client.ok_or(Error::NotFound)
}

async fn require_client(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    client_id: Uuid,
) -> Result<mcp_clients::Model> {
    mcp_clients::Entity::find_by_id(client_id)
        .filter(mcp_clients::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(map_db_err)?
        .ok_or(Error::NotFound)
}

async fn revoke_active_tokens_txn<C>(db: &C, tenant_id: Uuid, client_id: Uuid) -> Result<()>
where
    C: ConnectionTrait,
{
    let tokens = mcp_tokens::Entity::find()
        .filter(mcp_tokens::Column::TenantId.eq(tenant_id))
        .filter(mcp_tokens::Column::ClientId.eq(client_id))
        .filter(mcp_tokens::Column::RevokedAt.is_null())
        .all(db)
        .await
        .map_err(map_db_err)?;

    for token in tokens {
        let mut active: mcp_tokens::ActiveModel = token.into();
        active.revoked_at = Set(Some(Utc::now().into()));
        active.update(db).await.map_err(map_db_err)?;
    }

    Ok(())
}

async fn insert_token<C>(
    db: &C,
    tenant_id: Uuid,
    client_id: Uuid,
    token_name: String,
    expires_at: Option<DateTime<Utc>>,
    metadata: serde_json::Value,
    created_by: Option<Uuid>,
) -> Result<(mcp_tokens::Model, String)>
where
    C: ConnectionTrait,
{
    let plaintext_token = generate_token();
    let token = mcp_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        client_id: Set(client_id),
        token_name: Set(token_name),
        token_preview: Set(token_preview(&plaintext_token)),
        token_hash: Set(hash_token(&plaintext_token)),
        created_by: Set(created_by),
        last_used_at: Set(None),
        expires_at: Set(expires_at.map(Into::into)),
        revoked_at: Set(None),
        metadata: Set(metadata),
        created_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .map_err(map_db_err)?;

    Ok((token, plaintext_token))
}

pub(super) fn identity_for_client(
    client: &mcp_clients::Model,
    policy: &mcp_policies::Model,
) -> std::result::Result<McpIdentity, String> {
    Ok(McpIdentity {
        actor_id: client.id.to_string(),
        actor_type: client.actor_type(),
        tenant_id: Some(client.tenant_id.to_string()),
        delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
        display_name: Some(client.display_name.clone()),
        scopes: policy.granted_scopes_list()?,
    })
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    let payload = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    format!("mcp_{payload}")
}

pub(super) fn hash_token(token: &str) -> String {
    hex::encode(rustok_api::sha256_digest(&[token.as_bytes()]))
}

fn token_preview(token: &str) -> String {
    let visible = token.chars().take(12).collect::<String>();
    format!("{visible}...")
}

fn clamp_limit(limit: Option<u64>, default: u64, max: u64) -> u64 {
    limit.unwrap_or(default).clamp(1, max)
}

fn validate_mcp_label(value: &str, field: &str, max_chars: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::BadRequest(format!("MCP {field} must not be empty")));
    }
    if value.chars().count() > max_chars {
        return Err(Error::BadRequest(format!(
            "MCP {field} must be {max_chars} characters or fewer"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(Error::BadRequest(format!(
            "MCP {field} must not contain control characters"
        )));
    }
    Ok(())
}

fn validate_scaffold_slug_for_rust_type(slug: &str) -> Result<()> {
    let slug = slug.trim();
    if !slug
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
    {
        return Err(Error::BadRequest(
            "Scaffold slug must start with a lowercase ASCII letter so its generated Rust module type is valid"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_token_expiry(expires_at: Option<&DateTime<Utc>>) -> Result<()> {
    if expires_at.is_some_and(|expires_at| expires_at <= &Utc::now()) {
        return Err(Error::BadRequest(
            "MCP token expiry must be in the future".to_string(),
        ));
    }
    Ok(())
}

fn validate_slug(slug: &str) -> Result<()> {
    if slug.trim().is_empty() {
        return Err(Error::BadRequest(
            "MCP client slug must not be empty".to_string(),
        ));
    }
    if slug.len() > 96 {
        return Err(Error::BadRequest(
            "MCP client slug must be 96 characters or fewer".to_string(),
        ));
    }
    Ok(())
}

fn scaffold_apply_error_requires_recovery(
    error: &crate::alloy_scaffold::AlloyScaffoldError,
) -> bool {
    matches!(
        error,
        crate::alloy_scaffold::AlloyScaffoldError::FileWrite(_)
    )
}

fn scaffold_status_error(draft_id: Uuid, status: &str) -> Error {
    match status {
        MCP_SCAFFOLD_STATUS_APPLIED => Error::BadRequest(format!(
            "Scaffold draft {} has already been applied",
            draft_id
        )),
        MCP_SCAFFOLD_STATUS_APPLYING => Error::BadRequest(format!(
            "Scaffold draft {} is applying or was interrupted; verify and reconcile the target workspace before retrying",
            draft_id
        )),
        MCP_SCAFFOLD_STATUS_RECOVERY_REQUIRED => Error::BadRequest(format!(
            "Scaffold draft {} requires manual workspace reconciliation before it can be applied again",
            draft_id
        )),
        other => Error::BadRequest(format!(
            "Scaffold draft {} cannot be applied from status `{}`",
            draft_id, other
        )),
    }
}

fn to_json_array(values: &[String]) -> Result<serde_json::Value> {
    serde_json::to_value(values).map_err(|err| Error::BadRequest(err.to_string()))
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalize_metadata(value: serde_json::Value) -> serde_json::Value {
    if value.is_null() {
        serde_json::json!({})
    } else {
        value
    }
}

fn map_db_err(_err: sea_orm::DbErr) -> Error {
    tracing::error!(
        error_code = "mcp_management_database_operation_failed",
        "MCP management database operation failed"
    );
    Error::InternalServerError
}

#[cfg(test)]
mod tests {
    use super::Error;
    use super::{
        MCP_SCAFFOLD_STATUS_APPLYING, MCP_SCAFFOLD_STATUS_RECOVERY_REQUIRED, dedupe,
        scaffold_apply_error_requires_recovery, scaffold_status_error, token_preview,
        validate_mcp_label, validate_scaffold_slug_for_rust_type, validate_token_expiry,
    };
    use crate::alloy_scaffold::AlloyScaffoldError;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn partial_scaffold_file_write_requires_manual_reconciliation() {
        let error = AlloyScaffoldError::FileWrite(
            "Failed to write crates/modules/example/Cargo.toml: disk full".to_string(),
        );
        assert!(scaffold_apply_error_requires_recovery(&error));

        let preflight_error =
            AlloyScaffoldError::TargetAlreadyExists("crates/modules/example".to_string());
        assert!(!scaffold_apply_error_requires_recovery(&preflight_error));

        let preflight_workspace_error =
            AlloyScaffoldError::Workspace("workspace path is invalid".to_string());
        assert!(!scaffold_apply_error_requires_recovery(
            &preflight_workspace_error
        ));
    }

    #[test]
    fn interrupted_scaffold_apply_requires_workspace_reconciliation() {
        let error = scaffold_status_error(Uuid::new_v4(), MCP_SCAFFOLD_STATUS_APPLYING);
        assert!(matches!(
            error,
            Error::BadRequest(message) if message.contains("reconcile the target workspace")
        ));
    }

    #[test]
    fn recovery_required_scaffold_drafts_cannot_be_reapplied() {
        let error = scaffold_status_error(Uuid::new_v4(), MCP_SCAFFOLD_STATUS_RECOVERY_REQUIRED);
        assert!(matches!(
            error,
            Error::BadRequest(message) if message.contains("manual workspace reconciliation")
        ));
    }

    #[test]
    fn mcp_labels_respect_database_lengths_and_reject_control_characters() {
        assert!(validate_mcp_label("API client", "display name", 160).is_ok());
        assert!(validate_mcp_label("primary", "token name", 128).is_ok());
        assert!(validate_mcp_label("  ", "display name", 160).is_err());
        assert!(validate_mcp_label(&"x".repeat(161), "display name", 160).is_err());
        assert!(validate_mcp_label("primary\nrotated", "token name", 128).is_err());
    }

    #[test]
    fn scaffold_slug_must_produce_a_valid_rust_module_identifier() {
        assert!(validate_scaffold_slug_for_rust_type("newsletter").is_ok());
        assert!(validate_scaffold_slug_for_rust_type("newsletter_2").is_ok());
        assert!(validate_scaffold_slug_for_rust_type("  newsletter ").is_ok());
        assert!(validate_scaffold_slug_for_rust_type("123-newsletter").is_err());
        assert!(validate_scaffold_slug_for_rust_type("_newsletter").is_err());
        assert!(validate_scaffold_slug_for_rust_type("---").is_err());
    }

    #[test]
    fn token_expiry_must_be_in_the_future() {
        let past = Utc::now() - chrono::Duration::seconds(1);
        let future = Utc::now() + chrono::Duration::seconds(60);

        assert!(validate_token_expiry(None).is_ok());
        assert!(validate_token_expiry(Some(&future)).is_ok());
        assert!(validate_token_expiry(Some(&past)).is_err());
    }

    #[test]
    fn dedupe_sorts_and_trims_values() {
        let values = dedupe(vec![
            " tool.z ".to_string(),
            "tool.a".to_string(),
            "tool.z".to_string(),
            "".to_string(),
        ]);

        assert_eq!(values, vec!["tool.a".to_string(), "tool.z".to_string()]);
    }

    #[test]
    fn token_preview_does_not_leak_full_secret() {
        let preview = token_preview("mcp_super_secret_token_value");
        assert!(preview.starts_with("mcp_super_se"));
        assert!(preview.ends_with("..."));
        assert_ne!(preview, "mcp_super_secret_token_value");
    }
}

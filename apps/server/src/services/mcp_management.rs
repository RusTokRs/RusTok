use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::Expr,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::{mcp_audit_logs, mcp_clients, mcp_policies, mcp_scaffold_drafts, mcp_tokens};
use rustok_mcp::{
    ApplyModuleScaffoldResponse, McpAccessContext, McpActorType, McpIdentity,
    ScaffoldModuleRequest, apply_staged_scaffold, generate_module_scaffold,
};

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
}

#[derive(Debug, Clone)]
pub struct RotateMcpTokenInput {
    pub token_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
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
}

#[derive(Debug, Clone)]
pub struct StageMcpScaffoldDraftInput {
    pub client_id: Option<Uuid>,
    pub request: ScaffoldModuleRequest,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ApplyMcpScaffoldDraftInput {
    pub workspace_root: String,
    pub confirm: bool,
    pub applied_by: Option<Uuid>,
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

impl McpManagementService {
    pub async fn create_client(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: CreateMcpClientInput,
    ) -> Result<CreateMcpClientResult> {
        validate_slug(&input.slug)?;
        let token_name = input
            .token_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "primary".to_string());
        let actor_type = actor_type_slug(input.actor_type);
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
                actor_id: input.created_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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

        let policy = mcp_policies::Entity::find_by_client(db, client.id)
            .await
            .map_err(map_db_err)?;
        let tokens = mcp_tokens::Entity::find_by_client(db, client.id)
            .await
            .map_err(map_db_err)?;
        let effective_access_context = policy.as_ref().map(|policy| {
            policy.to_access_context(&client, Some(identity_for_client(&client, policy)))
        });

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
    ) -> Result<RotateMcpTokenResult> {
        let client = require_client(db, tenant_id, client_id).await?;
        let token_name = input
            .token_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "rotated".to_string());
        let txn = db.begin().await.map_err(map_db_err)?;

        if input.revoke_existing_tokens {
            revoke_active_tokens_txn(&txn, client.id).await?;
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
                actor_id: input.created_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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

    pub async fn update_policy(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
        input: UpdateMcpPolicyInput,
    ) -> Result<mcp_policies::Model> {
        let client = require_client(db, tenant_id, client_id).await?;
        let txn = db.begin().await.map_err(map_db_err)?;

        let allowed_tools = dedupe(input.allowed_tools);
        let denied_tools = dedupe(input.denied_tools);
        let granted_permissions = dedupe(input.granted_permissions);
        let granted_scopes = dedupe(input.granted_scopes);
        let metadata = normalize_metadata(input.metadata);

        let policy = if let Some(existing) = mcp_policies::Entity::find()
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
                actor_id: input.updated_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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
                actor_id: revoked_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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
        reason: Option<String>,
    ) -> Result<mcp_clients::Model> {
        let client = require_client(db, tenant_id, client_id).await?;
        let txn = db.begin().await.map_err(map_db_err)?;

        revoke_active_tokens_txn(&txn, client.id).await?;

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
                actor_id: revoked_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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

    pub async fn stage_scaffold_draft(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: StageMcpScaffoldDraftInput,
    ) -> Result<mcp_scaffold_drafts::Model> {
        if let Some(client_id) = input.client_id {
            let _ = require_client(db, tenant_id, client_id).await?;
        }

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
                actor_id: input.created_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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
        tenant_id: Uuid,
        draft_id: Uuid,
        input: ApplyMcpScaffoldDraftInput,
    ) -> Result<(mcp_scaffold_drafts::Model, ApplyModuleScaffoldResponse)> {
        if !input.confirm {
            return Err(Error::BadRequest(
                "Refusing to apply scaffold draft without confirm=true".to_string(),
            ));
        }

        let draft = Self::claim_scaffold_draft_for_apply(
            db,
            tenant_id,
            draft_id,
            input.applied_by,
            &input.workspace_root,
        )
        .await?;

        let staged_draft = draft
            .to_staged_draft()
            .map_err(|error| Error::BadRequest(error.to_string()))?;
        let apply_result = match apply_staged_scaffold(&staged_draft, &input.workspace_root) {
            Ok(response) => response,
            Err(error) => {
                Self::mark_scaffold_apply_failed(
                    db,
                    tenant_id,
                    &draft,
                    input.applied_by,
                    &input.workspace_root,
                    &error,
                )
                .await?;
                return Err(Error::BadRequest(error));
            }
        };

        let txn = db.begin().await.map_err(map_db_err)?;
        let mut active: mcp_scaffold_drafts::ActiveModel = draft.clone().into();
        active.status = Set(MCP_SCAFFOLD_STATUS_APPLIED.to_string());
        active.workspace_root = Set(Some(input.workspace_root.clone()));
        active.applied_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let updated = active.update(&txn).await.map_err(map_db_err)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: updated.client_id,
                token_id: None,
                actor_id: input.applied_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
                action: "scaffold_draft_applied".to_string(),
                outcome: "success".to_string(),
                tool_name: Some("alloy_apply_module_scaffold".to_string()),
                reason: None,
                correlation_id: Some(updated.id.to_string()),
                metadata: serde_json::json!({
                    "draft_id": updated.id,
                    "slug": updated.slug,
                    "crate_name": updated.crate_name,
                    "workspace_root": input.workspace_root,
                }),
                created_by: input.applied_by,
            },
        )
        .await?;

        txn.commit().await.map_err(map_db_err)?;
        Ok((updated, apply_result))
    }

    async fn claim_scaffold_draft_for_apply(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        draft_id: Uuid,
        applied_by: Option<Uuid>,
        workspace_root: &str,
    ) -> Result<mcp_scaffold_drafts::Model> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let result = mcp_scaffold_drafts::Entity::update_many()
            .filter(mcp_scaffold_drafts::Column::Id.eq(draft_id))
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
            .filter(mcp_scaffold_drafts::Column::Status.eq(MCP_SCAFFOLD_STATUS_STAGED))
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
            let existing = mcp_scaffold_drafts::Entity::find_by_id(draft_id)
                .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
                .one(&txn)
                .await
                .map_err(map_db_err)?
                .ok_or(Error::NotFound)?;
            return Err(scaffold_status_error(draft_id, &existing.status));
        }

        let claimed = mcp_scaffold_drafts::Entity::find_by_id(draft_id)
            .filter(mcp_scaffold_drafts::Column::TenantId.eq(tenant_id))
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
                actor_id: applied_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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
        workspace_root: &str,
        reason: &str,
    ) -> Result<()> {
        let txn = db.begin().await.map_err(map_db_err)?;
        let mut active: mcp_scaffold_drafts::ActiveModel = draft.clone().into();
        active.status = Set(MCP_SCAFFOLD_STATUS_STAGED.to_string());
        active.workspace_root = Set(Some(workspace_root.to_string()));
        active.updated_at = Set(Utc::now().into());
        let updated = active.update(&txn).await.map_err(map_db_err)?;

        Self::record_audit_event_txn(
            &txn,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: updated.client_id,
                token_id: None,
                actor_id: applied_by.map(|value| value.to_string()),
                actor_type: Some("human_user".to_string()),
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

async fn revoke_active_tokens_txn<C>(db: &C, client_id: Uuid) -> Result<()>
where
    C: ConnectionTrait,
{
    let tokens = mcp_tokens::Entity::find()
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

fn identity_for_client(client: &mcp_clients::Model, policy: &mcp_policies::Model) -> McpIdentity {
    McpIdentity {
        actor_id: client.id.to_string(),
        actor_type: client.actor_type(),
        tenant_id: Some(client.tenant_id.to_string()),
        delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
        display_name: Some(client.display_name.clone()),
        scopes: policy.granted_scopes_list(),
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    let payload = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    format!("mcp_{payload}")
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn token_preview(token: &str) -> String {
    let visible = token.chars().take(12).collect::<String>();
    format!("{visible}...")
}

fn actor_type_slug(actor_type: McpActorType) -> &'static str {
    match actor_type {
        McpActorType::HumanUser => "human_user",
        McpActorType::ServiceClient => "service_client",
        McpActorType::ModelAgent => "model_agent",
    }
}

fn clamp_limit(limit: Option<u64>, default: u64, max: u64) -> u64 {
    limit.unwrap_or(default).clamp(1, max)
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

fn scaffold_status_error(draft_id: Uuid, status: &str) -> Error {
    match status {
        MCP_SCAFFOLD_STATUS_APPLIED => Error::BadRequest(format!(
            "Scaffold draft {} has already been applied",
            draft_id
        )),
        MCP_SCAFFOLD_STATUS_APPLYING => Error::BadRequest(format!(
            "Scaffold draft {} is already being applied",
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

fn map_db_err(err: sea_orm::DbErr) -> Error {
    Error::BadRequest(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{dedupe, token_preview};

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

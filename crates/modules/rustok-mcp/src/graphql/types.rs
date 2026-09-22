use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    McpAccessContext, McpActorType, McpAuditEventRecord, McpClientDetailsRecord, McpClientRecord,
    McpPolicyRecord, McpScaffoldDraftRecord, McpTokenRecord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum McpActorTypeGql {
    HumanUser,
    ServiceClient,
    ModelAgent,
}

impl McpActorTypeGql {
    pub fn to_runtime(self) -> McpActorType {
        match self {
            Self::HumanUser => McpActorType::HumanUser,
            Self::ServiceClient => McpActorType::ServiceClient,
            Self::ModelAgent => McpActorType::ModelAgent,
        }
    }
}

impl From<McpActorType> for McpActorTypeGql {
    fn from(value: McpActorType) -> Self {
        match value {
            McpActorType::HumanUser => Self::HumanUser,
            McpActorType::ServiceClient => Self::ServiceClient,
            McpActorType::ModelAgent => Self::ModelAgent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum McpScaffoldDraftStatusGql {
    Staged,
    Applied,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpClientGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub client_key: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorTypeGql,
    pub delegated_user_id: Option<Uuid>,
    pub is_active: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub metadata: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<McpClientRecord> for McpClientGql {
    fn from(value: McpClientRecord) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            client_key: value.client_key,
            slug: value.slug,
            display_name: value.display_name,
            description: value.description,
            actor_type: value.actor_type.into(),
            delegated_user_id: value.delegated_user_id,
            is_active: value.is_active,
            revoked_at: value.revoked_at,
            last_used_at: value.last_used_at,
            metadata: value.metadata.to_string(),
            created_by: value.created_by,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpPolicyGql {
    pub id: Uuid,
    pub client_id: Uuid,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: String,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<McpPolicyRecord> for McpPolicyGql {
    fn from(value: McpPolicyRecord) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            allowed_tools: value.allowed_tools,
            denied_tools: value.denied_tools,
            granted_permissions: value.granted_permissions,
            granted_scopes: value.granted_scopes,
            metadata: value.metadata.to_string(),
            updated_by: value.updated_by,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpTokenGql {
    pub id: Uuid,
    pub client_id: Uuid,
    pub token_name: String,
    pub token_preview: String,
    pub is_active: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
}

impl From<McpTokenRecord> for McpTokenGql {
    fn from(value: McpTokenRecord) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            token_name: value.token_name,
            token_preview: value.token_preview,
            is_active: value.is_active,
            last_used_at: value.last_used_at,
            expires_at: value.expires_at,
            revoked_at: value.revoked_at,
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpAuditEventGql {
    pub id: Uuid,
    pub client_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub action: String,
    pub outcome: String,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<McpAuditEventRecord> for McpAuditEventGql {
    fn from(value: McpAuditEventRecord) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            token_id: value.token_id,
            actor_id: value.actor_id,
            actor_type: value.actor_type,
            action: value.action,
            outcome: value.outcome,
            tool_name: value.tool_name,
            reason: value.reason,
            correlation_id: value.correlation_id,
            metadata: value.metadata.to_string(),
            created_by: value.created_by,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpClientDetailsGql {
    pub client: McpClientGql,
    pub policy: Option<McpPolicyGql>,
    pub tokens: Vec<McpTokenGql>,
    pub effective_access_context: Option<String>,
}

impl TryFrom<McpClientDetailsRecord> for McpClientDetailsGql {
    type Error = async_graphql::Error;

    fn try_from(value: McpClientDetailsRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            client: value.client.into(),
            policy: value.policy.map(Into::into),
            tokens: value.tokens.into_iter().map(Into::into).collect(),
            effective_access_context: serialize_access_context(
                value.effective_access_context.as_ref(),
            )?,
        })
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct McpModuleScaffoldDraftGql {
    pub id: Uuid,
    pub client_id: Option<Uuid>,
    pub slug: String,
    pub crate_name: String,
    pub status: McpScaffoldDraftStatusGql,
    pub request_json: String,
    pub preview_json: String,
    pub workspace_root: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<McpScaffoldDraftRecord> for McpModuleScaffoldDraftGql {
    type Error = async_graphql::Error;

    fn try_from(value: McpScaffoldDraftRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            client_id: value.client_id,
            slug: value.slug,
            crate_name: value.crate_name,
            status: if value.status == "applied" {
                McpScaffoldDraftStatusGql::Applied
            } else {
                McpScaffoldDraftStatusGql::Staged
            },
            request_json: serde_json::to_string(&value.request_payload)
                .map_err(|error| async_graphql::Error::new(error.to_string()))?,
            preview_json: serde_json::to_string(&value.preview_payload)
                .map_err(|error| async_graphql::Error::new(error.to_string()))?,
            workspace_root: value.workspace_root,
            applied_at: value.applied_at,
            created_by: value.created_by,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct CreateMcpClientResultGql {
    pub client: McpClientGql,
    pub policy: McpPolicyGql,
    pub token: McpTokenGql,
    pub plaintext_token: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RotateMcpTokenResultGql {
    pub client: McpClientGql,
    pub token: McpTokenGql,
    pub plaintext_token: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateMcpClientInput {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorTypeGql,
    pub delegated_user_id: Option<Uuid>,
    pub token_name: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    #[graphql(default)]
    pub allowed_tools: Vec<String>,
    #[graphql(default)]
    pub denied_tools: Vec<String>,
    #[graphql(default)]
    pub granted_permissions: Vec<String>,
    #[graphql(default)]
    pub granted_scopes: Vec<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct RotateMcpTokenInput {
    pub token_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoke_existing_tokens: Option<bool>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateMcpPolicyInput {
    #[graphql(default)]
    pub allowed_tools: Vec<String>,
    #[graphql(default)]
    pub denied_tools: Vec<String>,
    #[graphql(default)]
    pub granted_permissions: Vec<String>,
    #[graphql(default)]
    pub granted_scopes: Vec<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct StageMcpModuleScaffoldDraftInput {
    pub client_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub description: String,
    #[graphql(default)]
    pub dependencies: Vec<String>,
    pub with_graphql: Option<bool>,
    pub with_rest: Option<bool>,
}

#[derive(Debug, Clone, InputObject)]
pub struct ApplyMcpModuleScaffoldDraftInput {
    pub workspace_root: String,
    pub confirm: bool,
}

pub fn parse_metadata(metadata: Option<String>) -> async_graphql::Result<serde_json::Value> {
    metadata
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                async_graphql::Error::new(format!("Invalid metadata JSON: {error}"))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or_else(|| serde_json::json!({})))
}

fn serialize_access_context(
    value: Option<&McpAccessContext>,
) -> async_graphql::Result<Option<String>> {
    value
        .map(|context| {
            serde_json::to_string(context).map_err(|error| {
                async_graphql::Error::new(format!("Failed to serialize access context: {error}"))
            })
        })
        .transpose()
}

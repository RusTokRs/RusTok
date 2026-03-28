use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::policy::ChannelResolutionRuleDefinition;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelInput {
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelTargetInput {
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelTargetInput {
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindChannelModuleInput {
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindChannelOauthAppInput {
    pub oauth_app_id: Uuid,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResolutionPolicySetInput {
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResolutionRuleInput {
    pub priority: i32,
    pub is_active: bool,
    pub definition: ChannelResolutionRuleDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub status: String,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTargetResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelModuleBindingResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOauthAppResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub oauth_app_id: Uuid,
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionPolicySetResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub schema_version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionRuleResponse {
    pub id: Uuid,
    pub policy_set_id: Uuid,
    pub priority: i32,
    pub is_active: bool,
    pub action_channel_id: Uuid,
    pub definition: ChannelResolutionRuleDefinition,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionPolicySetDetailResponse {
    pub policy_set: ChannelResolutionPolicySetResponse,
    pub rules: Vec<ChannelResolutionRuleResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDetailResponse {
    pub channel: ChannelResponse,
    pub targets: Vec<ChannelTargetResponse>,
    pub module_bindings: Vec<ChannelModuleBindingResponse>,
    pub oauth_apps: Vec<ChannelOauthAppResponse>,
}

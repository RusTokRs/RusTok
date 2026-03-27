use rustok_api::context::ChannelResolutionSource;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelAdminBootstrap {
    pub current_channel: Option<ResolvedChannelContext>,
    pub channels: Vec<ChannelDetail>,
    pub available_modules: Vec<AvailableModuleItem>,
    pub oauth_apps: Vec<AvailableOauthAppItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResolvedChannelContext {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub status: String,
    pub target_type: Option<String>,
    pub target_value: Option<String>,
    pub settings: Value,
    pub resolution_source: ChannelResolutionSource,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelDetail {
    pub channel: ChannelRecord,
    pub targets: Vec<ChannelTargetRecord>,
    pub module_bindings: Vec<ChannelModuleBindingRecord>,
    pub oauth_apps: Vec<ChannelOauthAppRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelRecord {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub status: String,
    pub settings: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelTargetRecord {
    pub id: String,
    pub channel_id: String,
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelModuleBindingRecord {
    pub id: String,
    pub channel_id: String,
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelOauthAppRecord {
    pub id: String,
    pub channel_id: String,
    pub oauth_app_id: String,
    pub role: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AvailableModuleItem {
    pub slug: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AvailableOauthAppItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub app_type: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateChannelPayload {
    pub tenant_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateChannelTargetPayload {
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindChannelModulePayload {
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindChannelOauthAppPayload {
    pub oauth_app_id: String,
    pub role: Option<String>,
}

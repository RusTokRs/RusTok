mod native_server_adapter;
mod rest_adapter;

use leptos::prelude::ServerFnError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap,
    ChannelModuleBindingRecord, ChannelOauthAppRecord, ChannelRecord, ChannelTargetRecord,
    CreateChannelPayload, CreateChannelTargetPayload, CreateResolutionPolicySetPayload,
    CreateResolutionRulePayload, ReorderResolutionRulesPayload, UpdateResolutionRulePayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Rest(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rest(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ChannelAdminBootstrap, ApiError> {
    match native_server_adapter::channel_bootstrap_native().await {
        Ok(payload) => Ok(payload),
        Err(_) => rest_adapter::get_json("/api/channels/bootstrap", token, tenant_slug).await,
    }
}

pub async fn create_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    payload: &CreateChannelPayload,
) -> Result<ChannelRecord, ApiError> {
    match native_server_adapter::channel_create_native(payload.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => rest_adapter::post_json("/api/channels/", payload, token, tenant_slug).await,
    }
}

pub async fn make_default_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
) -> Result<ChannelRecord, ApiError> {
    match native_server_adapter::channel_set_default_native(channel_id.to_string()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/default"),
                &serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn create_resolution_policy_set(
    token: Option<String>,
    tenant_slug: Option<String>,
    payload: &CreateResolutionPolicySetPayload,
) -> Result<crate::model::ChannelResolutionPolicySetRecord, ApiError> {
    match native_server_adapter::channel_create_resolution_policy_set_native(payload.clone()).await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json("/api/channels/policies", payload, token, tenant_slug).await
        }
    }
}

pub async fn activate_resolution_policy_set(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
) -> Result<crate::model::ChannelResolutionPolicySetRecord, ApiError> {
    match native_server_adapter::channel_activate_resolution_policy_set_native(
        policy_set_id.to_string(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/activate"),
                &serde_json::json!({}),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn create_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    payload: &CreateResolutionRulePayload,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    match native_server_adapter::channel_create_resolution_rule_native(
        policy_set_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/rules"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn update_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    rule_id: &str,
    payload: &UpdateResolutionRulePayload,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    match native_server_adapter::channel_update_resolution_rule_native(
        policy_set_id.to_string(),
        rule_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::patch_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/{rule_id}"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn reorder_resolution_rules(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    payload: &ReorderResolutionRulesPayload,
) -> Result<Vec<crate::model::ChannelResolutionRuleRecord>, ApiError> {
    match native_server_adapter::channel_reorder_resolution_rules_native(
        policy_set_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/reorder"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn delete_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    rule_id: &str,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    match native_server_adapter::channel_delete_resolution_rule_native(
        policy_set_id.to_string(),
        rule_id.to_string(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::delete_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/{rule_id}"),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn create_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    match native_server_adapter::channel_create_target_native(
        channel_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/targets"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn update_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    match native_server_adapter::channel_update_target_native(
        channel_id.to_string(),
        target_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::patch_json(
                &format!("/api/channels/{channel_id}/targets/{target_id}"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn bind_module(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelModulePayload,
) -> Result<ChannelModuleBindingRecord, ApiError> {
    match native_server_adapter::channel_bind_module_native(channel_id.to_string(), payload.clone())
        .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/modules"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn bind_oauth_app(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelOauthAppPayload,
) -> Result<ChannelOauthAppRecord, ApiError> {
    match native_server_adapter::channel_bind_oauth_app_native(
        channel_id.to_string(),
        payload.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/oauth-apps"),
                payload,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn delete_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
) -> Result<ChannelTargetRecord, ApiError> {
    match native_server_adapter::channel_delete_target_native(
        channel_id.to_string(),
        target_id.to_string(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/targets/{target_id}"),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn delete_module_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelModuleBindingRecord, ApiError> {
    match native_server_adapter::channel_delete_module_binding_native(
        channel_id.to_string(),
        binding_id.to_string(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/modules/{binding_id}"),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn delete_oauth_app_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelOauthAppRecord, ApiError> {
    match native_server_adapter::channel_delete_oauth_app_binding_native(
        channel_id.to_string(),
        binding_id.to_string(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/oauth-apps/{binding_id}"),
                token,
                tenant_slug,
            )
            .await
        }
    }
}

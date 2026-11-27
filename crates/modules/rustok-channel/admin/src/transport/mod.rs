//! Channel admin transport policy.
//!
//! The admin surface has exactly one transport per build and never switches at
//! runtime:
//!
//! * a server-side render (including hydration) calls the module's server
//!   functions, which run in-process with the session the request already carries;
//! * a browser build calls the REST API of the origin that served it.
//!
//! The previous implementation called the server function first and repeated the
//! *same* call over REST whenever it returned any error. That had three defects:
//!
//! * a rejected mutation (`400`/`403`/`404`) was executed a second time, so
//!   non-idempotent channel mutations could be applied twice;
//! * a server-side render fell through to `RUSTOK_API_URL` (defaulting to
//!   `http://localhost:5150`) and attached a bearer token that a cookie-backed
//!   admin session does not have;
//! * transport failures were indistinguishable from domain failures, because the
//!   first error was discarded.
//!
//! Selecting one path fixes all three: a transport failure is reported as-is and
//! a mutation is never replayed.

mod native_server_adapter;
mod rest_adapter;

use leptos::prelude::ServerFnError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::future::Future;

use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap,
    ChannelModuleBindingRecord, ChannelOauthAppRecord, ChannelRecord, ChannelTargetRecord,
    CreateChannelPayload, CreateChannelTargetPayload, CreateResolutionPolicySetPayload,
    CreateResolutionRulePayload, ReorderResolutionRulesPayload, UpdateChannelPayload,
    UpdateResolutionRulePayload,
};

/// Transport that a build can actually use for admin calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportPath {
    /// Leptos server function; runs in-process on the server.
    NativeServer,
    /// Same-origin REST endpoint of the host that served the admin bundle.
    Rest,
}

impl TransportPath {
    fn as_str(self) -> &'static str {
        match self {
            Self::NativeServer => "native_server",
            Self::Rest => "rest",
        }
    }
}

/// Selects exactly one transport for the current build.
///
/// A hydrated render still executes on the server, so `hydrate` builds keep the
/// in-process server function instead of issuing an HTTP request to themselves.
fn selected_transport_path() -> TransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        TransportPath::Rest
    } else {
        TransportPath::NativeServer
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Executes one admin call over the single transport this build selected.
///
/// There is deliberately no cross-transport fallback: a failed mutation must not be
/// replayed on another path. `surface` is only used for diagnostics so a failed
/// admin action can be traced to the call that produced it.
async fn dispatch<T, N, NFut, R, RFut>(
    surface: &'static str,
    native: N,
    rest: R,
) -> Result<T, ApiError>
where
    N: FnOnce() -> NFut,
    NFut: Future<Output = Result<T, ServerFnError>>,
    R: FnOnce() -> RFut,
    RFut: Future<Output = Result<T, ApiError>>,
{
    let path = selected_transport_path();
    let result = match path {
        TransportPath::NativeServer => native().await.map_err(ApiError::from),
        TransportPath::Rest => rest().await,
    };

    result.map_err(|error| {
        tracing::debug!(
            surface,
            transport = path.as_str(),
            error = %error,
            "channel admin transport call failed"
        );
        error
    })
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ChannelAdminBootstrap, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.bootstrap",
        || native_server_adapter::channel_bootstrap_native(),
        || rest_adapter::get_json("/api/channels/bootstrap", rest_token, rest_tenant),
    )
    .await
}

pub async fn create_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    payload: &CreateChannelPayload,
) -> Result<ChannelRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.create_channel",
        || native_server_adapter::channel_create_native(payload.clone()),
        || rest_adapter::post_json("/api/channels/", payload, rest_token, rest_tenant),
    )
    .await
}

/// Renames a channel or replaces its settings. The slug is immutable.
pub async fn update_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &UpdateChannelPayload,
) -> Result<ChannelRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.update_channel",
        || native_server_adapter::channel_update_native(channel_id.to_string(), payload.clone()),
        || {
            rest_adapter::patch_json(
                &format!("/api/channels/{channel_id}"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

/// Activates or deactivates a channel.
pub async fn set_channel_active(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    is_active: bool,
) -> Result<ChannelRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.set_channel_active",
        || native_server_adapter::channel_set_active_native(channel_id.to_string(), is_active),
        || {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/active"),
                &serde_json::json!({ "is_active": is_active }),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

/// Deletes a channel together with everything that referenced it.
pub async fn delete_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
) -> Result<ChannelRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.delete_channel",
        || native_server_adapter::channel_delete_native(channel_id.to_string()),
        || {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}"),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn make_default_channel(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
) -> Result<ChannelRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.set_default_channel",
        || native_server_adapter::channel_set_default_native(channel_id.to_string()),
        || {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/default"),
                &serde_json::json!({}),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn create_resolution_policy_set(
    token: Option<String>,
    tenant_slug: Option<String>,
    payload: &CreateResolutionPolicySetPayload,
) -> Result<crate::model::ChannelResolutionPolicySetRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.create_resolution_policy_set",
        || native_server_adapter::channel_create_resolution_policy_set_native(payload.clone()),
        || rest_adapter::post_json("/api/channels/policies", payload, rest_token, rest_tenant),
    )
    .await
}

pub async fn activate_resolution_policy_set(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
) -> Result<crate::model::ChannelResolutionPolicySetRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.activate_resolution_policy_set",
        || {
            native_server_adapter::channel_activate_resolution_policy_set_native(
                policy_set_id.to_string(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/activate"),
                &serde_json::json!({}),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn create_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    payload: &CreateResolutionRulePayload,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.create_resolution_rule",
        || {
            native_server_adapter::channel_create_resolution_rule_native(
                policy_set_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/rules"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn update_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    rule_id: &str,
    payload: &UpdateResolutionRulePayload,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.update_resolution_rule",
        || {
            native_server_adapter::channel_update_resolution_rule_native(
                policy_set_id.to_string(),
                rule_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::patch_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/{rule_id}"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn reorder_resolution_rules(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    payload: &ReorderResolutionRulesPayload,
) -> Result<Vec<crate::model::ChannelResolutionRuleRecord>, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.reorder_resolution_rules",
        || {
            native_server_adapter::channel_reorder_resolution_rules_native(
                policy_set_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/reorder"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn delete_resolution_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    policy_set_id: &str,
    rule_id: &str,
) -> Result<crate::model::ChannelResolutionRuleRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.delete_resolution_rule",
        || {
            native_server_adapter::channel_delete_resolution_rule_native(
                policy_set_id.to_string(),
                rule_id.to_string(),
            )
        },
        || {
            rest_adapter::delete_json(
                &format!("/api/channels/policies/{policy_set_id}/rules/{rule_id}"),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn create_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.create_target",
        || {
            native_server_adapter::channel_create_target_native(
                channel_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/targets"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn update_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
    payload: &CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.update_target",
        || {
            native_server_adapter::channel_update_target_native(
                channel_id.to_string(),
                target_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::patch_json(
                &format!("/api/channels/{channel_id}/targets/{target_id}"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn delete_target(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    target_id: &str,
) -> Result<ChannelTargetRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.delete_target",
        || {
            native_server_adapter::channel_delete_target_native(
                channel_id.to_string(),
                target_id.to_string(),
            )
        },
        || {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/targets/{target_id}"),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn bind_module(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelModulePayload,
) -> Result<ChannelModuleBindingRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.bind_module",
        || {
            native_server_adapter::channel_bind_module_native(
                channel_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/modules"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn delete_module_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelModuleBindingRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.delete_module_binding",
        || {
            native_server_adapter::channel_delete_module_binding_native(
                channel_id.to_string(),
                binding_id.to_string(),
            )
        },
        || {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/modules/{binding_id}"),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn bind_oauth_app(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    payload: &BindChannelOauthAppPayload,
) -> Result<ChannelOauthAppRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.bind_oauth_app",
        || {
            native_server_adapter::channel_bind_oauth_app_native(
                channel_id.to_string(),
                payload.clone(),
            )
        },
        || {
            rest_adapter::post_json(
                &format!("/api/channels/{channel_id}/oauth-apps"),
                payload,
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

pub async fn delete_oauth_app_binding(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: &str,
    binding_id: &str,
) -> Result<ChannelOauthAppRecord, ApiError> {
    let rest_token = token.clone();
    let rest_tenant = tenant_slug.clone();
    dispatch(
        "channel.admin.delete_oauth_app_binding",
        || {
            native_server_adapter::channel_delete_oauth_app_binding_native(
                channel_id.to_string(),
                binding_id.to_string(),
            )
        },
        || {
            rest_adapter::delete_json(
                &format!("/api/channels/{channel_id}/oauth-apps/{binding_id}"),
                rest_token,
                rest_tenant,
            )
        },
    )
    .await
}

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    AiAdminBootstrap, AiChatRunPayload, AiChatSessionDetailPayload, AiProviderProfilePayload,
    AiProviderTestResultPayload, AiSendMessageResultPayload, AiTaskProfilePayload,
    AiToolProfilePayload,
};
#[cfg(feature = "ssr")]
use crate::model::{
    AiApprovalRequestPayload, AiChatMessagePayload, AiChatSessionSummaryPayload, AiToolCallPayload,
    AiToolTracePayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
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

pub async fn fetch_bootstrap() -> Result<AiAdminBootstrap, ApiError> {
    ai_bootstrap_native().await.map_err(Into::into)
}

pub async fn fetch_session(
    session_id: String,
) -> Result<Option<AiChatSessionDetailPayload>, ApiError> {
    ai_session_native(session_id).await.map_err(Into::into)
}

pub async fn create_provider(
    slug: String,
    display_name: String,
    provider_kind: String,
    base_url: String,
    model: String,
    api_key_secret: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    capabilities: Vec<String>,
    allowed_task_profiles: Vec<String>,
    denied_task_profiles: Vec<String>,
    restricted_role_slugs: Vec<String>,
) -> Result<AiProviderProfilePayload, ApiError> {
    ai_create_provider_native(
        slug,
        display_name,
        provider_kind,
        base_url,
        model,
        api_key_secret,
        temperature,
        max_tokens,
        capabilities,
        allowed_task_profiles,
        denied_task_profiles,
        restricted_role_slugs,
    )
    .await
    .map_err(Into::into)
}

pub async fn update_provider(
    id: String,
    display_name: String,
    base_url: String,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    capabilities: Vec<String>,
    allowed_task_profiles: Vec<String>,
    denied_task_profiles: Vec<String>,
    restricted_role_slugs: Vec<String>,
    is_active: bool,
) -> Result<AiProviderProfilePayload, ApiError> {
    ai_update_provider_native(
        id,
        display_name,
        base_url,
        model,
        temperature,
        max_tokens,
        capabilities,
        allowed_task_profiles,
        denied_task_profiles,
        restricted_role_slugs,
        is_active,
    )
    .await
    .map_err(Into::into)
}

pub async fn test_provider(id: String) -> Result<AiProviderTestResultPayload, ApiError> {
    ai_test_provider_native(id).await.map_err(Into::into)
}

pub async fn deactivate_provider(id: String) -> Result<AiProviderProfilePayload, ApiError> {
    ai_deactivate_provider_native(id).await.map_err(Into::into)
}

pub async fn create_tool_profile(
    slug: String,
    display_name: String,
    description: Option<String>,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    sensitive_tools: Vec<String>,
) -> Result<AiToolProfilePayload, ApiError> {
    ai_create_tool_profile_native(
        slug,
        display_name,
        description,
        allowed_tools,
        denied_tools,
        sensitive_tools,
    )
    .await
    .map_err(Into::into)
}

pub async fn update_tool_profile(
    id: String,
    display_name: String,
    description: Option<String>,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    sensitive_tools: Vec<String>,
    is_active: bool,
) -> Result<AiToolProfilePayload, ApiError> {
    ai_update_tool_profile_native(
        id,
        display_name,
        description,
        allowed_tools,
        denied_tools,
        sensitive_tools,
        is_active,
    )
    .await
    .map_err(Into::into)
}

pub async fn create_task_profile(
    slug: String,
    display_name: String,
    description: Option<String>,
    target_capability: String,
    system_prompt: Option<String>,
    allowed_provider_profile_ids: Vec<String>,
    preferred_provider_profile_ids: Vec<String>,
    tool_profile_id: Option<String>,
    default_execution_mode: String,
) -> Result<AiTaskProfilePayload, ApiError> {
    ai_create_task_profile_native(
        slug,
        display_name,
        description,
        target_capability,
        system_prompt,
        allowed_provider_profile_ids,
        preferred_provider_profile_ids,
        tool_profile_id,
        default_execution_mode,
    )
    .await
    .map_err(Into::into)
}

pub async fn update_task_profile(
    id: String,
    display_name: String,
    description: Option<String>,
    target_capability: String,
    system_prompt: Option<String>,
    allowed_provider_profile_ids: Vec<String>,
    preferred_provider_profile_ids: Vec<String>,
    tool_profile_id: Option<String>,
    default_execution_mode: String,
    is_active: bool,
) -> Result<AiTaskProfilePayload, ApiError> {
    ai_update_task_profile_native(
        id,
        display_name,
        description,
        target_capability,
        system_prompt,
        allowed_provider_profile_ids,
        preferred_provider_profile_ids,
        tool_profile_id,
        default_execution_mode,
        is_active,
    )
    .await
    .map_err(Into::into)
}

pub async fn start_session(
    title: String,
    provider_profile_id: Option<String>,
    task_profile_id: Option<String>,
    tool_profile_id: Option<String>,
    initial_message: Option<String>,
) -> Result<AiSendMessageResultPayload, ApiError> {
    ai_start_session_native(
        title,
        provider_profile_id,
        task_profile_id,
        tool_profile_id,
        initial_message,
    )
    .await
    .map_err(Into::into)
}

pub async fn send_message(
    session_id: String,
    content: String,
) -> Result<AiSendMessageResultPayload, ApiError> {
    ai_send_message_native(session_id, content)
        .await
        .map_err(Into::into)
}

pub async fn resume_approval(
    approval_id: String,
    approved: bool,
    reason: Option<String>,
) -> Result<AiSendMessageResultPayload, ApiError> {
    ai_resume_approval_native(approval_id, approved, reason)
        .await
        .map_err(Into::into)
}

#[allow(dead_code)]
pub async fn cancel_run(run_id: String) -> Result<AiChatRunPayload, ApiError> {
    ai_cancel_run_native(run_id).await.map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "ai/bootstrap")]
async fn ai_bootstrap_native() -> Result<AiAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_overview_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        Ok(AiAdminBootstrap {
            providers: rustok_ai::AiManagementService::list_provider_profiles(
                &app_ctx.db,
                auth.tenant_id,
            )
            .await
            .map_err(server_error)?
            .into_iter()
            .map(map_provider)
            .collect(),
            task_profiles: rustok_ai::AiManagementService::list_task_profiles(
                &app_ctx.db,
                auth.tenant_id,
            )
            .await
            .map_err(server_error)?
            .into_iter()
            .map(map_task_profile)
            .collect(),
            tool_profiles: rustok_ai::AiManagementService::list_tool_profiles(
                &app_ctx.db,
                auth.tenant_id,
            )
            .await
            .map_err(server_error)?
            .into_iter()
            .map(map_tool_profile)
            .collect(),
            sessions: rustok_ai::AiManagementService::list_chat_sessions(
                &app_ctx.db,
                auth.tenant_id,
            )
            .await
            .map_err(server_error)?
            .into_iter()
            .map(map_session_summary)
            .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-ai-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/session")]
async fn ai_session_native(
    session_id: String,
) -> Result<Option<AiChatSessionDetailPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_session_read_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let session_id = parse_uuid(&session_id, "session_id")?;
        let detail = rustok_ai::AiManagementService::chat_session_detail(
            &app_ctx.db,
            auth.tenant_id,
            session_id,
        )
        .await
        .map_err(server_error)?;
        Ok(detail.map(map_session_detail))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = session_id;
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/create-provider")]
async fn ai_create_provider_native(
    slug: String,
    display_name: String,
    provider_kind: String,
    base_url: String,
    model: String,
    api_key_secret: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    capabilities: Vec<String>,
    allowed_task_profiles: Vec<String>,
    denied_task_profiles: Vec<String>,
    restricted_role_slugs: Vec<String>,
) -> Result<AiProviderProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_provider_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::create_provider_profile(
            &app_ctx.db,
            &operator(&auth),
            rustok_ai::CreateAiProviderProfileInput {
                slug,
                display_name,
                provider_kind: parse_provider_kind(&provider_kind)?,
                base_url,
                model,
                api_key_secret,
                temperature,
                max_tokens,
                capabilities: parse_capabilities(capabilities)?,
                usage_policy: rustok_ai::ProviderUsagePolicy {
                    allowed_task_profiles,
                    denied_task_profiles,
                    restricted_role_slugs,
                },
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_provider(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            slug,
            display_name,
            provider_kind,
            base_url,
            model,
            api_key_secret,
            temperature,
            max_tokens,
            capabilities,
            allowed_task_profiles,
            denied_task_profiles,
            restricted_role_slugs,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/test-provider")]
async fn ai_test_provider_native(id: String) -> Result<AiProviderTestResultPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_provider_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::test_provider_profile(
            &app_ctx.db,
            auth.tenant_id,
            parse_uuid(&id, "id")?,
        )
        .await
        .map_err(server_error)?;
        Ok(AiProviderTestResultPayload {
            ok: item.ok,
            provider: item.provider,
            model: item.model,
            latency_ms: item.latency_ms,
            message: item.message,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/update-provider")]
async fn ai_update_provider_native(
    id: String,
    display_name: String,
    base_url: String,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    capabilities: Vec<String>,
    allowed_task_profiles: Vec<String>,
    denied_task_profiles: Vec<String>,
    restricted_role_slugs: Vec<String>,
    is_active: bool,
) -> Result<AiProviderProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_provider_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::update_provider_profile(
            &app_ctx.db,
            &operator(&auth),
            parse_uuid(&id, "id")?,
            rustok_ai::UpdateAiProviderProfileInput {
                display_name,
                base_url,
                model,
                temperature,
                max_tokens,
                capabilities: parse_capabilities(capabilities)?,
                usage_policy: rustok_ai::ProviderUsagePolicy {
                    allowed_task_profiles,
                    denied_task_profiles,
                    restricted_role_slugs,
                },
                metadata: serde_json::json!({}),
                is_active,
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_provider(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            id,
            display_name,
            base_url,
            model,
            temperature,
            max_tokens,
            capabilities,
            allowed_task_profiles,
            denied_task_profiles,
            restricted_role_slugs,
            is_active,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/deactivate-provider")]
async fn ai_deactivate_provider_native(
    id: String,
) -> Result<AiProviderProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_provider_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::deactivate_provider_profile(
            &app_ctx.db,
            &operator(&auth),
            parse_uuid(&id, "id")?,
        )
        .await
        .map_err(server_error)?;
        Ok(map_provider(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/create-tool-profile")]
async fn ai_create_tool_profile_native(
    slug: String,
    display_name: String,
    description: Option<String>,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    sensitive_tools: Vec<String>,
) -> Result<AiToolProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_tool_profile_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::create_tool_profile(
            &app_ctx.db,
            &operator(&auth),
            rustok_ai::CreateAiToolProfileInput {
                slug,
                display_name,
                description,
                allowed_tools,
                denied_tools,
                sensitive_tools,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_tool_profile(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            slug,
            display_name,
            description,
            allowed_tools,
            denied_tools,
            sensitive_tools,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/update-tool-profile")]
async fn ai_update_tool_profile_native(
    id: String,
    display_name: String,
    description: Option<String>,
    allowed_tools: Vec<String>,
    denied_tools: Vec<String>,
    sensitive_tools: Vec<String>,
    is_active: bool,
) -> Result<AiToolProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_tool_profile_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::update_tool_profile(
            &app_ctx.db,
            &operator(&auth),
            parse_uuid(&id, "id")?,
            rustok_ai::UpdateAiToolProfileInput {
                display_name,
                description,
                allowed_tools,
                denied_tools,
                sensitive_tools,
                metadata: serde_json::json!({}),
                is_active,
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_tool_profile(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            id,
            display_name,
            description,
            allowed_tools,
            denied_tools,
            sensitive_tools,
            is_active,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/create-task-profile")]
async fn ai_create_task_profile_native(
    slug: String,
    display_name: String,
    description: Option<String>,
    target_capability: String,
    system_prompt: Option<String>,
    allowed_provider_profile_ids: Vec<String>,
    preferred_provider_profile_ids: Vec<String>,
    tool_profile_id: Option<String>,
    default_execution_mode: String,
) -> Result<AiTaskProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_tool_profile_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::create_task_profile(
            &app_ctx.db,
            &operator(&auth),
            rustok_ai::CreateAiTaskProfileInput {
                slug,
                display_name,
                description,
                target_capability: parse_capability(&target_capability)?,
                system_prompt,
                allowed_provider_profile_ids: parse_uuid_list(
                    allowed_provider_profile_ids,
                    "allowed_provider_profile_ids",
                )?,
                preferred_provider_profile_ids: parse_uuid_list(
                    preferred_provider_profile_ids,
                    "preferred_provider_profile_ids",
                )?,
                fallback_strategy: "ordered".to_string(),
                tool_profile_id: parse_optional_uuid(tool_profile_id, "tool_profile_id")?,
                approval_policy: serde_json::json!({}),
                default_execution_mode: parse_execution_mode(&default_execution_mode)?,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_task_profile(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            slug,
            display_name,
            description,
            target_capability,
            system_prompt,
            allowed_provider_profile_ids,
            preferred_provider_profile_ids,
            tool_profile_id,
            default_execution_mode,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/update-task-profile")]
async fn ai_update_task_profile_native(
    id: String,
    display_name: String,
    description: Option<String>,
    target_capability: String,
    system_prompt: Option<String>,
    allowed_provider_profile_ids: Vec<String>,
    preferred_provider_profile_ids: Vec<String>,
    tool_profile_id: Option<String>,
    default_execution_mode: String,
    is_active: bool,
) -> Result<AiTaskProfilePayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_tool_profile_manage_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::update_task_profile(
            &app_ctx.db,
            &operator(&auth),
            parse_uuid(&id, "id")?,
            rustok_ai::UpdateAiTaskProfileInput {
                display_name,
                description,
                target_capability: parse_capability(&target_capability)?,
                system_prompt,
                allowed_provider_profile_ids: parse_uuid_list(
                    allowed_provider_profile_ids,
                    "allowed_provider_profile_ids",
                )?,
                preferred_provider_profile_ids: parse_uuid_list(
                    preferred_provider_profile_ids,
                    "preferred_provider_profile_ids",
                )?,
                fallback_strategy: "ordered".to_string(),
                tool_profile_id: parse_optional_uuid(tool_profile_id, "tool_profile_id")?,
                approval_policy: serde_json::json!({}),
                default_execution_mode: parse_execution_mode(&default_execution_mode)?,
                metadata: serde_json::json!({}),
                is_active,
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_task_profile(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            id,
            display_name,
            description,
            target_capability,
            system_prompt,
            allowed_provider_profile_ids,
            preferred_provider_profile_ids,
            tool_profile_id,
            default_execution_mode,
            is_active,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/start-session")]
async fn ai_start_session_native(
    title: String,
    provider_profile_id: Option<String>,
    task_profile_id: Option<String>,
    tool_profile_id: Option<String>,
    initial_message: Option<String>,
) -> Result<AiSendMessageResultPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_session_run_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::start_chat_session(
            &app_ctx,
            &operator(&auth),
            rustok_ai::StartAiChatSessionInput {
                title,
                provider_profile_id: parse_optional_uuid(
                    provider_profile_id,
                    "provider_profile_id",
                )?,
                task_profile_id: parse_optional_uuid(task_profile_id, "task_profile_id")?,
                tool_profile_id: parse_optional_uuid(tool_profile_id, "tool_profile_id")?,
                execution_mode: None,
                override_config: rustok_ai::ExecutionOverride::default(),
                initial_message,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .map_err(server_error)?;
        Ok(map_send_result(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            title,
            provider_profile_id,
            task_profile_id,
            tool_profile_id,
            initial_message,
        );
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/send-message")]
async fn ai_send_message_native(
    session_id: String,
    content: String,
) -> Result<AiSendMessageResultPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_session_run_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::send_chat_message(
            &app_ctx,
            &operator(&auth),
            parse_uuid(&session_id, "session_id")?,
            rustok_ai::SendAiChatMessageInput { content },
        )
        .await
        .map_err(server_error)?;
        Ok(map_send_result(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (session_id, content);
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/resume-approval")]
async fn ai_resume_approval_native(
    approval_id: String,
    approved: bool,
    reason: Option<String>,
) -> Result<AiSendMessageResultPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_approval_resolve_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::resume_approval(
            &app_ctx,
            &operator(&auth),
            parse_uuid(&approval_id, "approval_id")?,
            rustok_ai::ResumeAiApprovalInput { approved, reason },
        )
        .await
        .map_err(server_error)?;
        Ok(map_send_result(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (approval_id, approved, reason);
        Err(ServerFnError::new("SSR only"))
    }
}

#[server(prefix = "/api/fn", endpoint = "ai/cancel-run")]
async fn ai_cancel_run_native(run_id: String) -> Result<AiChatRunPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_ai_run_cancel_permission(&auth.permissions)?;
        let app_ctx = leptos::prelude::expect_context::<loco_rs::app::AppContext>();
        let item = rustok_ai::AiManagementService::cancel_run(
            &app_ctx.db,
            &operator(&auth),
            parse_uuid(&run_id, "run_id")?,
        )
        .await
        .map_err(server_error)?;
        Ok(map_run(item))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = run_id;
        Err(ServerFnError::new("SSR only"))
    }
}

#[cfg(feature = "ssr")]
fn ensure_ai_provider_manage_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_PROVIDERS_MANAGE,
    ) {
        return Err(ServerFnError::new("ai:providers:manage required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_tool_profile_manage_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_TASK_PROFILES_MANAGE,
    ) {
        return Err(ServerFnError::new("ai:task_profiles:manage required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_session_read_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_SESSIONS_READ,
    ) {
        return Err(ServerFnError::new("ai:sessions:read required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_session_run_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_core::Permission::AI_SESSIONS_RUN)
    {
        return Err(ServerFnError::new("ai:sessions:run required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_approval_resolve_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_APPROVALS_RESOLVE,
    ) {
        return Err(ServerFnError::new("ai:approvals:resolve required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_run_cancel_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_core::Permission::AI_RUNS_CANCEL)
    {
        return Err(ServerFnError::new("ai:runs:cancel required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_ai_overview_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_PROVIDERS_READ,
    ) || rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_TASK_PROFILES_READ,
    ) || rustok_api::has_effective_permission(
        permissions,
        &rustok_core::Permission::AI_SESSIONS_READ,
    ) {
        Ok(())
    } else {
        Err(ServerFnError::new("AI read permissions required"))
    }
}

#[cfg(feature = "ssr")]
fn operator(auth: &rustok_api::AuthContext) -> rustok_ai::AiOperatorContext {
    rustok_ai::AiOperatorContext {
        tenant_id: auth.tenant_id,
        user_id: auth.user_id,
        permissions: auth.permissions.clone(),
    }
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn server_error(error: impl std::fmt::Display) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

#[cfg(feature = "ssr")]
fn map_provider(value: rustok_ai::AiProviderProfileRecord) -> AiProviderProfilePayload {
    AiProviderProfilePayload {
        id: value.id.to_string(),
        slug: value.slug,
        display_name: value.display_name,
        provider_kind: value.provider_kind.slug().to_string(),
        base_url: value.base_url,
        model: value.model,
        temperature: value.temperature,
        max_tokens: value.max_tokens,
        is_active: value.is_active,
        has_secret: value.has_secret,
        capabilities: value
            .capabilities
            .into_iter()
            .map(|item| item.slug().to_string())
            .collect(),
        allowed_task_profiles: value.usage_policy.allowed_task_profiles,
        denied_task_profiles: value.usage_policy.denied_task_profiles,
        restricted_role_slugs: value.usage_policy.restricted_role_slugs,
        metadata: value.metadata.to_string(),
    }
}

#[cfg(feature = "ssr")]
fn map_tool_profile(value: rustok_ai::AiToolProfileRecord) -> AiToolProfilePayload {
    AiToolProfilePayload {
        id: value.id.to_string(),
        slug: value.slug,
        display_name: value.display_name,
        description: value.description,
        allowed_tools: value.allowed_tools,
        denied_tools: value.denied_tools,
        sensitive_tools: value.sensitive_tools,
        is_active: value.is_active,
        metadata: value.metadata.to_string(),
    }
}

#[cfg(feature = "ssr")]
fn map_task_profile(value: rustok_ai::AiTaskProfileRecord) -> AiTaskProfilePayload {
    AiTaskProfilePayload {
        id: value.id.to_string(),
        slug: value.slug,
        display_name: value.display_name,
        description: value.description,
        target_capability: value.target_capability.slug().to_string(),
        system_prompt: value.system_prompt,
        allowed_provider_profile_ids: value
            .allowed_provider_profile_ids
            .into_iter()
            .map(|value| value.to_string())
            .collect(),
        preferred_provider_profile_ids: value
            .preferred_provider_profile_ids
            .into_iter()
            .map(|value| value.to_string())
            .collect(),
        fallback_strategy: value.fallback_strategy,
        tool_profile_id: value.tool_profile_id.map(|value| value.to_string()),
        default_execution_mode: value.default_execution_mode.slug().to_string(),
        is_active: value.is_active,
        metadata: value.metadata.to_string(),
    }
}

#[cfg(feature = "ssr")]
fn map_session_summary(value: rustok_ai::AiChatSessionSummary) -> AiChatSessionSummaryPayload {
    AiChatSessionSummaryPayload {
        id: value.id.to_string(),
        title: value.title,
        provider_profile_id: value.provider_profile_id.to_string(),
        task_profile_id: value
            .task_profile_id
            .map(|value: uuid::Uuid| value.to_string()),
        tool_profile_id: value
            .tool_profile_id
            .map(|value: uuid::Uuid| value.to_string()),
        execution_mode: value.execution_mode.slug().to_string(),
        status: value.status,
        latest_run_status: value.latest_run_status,
        pending_approvals: value.pending_approvals as i32,
    }
}

#[cfg(feature = "ssr")]
fn map_message(value: rustok_ai::AiChatMessageRecord) -> AiChatMessagePayload {
    AiChatMessagePayload {
        id: value.id.to_string(),
        session_id: value.session_id.to_string(),
        run_id: value.run_id.map(|value: uuid::Uuid| value.to_string()),
        role: format!("{:?}", value.role).to_lowercase(),
        content: value.content,
        name: value.name,
        tool_call_id: value.tool_call_id,
        tool_calls: value
            .tool_calls
            .into_iter()
            .map(|call| AiToolCallPayload {
                id: call.id,
                name: call.name,
                arguments_json: call.arguments.to_string(),
            })
            .collect(),
        metadata: value.metadata.to_string(),
        created_at: value.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_run(value: rustok_ai::AiChatRunRecord) -> AiChatRunPayload {
    AiChatRunPayload {
        id: value.id.to_string(),
        session_id: value.session_id.to_string(),
        provider_profile_id: value.provider_profile_id.to_string(),
        task_profile_id: value
            .task_profile_id
            .map(|value: uuid::Uuid| value.to_string()),
        tool_profile_id: value
            .tool_profile_id
            .map(|value: uuid::Uuid| value.to_string()),
        status: value.status,
        model: value.model,
        execution_mode: value.execution_mode.slug().to_string(),
        execution_path: value.execution_path.slug().to_string(),
        temperature: value.temperature,
        max_tokens: value.max_tokens,
        error_message: value.error_message,
        pending_approval_id: value
            .pending_approval_id
            .map(|value: uuid::Uuid| value.to_string()),
        decision_trace: serde_json::to_string(&value.decision_trace)
            .unwrap_or_else(|_| "{}".to_string()),
        metadata: value.metadata.to_string(),
        created_at: value.created_at.to_rfc3339(),
        started_at: value.started_at.to_rfc3339(),
        completed_at: value
            .completed_at
            .map(|value: chrono::DateTime<chrono::Utc>| value.to_rfc3339()),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_trace(value: rustok_ai::ToolTrace) -> AiToolTracePayload {
    AiToolTracePayload {
        tool_name: value.tool_name,
        input_payload: value.input_payload.to_string(),
        output_payload: value
            .output_payload
            .map(|value: serde_json::Value| value.to_string()),
        status: value.status,
        duration_ms: value.duration_ms,
        sensitive: value.sensitive,
        error_message: value.error_message,
        created_at: value.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_approval(value: rustok_ai::AiApprovalRequestRecord) -> AiApprovalRequestPayload {
    AiApprovalRequestPayload {
        id: value.id.to_string(),
        session_id: value.session_id.to_string(),
        run_id: value.run_id.to_string(),
        tool_name: value.tool_name,
        tool_call_id: value.tool_call_id,
        tool_input: value.tool_input.to_string(),
        reason: value.reason,
        status: value.status,
        resolved_by: value.resolved_by.map(|value: uuid::Uuid| value.to_string()),
        resolved_at: value
            .resolved_at
            .map(|value: chrono::DateTime<chrono::Utc>| value.to_rfc3339()),
        metadata: value.metadata.to_string(),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_session_detail(value: rustok_ai::AiChatSessionDetail) -> AiChatSessionDetailPayload {
    AiChatSessionDetailPayload {
        session: map_session_summary(value.session),
        provider_profile: map_provider(value.provider_profile),
        task_profile: value.task_profile.map(map_task_profile),
        tool_profile: value.tool_profile.map(map_tool_profile),
        messages: value.messages.into_iter().map(map_message).collect(),
        runs: value.runs.into_iter().map(map_run).collect(),
        tool_traces: value.tool_traces.into_iter().map(map_trace).collect(),
        approvals: value.approvals.into_iter().map(map_approval).collect(),
    }
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(Some(parse_uuid(&value, field_name)?)),
        _ => Ok(None),
    }
}

#[cfg(feature = "ssr")]
fn parse_uuid_list(
    values: Vec<String>,
    field_name: &str,
) -> Result<Vec<uuid::Uuid>, ServerFnError> {
    values
        .into_iter()
        .map(|value| parse_uuid(&value, field_name))
        .collect()
}

#[cfg(feature = "ssr")]
fn parse_capability(value: &str) -> Result<rustok_ai::ProviderCapability, ServerFnError> {
    match value.trim() {
        "structured_generation" => Ok(rustok_ai::ProviderCapability::StructuredGeneration),
        "image_generation" => Ok(rustok_ai::ProviderCapability::ImageGeneration),
        "multimodal_understanding" => Ok(rustok_ai::ProviderCapability::MultimodalUnderstanding),
        "code_generation" => Ok(rustok_ai::ProviderCapability::CodeGeneration),
        "alloy_assist" => Ok(rustok_ai::ProviderCapability::AlloyAssist),
        "text_generation" | "" => Ok(rustok_ai::ProviderCapability::TextGeneration),
        _ => Err(ServerFnError::new("Invalid target_capability")),
    }
}

#[cfg(feature = "ssr")]
fn parse_execution_mode(value: &str) -> Result<rustok_ai::ExecutionMode, ServerFnError> {
    match value.trim() {
        "direct" => Ok(rustok_ai::ExecutionMode::Direct),
        "mcp_tooling" => Ok(rustok_ai::ExecutionMode::McpTooling),
        "auto" | "" => Ok(rustok_ai::ExecutionMode::Auto),
        _ => Err(ServerFnError::new("Invalid execution_mode")),
    }
}

#[cfg(feature = "ssr")]
fn parse_provider_kind(value: &str) -> Result<rustok_ai::ProviderKind, ServerFnError> {
    match value.trim() {
        "anthropic" => Ok(rustok_ai::ProviderKind::Anthropic),
        "gemini" => Ok(rustok_ai::ProviderKind::Gemini),
        "openai_compatible" | "" => Ok(rustok_ai::ProviderKind::OpenAiCompatible),
        _ => Err(ServerFnError::new("Invalid provider_kind")),
    }
}

#[cfg(feature = "ssr")]
fn parse_capabilities(
    values: Vec<String>,
) -> Result<Vec<rustok_ai::ProviderCapability>, ServerFnError> {
    values
        .into_iter()
        .map(|value| parse_capability(&value))
        .collect()
}

#[cfg(feature = "ssr")]
fn map_send_result(value: rustok_ai::AiSendMessageResult) -> AiSendMessageResultPayload {
    AiSendMessageResultPayload {
        session: map_session_detail(value.session),
        run: map_run(value.run),
    }
}

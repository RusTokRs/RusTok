use std::collections::HashMap;
use uuid::Uuid;

use crate::entities::{
    ai_approval_requests, ai_chat_messages, ai_chat_runs, ai_chat_sessions, ai_provider_profiles,
    ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::model::{
    AiRunDecisionTrace, ChatMessage, ChatMessageRole, ProviderKind, ProviderUsagePolicy, TaskProfile, ToolTrace,
};
use crate::{AiError, AiResult};

use super::helpers::{
    capability_from_slug, capability_list, execution_mode_from_slug, json_err,
    provider_kind_from_slug, string_list, to_utc, uuid_list,
};
use super::types::{
    AiApprovalRequestRecord, AiChatMessageRecord, AiChatRunRecord, AiProviderProfileRecord,
    AiRecentRunRecord, AiTaskProfileRecord, AiToolProfileRecord,
};

pub fn map_provider_profile(model: ai_provider_profiles::Model) -> AiResult<AiProviderProfileRecord> {
    Ok(AiProviderProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        provider_kind: provider_kind_from_slug(&model.provider_kind)?,
        base_url: model.base_url,
        model: model.model,
        temperature: model.temperature,
        max_tokens: model.max_tokens,
        is_active: model.is_active,
        has_secret: model
            .api_key_secret
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        capabilities: capability_list(&model.capabilities)?,
        usage_policy: ProviderUsagePolicy {
            allowed_task_profiles: string_list(&model.allowed_task_profiles),
            denied_task_profiles: string_list(&model.denied_task_profiles),
            restricted_role_slugs: string_list(&model.restricted_role_slugs),
        },
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    })
}

pub fn map_task_profile(model: ai_task_profiles::Model) -> AiResult<AiTaskProfileRecord> {
    Ok(AiTaskProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        description: model.description,
        target_capability: capability_from_slug(&model.target_capability)?,
        system_prompt: model.system_prompt,
        allowed_provider_profile_ids: uuid_list(&model.allowed_provider_profile_ids),
        preferred_provider_profile_ids: uuid_list(&model.preferred_provider_profile_ids),
        fallback_strategy: model.fallback_strategy,
        tool_profile_id: model.tool_profile_id,
        approval_policy: model.approval_policy,
        default_execution_mode: execution_mode_from_slug(&model.default_execution_mode)?,
        is_active: model.is_active,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    })
}

pub fn task_profile_runtime(record: &AiTaskProfileRecord) -> TaskProfile {
    TaskProfile {
        id: record.id,
        slug: record.slug.clone(),
        display_name: record.display_name.clone(),
        description: record.description.clone(),
        target_capability: record.target_capability,
        system_prompt: record.system_prompt.clone(),
        allowed_provider_profile_ids: record.allowed_provider_profile_ids.clone(),
        preferred_provider_profile_ids: record.preferred_provider_profile_ids.clone(),
        fallback_strategy: record.fallback_strategy.clone(),
        tool_profile_id: record.tool_profile_id,
        approval_policy: record.approval_policy.clone(),
        default_execution_mode: record.default_execution_mode,
        is_active: record.is_active,
        metadata: record.metadata.clone(),
    }
}

pub fn map_tool_profile(model: ai_tool_profiles::Model) -> AiToolProfileRecord {
    AiToolProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        description: model.description,
        allowed_tools: string_list(&model.allowed_tools),
        denied_tools: string_list(&model.denied_tools),
        sensitive_tools: string_list(&model.sensitive_tools),
        is_active: model.is_active,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    }
}

pub fn map_message_record(model: ai_chat_messages::Model) -> AiResult<AiChatMessageRecord> {
    Ok(AiChatMessageRecord {
        id: model.id,
        session_id: model.session_id,
        run_id: model.run_id,
        role: map_role(&model.role)?,
        content: model.content,
        name: model.name,
        tool_call_id: model.tool_call_id,
        tool_calls: serde_json::from_value(model.tool_calls).map_err(json_err)?,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        created_by: model.created_by,
    })
}

pub fn map_run_record(model: ai_chat_runs::Model) -> AiResult<AiChatRunRecord> {
    Ok(AiChatRunRecord {
        id: model.id,
        session_id: model.session_id,
        provider_profile_id: model.provider_profile_id,
        task_profile_id: model.task_profile_id,
        tool_profile_id: model.tool_profile_id,
        status: model.status,
        model: model.model,
        execution_mode: execution_mode_from_slug(&model.execution_mode)?,
        execution_path: execution_mode_from_slug(&model.execution_path)?,
        requested_locale: model.requested_locale,
        resolved_locale: model.resolved_locale,
        temperature: model.temperature,
        max_tokens: model.max_tokens,
        error_message: model.error_message,
        pending_approval_id: model.pending_approval_id,
        decision_trace: serde_json::from_value(model.decision_trace).unwrap_or_default(),
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        started_at: to_utc(model.started_at),
        completed_at: model.completed_at.map(to_utc),
        updated_at: to_utc(model.updated_at),
    })
}

pub fn map_recent_run_record(
    model: ai_chat_runs::Model,
    sessions: &HashMap<Uuid, ai_chat_sessions::Model>,
    providers: &HashMap<Uuid, ai_provider_profiles::Model>,
    tasks: &HashMap<Uuid, ai_task_profiles::Model>,
) -> AiResult<AiRecentRunRecord> {
    let session_title = sessions
        .get(&model.session_id)
        .map(|session| session.title.clone())
        .unwrap_or_else(|| model.session_id.to_string());
    let provider = providers.get(&model.provider_profile_id);
    let task = model
        .task_profile_id
        .and_then(|task_id| tasks.get(&task_id));
    let completed_at = model.completed_at.map(to_utc);
    let started_at = to_utc(model.started_at);
    let updated_at = to_utc(model.updated_at);
    let duration_ms = completed_at
        .unwrap_or(updated_at)
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0);
    let decision_trace: AiRunDecisionTrace =
        serde_json::from_value(model.decision_trace).unwrap_or_default();

    let provider_kind = match provider {
        Some(value) => provider_kind_from_slug(&value.provider_kind)?,
        None => ProviderKind::OpenAiCompatible,
    };

    Ok(AiRecentRunRecord {
        id: model.id,
        session_id: model.session_id,
        session_title,
        provider_profile_id: model.provider_profile_id,
        provider_display_name: provider
            .map(|value| value.display_name.clone())
            .unwrap_or_else(|| model.provider_profile_id.to_string()),
        provider_kind,
        task_profile_id: model.task_profile_id,
        task_profile_slug: task.map(|value| value.slug.clone()),
        status: model.status,
        model: model.model,
        execution_mode: execution_mode_from_slug(&model.execution_mode)?,
        execution_path: execution_mode_from_slug(&model.execution_path)?,
        execution_target: decision_trace.execution_target,
        requested_locale: model.requested_locale,
        resolved_locale: model.resolved_locale,
        error_message: model.error_message,
        started_at,
        completed_at,
        updated_at,
        duration_ms,
    })
}

pub fn map_approval_record(model: ai_approval_requests::Model) -> AiApprovalRequestRecord {
    AiApprovalRequestRecord {
        id: model.id,
        session_id: model.session_id,
        run_id: model.run_id,
        tool_name: model.tool_name,
        tool_call_id: model.tool_call_id,
        tool_input: model.tool_input,
        reason: model.reason,
        status: model.status,
        resolved_by: model.resolved_by,
        resolved_at: model.resolved_at.map(to_utc),
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    }
}

pub fn map_trace_record(model: ai_tool_traces::Model) -> ToolTrace {
    ToolTrace {
        tool_name: model.tool_name,
        input_payload: model.input_payload,
        output_payload: model.output_payload,
        status: model.status,
        duration_ms: model.duration_ms.unwrap_or_default(),
        sensitive: model.sensitive,
        error_message: model.error_message,
        created_at: to_utc(model.created_at),
    }
}

pub fn map_chat_message(model: ai_chat_messages::Model) -> AiResult<ChatMessage> {
    Ok(ChatMessage {
        role: map_role(&model.role)?,
        content: model.content,
        name: model.name,
        tool_call_id: model.tool_call_id,
        tool_calls: serde_json::from_value(model.tool_calls).map_err(json_err)?,
        metadata: model.metadata,
    })
}

pub fn map_role(value: &str) -> AiResult<ChatMessageRole> {
    match value {
        "system" => Ok(ChatMessageRole::System),
        "user" => Ok(ChatMessageRole::User),
        "assistant" => Ok(ChatMessageRole::Assistant),
        "tool" => Ok(ChatMessageRole::Tool),
        other => Err(AiError::Runtime(format!(
            "unknown AI message role: {other}"
        ))),
    }
}

pub fn role_slug(role: ChatMessageRole) -> &'static str {
    match role {
        ChatMessageRole::System => "system",
        ChatMessageRole::User => "user",
        ChatMessageRole::Assistant => "assistant",
        ChatMessageRole::Tool => "tool",
    }
}

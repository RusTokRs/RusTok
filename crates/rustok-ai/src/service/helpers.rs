use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Statement,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use rustok_api::normalize_locale_tag;
use rustok_api::{Action, Permission};

use crate::entities::{
    ai_approval_requests, ai_chat_messages, ai_chat_runs, ai_chat_sessions, ai_provider_profiles,
    ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::model::{
    AiProviderConfig, AiRunDecisionTrace, ChatMessage, ChatMessageRole, ExecutionMode,
    PendingApproval, ProviderCapability, ProviderStreamEvent, ProviderUsagePolicy, ToolTrace,
};
use crate::policy::ToolExecutionPolicy;
use crate::streaming::{AiRunStreamEvent, AiRunStreamEventKind, ai_run_stream_hub};
use crate::{AiError, AiResult, ProviderEgressPolicy, ProviderSlug};

use super::mapping::role_slug;
use super::types::AiOperatorContext;

pub fn db_err(error: impl std::fmt::Display) -> AiError {
    AiError::Runtime(error.to_string())
}

pub fn json_err(error: impl std::fmt::Display) -> AiError {
    AiError::Serialization(error.to_string())
}

pub fn to_utc(value: sea_orm::prelude::DateTimeWithTimeZone) -> DateTime<Utc> {
    value.with_timezone(&Utc)
}

pub fn validate_slug(value: &str) -> AiResult<()> {
    let slug = value.trim();
    if slug.is_empty() {
        return Err(AiError::Validation("slug is required".to_string()));
    }
    if slug.len() > 96 {
        return Err(AiError::Validation("slug is too long".to_string()));
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AiError::Validation(
            "slug must contain only lowercase letters, digits, '-' or '_'".to_string(),
        ));
    }
    Ok(())
}

pub fn parse_uuid_str(value: Option<&str>) -> AiResult<Uuid> {
    let value = value
        .ok_or_else(|| AiError::Runtime("tenant id is missing in AI access context".to_string()))?;
    Uuid::parse_str(value).map_err(|error| AiError::Runtime(format!("invalid uuid: {error}")))
}

pub fn provider_slug_from_str(value: &str) -> AiResult<ProviderSlug> {
    ProviderSlug::new(value).map_err(AiError::Validation)
}

pub fn validate_provider_profile_contract(
    provider_slug: &ProviderSlug,
    settings: &std::collections::BTreeMap<String, serde_json::Value>,
    credential_refs: &std::collections::BTreeMap<String, rustok_secrets::SecretRef>,
    egress_policy: &ProviderEgressPolicy,
) -> AiResult<()> {
    let descriptor = crate::provider_catalog_entry(provider_slug).ok_or_else(|| {
        AiError::Validation(format!(
            "unknown Rig provider integration `{provider_slug}`"
        ))
    })?;
    if !descriptor.compiled_in {
        return Err(AiError::Validation(format!(
            "Rig provider integration `{provider_slug}` is not compiled into this deployment"
        )));
    }
    egress_policy
        .validate_settings(descriptor, settings)
        .map_err(AiError::Validation)?;
    for field in descriptor.credentials {
        if field.required && !credential_refs.contains_key(field.key) {
            return Err(AiError::Validation(format!(
                "provider credential reference `{}` is required",
                field.key
            )));
        }
    }
    for key in credential_refs.keys() {
        if !descriptor.credentials.iter().any(|field| field.key == key) {
            return Err(AiError::Validation(format!(
                "unknown provider credential reference `{key}`"
            )));
        }
    }
    Ok(())
}

pub fn validate_provider_target_profile_contract(
    targets: &crate::AiProviderTargetCatalog,
    target_id: &crate::ProviderTargetId,
    credential_refs: &std::collections::BTreeMap<String, rustok_secrets::SecretRef>,
    egress_policy: &ProviderEgressPolicy,
) -> AiResult<crate::ProviderSlug> {
    let target = targets.get(target_id).ok_or_else(|| {
        AiError::Validation(format!("unknown deployment provider target `{target_id}`"))
    })?;
    match target.auth {
        crate::ProviderTargetAuth::SecretRefs => validate_provider_profile_contract(
            &target.provider_slug,
            &target.settings,
            credential_refs,
            egress_policy,
        )?,
        crate::ProviderTargetAuth::WorkloadIdentity | crate::ProviderTargetAuth::None => {
            let descriptor =
                crate::provider_catalog_entry(&target.provider_slug).ok_or_else(|| {
                    AiError::Validation(format!(
                        "unknown Rig provider integration `{}`",
                        target.provider_slug
                    ))
                })?;
            egress_policy
                .validate_settings(descriptor, &target.settings)
                .map_err(AiError::Validation)?;
            if !credential_refs.is_empty() {
                return Err(AiError::Validation(format!(
                    "deployment target `{target_id}` does not accept tenant credential references"
                )));
            }
        }
    }
    Ok(target.provider_slug.clone())
}

pub fn capability_from_slug(value: &str) -> AiResult<ProviderCapability> {
    match value {
        "text_generation" => Ok(ProviderCapability::TextGeneration),
        "structured_generation" => Ok(ProviderCapability::StructuredGeneration),
        "image_generation" => Ok(ProviderCapability::ImageGeneration),
        "multimodal_understanding" => Ok(ProviderCapability::MultimodalUnderstanding),
        "code_generation" => Ok(ProviderCapability::CodeGeneration),
        "alloy_assist" => Ok(ProviderCapability::AlloyAssist),
        other => Err(AiError::Runtime(format!(
            "invalid provider capability slug: {other}"
        ))),
    }
}

pub fn execution_mode_from_slug(value: &str) -> AiResult<ExecutionMode> {
    match value {
        "auto" => Ok(ExecutionMode::Auto),
        "direct" => Ok(ExecutionMode::Direct),
        "mcp_tooling" => Ok(ExecutionMode::McpTooling),
        other => Err(AiError::Runtime(format!(
            "invalid execution mode slug: {other}"
        ))),
    }
}

pub fn provider_config(
    model: &ai_provider_profiles::Model,
    targets: &crate::AiProviderTargetCatalog,
    egress_policy: &ProviderEgressPolicy,
) -> AiResult<AiProviderConfig> {
    let target_id =
        crate::ProviderTargetId::new(&model.provider_target_id).map_err(AiError::InvalidConfig)?;
    let target = targets.get(&target_id).ok_or_else(|| {
        AiError::InvalidConfig(format!(
            "deployment provider target `{target_id}` is unavailable"
        ))
    })?;
    let provider_slug = provider_slug_from_str(&model.provider_slug)?;
    if target.provider_slug != provider_slug {
        return Err(AiError::InvalidConfig(format!(
            "provider profile `{}` does not match deployment target `{target_id}`",
            model.slug
        )));
    }
    let credential_refs =
        serde_json::from_value(model.credential_refs.clone()).map_err(json_err)?;
    validate_provider_target_profile_contract(targets, &target_id, &credential_refs, egress_policy)
        .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
    Ok(AiProviderConfig {
        tenant_id: model.tenant_id,
        provider_slug,
        target_auth: target.auth,
        model: model.model.clone(),
        settings: target.settings.clone(),
        credential_refs,
        temperature: model.temperature,
        max_tokens: model.max_tokens.map(|value| value.max(0) as u32),
        capabilities: capability_list(&model.capabilities)?,
        usage_policy: ProviderUsagePolicy {
            allowed_task_profiles: string_list(&model.allowed_task_profiles),
            denied_task_profiles: string_list(&model.denied_task_profiles),
            restricted_role_slugs: string_list(&model.restricted_role_slugs),
        },
    })
}

pub fn policy_from_model(model: Option<&ai_tool_profiles::Model>) -> ToolExecutionPolicy {
    let mut sensitive_tools = match model {
        Some(model) => string_list(&model.sensitive_tools),
        None => Vec::new(),
    };
    merge_content_ai_sensitive_tools(&mut sensitive_tools);

    match model {
        Some(model) => ToolExecutionPolicy::new(
            match string_list(&model.allowed_tools) {
                values if values.is_empty() => None,
                values => Some(values),
            },
            string_list(&model.denied_tools),
            sensitive_tools,
        ),
        None => ToolExecutionPolicy::new(None, Vec::new(), sensitive_tools),
    }
}

fn merge_content_ai_sensitive_tools(sensitive_tools: &mut Vec<String>) {
    for tool_name in rustok_ai_content::content_ai_sensitive_tools() {
        if !sensitive_tools
            .iter()
            .any(|existing| existing == &tool_name)
        {
            sensitive_tools.push(tool_name);
        }
    }
}

pub fn string_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| item.as_str().map(|value| value.to_string()))
        .collect()
}

pub fn uuid_list(value: &serde_json::Value) -> Vec<Uuid> {
    string_list(value)
        .into_iter()
        .filter_map(|value| Uuid::parse_str(&value).ok())
        .collect()
}

pub fn capability_list(value: &serde_json::Value) -> AiResult<Vec<ProviderCapability>> {
    string_list(value)
        .into_iter()
        .map(|value| capability_from_slug(&value))
        .collect()
}

pub fn to_json_array(values: Vec<String>) -> AiResult<serde_json::Value> {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>(),
    )
    .map_err(json_err)
}

pub fn uuid_json_array(values: Vec<Uuid>) -> serde_json::Value {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| json!([]))
}

pub fn capability_json_array(values: Vec<ProviderCapability>) -> serde_json::Value {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.slug().to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| json!([]))
}

pub fn normalize_metadata(value: serde_json::Value) -> serde_json::Value {
    if value.is_object() { value } else { json!({}) }
}

pub fn normalize_nonempty(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn merge_metadata(base: serde_json::Value, extension: serde_json::Value) -> serde_json::Value {
    let mut merged = normalize_metadata(base);
    if let (Some(target), Some(source)) = (merged.as_object_mut(), extension.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    merged
}

pub fn has_effective_permission(operator: &AiOperatorContext, permission: Permission) -> bool {
    operator.permissions.contains(&permission)
        || operator
            .permissions
            .contains(&Permission::new(permission.resource, Action::Manage))
}

pub fn ensure_permission(operator: &AiOperatorContext, permission: Permission) -> AiResult<()> {
    if has_effective_permission(operator, permission) {
        Ok(())
    } else {
        Err(AiError::Validation(format!(
            "permission denied: {}",
            permission
        )))
    }
}

pub fn enforce_task_permissions(
    operator: &AiOperatorContext,
    task_profile: Option<&ai_task_profiles::Model>,
) -> AiResult<()> {
    let Some(task_profile) = task_profile else {
        return ensure_permission(operator, Permission::AI_TASKS_TEXT_RUN);
    };

    match capability_from_slug(&task_profile.target_capability)? {
        ProviderCapability::TextGeneration | ProviderCapability::StructuredGeneration => {
            ensure_permission(operator, Permission::AI_TASKS_TEXT_RUN)?;
        }
        ProviderCapability::ImageGeneration => {
            ensure_permission(operator, Permission::AI_TASKS_IMAGE_RUN)?;
        }
        ProviderCapability::MultimodalUnderstanding => {
            ensure_permission(operator, Permission::AI_TASKS_MULTIMODAL_RUN)?;
        }
        ProviderCapability::CodeGeneration => {
            ensure_permission(operator, Permission::AI_TASKS_CODE_RUN)?;
        }
        ProviderCapability::AlloyAssist => {
            ensure_permission(operator, Permission::AI_TASKS_ALLOY_RUN)?;
        }
    }

    if task_profile.slug == "alloy_code" {
        ensure_permission(operator, Permission::AI_TASKS_CODE_RUN)?;
        ensure_permission(operator, Permission::AI_TASKS_ALLOY_RUN)?;
    }

    if matches!(
        task_profile.slug.as_str(),
        "product_copy" | "product_attributes"
    ) {
        ensure_permission(operator, Permission::AI_TASKS_TEXT_RUN)?;
        ensure_permission(operator, Permission::PRODUCTS_UPDATE)?;
    }

    Ok(())
}

pub async fn resolve_task_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    preferred_locale: Option<&str>,
    requested_locale: Option<&str>,
    task_slug: Option<&str>,
) -> AiResult<String> {
    let requested = validate_locale_tag_opt(requested_locale)?;
    let preferred = validate_locale_tag_opt(preferred_locale)?;
    let (tenant_default_locale, tenant_enabled_locales) =
        load_tenant_locale_policy(db, tenant_id).await?;
    let tenant_default_locale = tenant_default_locale.unwrap_or_else(|| "en".to_string());

    if task_slug.is_some_and(task_allows_free_locale) {
        return Ok(requested.or(preferred).unwrap_or(tenant_default_locale));
    }

    for candidate in [
        requested.clone(),
        preferred,
        Some(tenant_default_locale.clone()),
    ]
    .into_iter()
    .flatten()
    {
        if tenant_enabled_locales.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Ok(tenant_default_locale)
}

pub fn validate_locale_tag_opt(locale: Option<&str>) -> AiResult<Option<String>> {
    locale.map(validate_locale_tag).transpose()
}

pub fn validate_locale_tag(locale: &str) -> AiResult<String> {
    normalize_locale_tag(locale)
        .ok_or_else(|| AiError::Validation(format!("invalid locale `{locale}`")))
}

pub async fn load_tenant_locale_policy(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> AiResult<(Option<String>, Vec<String>)> {
    let backend = db.get_database_backend();
    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            "SELECT default_locale, settings FROM tenants WHERE id = ?1",
            vec![tenant_id.into()],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            "SELECT default_locale, settings FROM tenants WHERE id = $1",
            vec![tenant_id.into()],
        ),
    };

    let Some(row) = db.query_one(statement).await.map_err(db_err)? else {
        return Ok((Some("en".to_string()), vec!["en".to_string()]));
    };

    let default_locale = row
        .try_get::<String>("", "default_locale")
        .ok()
        .and_then(|value| validate_locale_tag_opt(Some(value.as_str())).ok().flatten());
    let settings = row
        .try_get::<serde_json::Value>("", "settings")
        .unwrap_or_else(|_| json!({}));
    let mut enabled_locales = locale_list_from_settings(&settings);
    if let Some(default_locale) = default_locale.as_ref() {
        if !enabled_locales.contains(default_locale) {
            enabled_locales.push(default_locale.clone());
        }
    }
    if enabled_locales.is_empty() {
        enabled_locales.push(default_locale.clone().unwrap_or_else(|| "en".to_string()));
    }

    Ok((default_locale, enabled_locales))
}

pub fn locale_list_from_settings(settings: &serde_json::Value) -> Vec<String> {
    let mut locales = Vec::new();
    for key in ["enabled_locales", "supported_locales", "locales"] {
        if let Some(values) = settings.get(key).and_then(|value| value.as_array()) {
            for value in values {
                if let Some(locale) = value.as_str() {
                    if let Ok(locale) = validate_locale_tag(locale) {
                        if !locales.contains(&locale) {
                            locales.push(locale);
                        }
                    }
                }
            }
        }
    }
    locales
}

pub fn task_allows_free_locale(task_slug: &str) -> bool {
    matches!(
        task_slug,
        "operator_chat" | "alloy_code" | "summarization" | "translation"
    )
}

pub fn runtime_execution_target(execution_mode: ExecutionMode) -> &'static str {
    match execution_mode {
        ExecutionMode::Auto => "runtime:auto",
        ExecutionMode::Direct => "direct:runtime",
        ExecutionMode::McpTooling => "mcp:rustok-mcp",
    }
}

pub fn enrich_decision_trace(
    mut trace: AiRunDecisionTrace,
    execution_mode: ExecutionMode,
    requested_locale: Option<String>,
    resolved_locale: String,
) -> AiRunDecisionTrace {
    trace.execution_mode = Some(execution_mode);
    trace.requested_locale = requested_locale;
    trace.resolved_locale = Some(resolved_locale);
    if trace.execution_target.is_none() {
        trace.execution_target = Some(match execution_mode {
            ExecutionMode::Direct => "direct".to_string(),
            ExecutionMode::McpTooling => "mcp:rustok-mcp".to_string(),
            ExecutionMode::Auto => "auto".to_string(),
        });
    }
    trace
}

pub fn build_task_job_user_message(
    task_slug: &str,
    requested_locale: Option<&str>,
    resolved_locale: &str,
    task_input: &serde_json::Value,
) -> ChatMessage {
    let mut metadata = json!({
        "task_job": true,
        "task_slug": task_slug,
        "resolved_locale": resolved_locale,
        "task_input": task_input,
    });
    if let Some(requested_locale) = requested_locale {
        metadata["requested_locale"] = json!(requested_locale);
    }

    let pretty_input =
        serde_json::to_string_pretty(task_input).unwrap_or_else(|_| task_input.to_string());
    ChatMessage {
        role: ChatMessageRole::User,
        content: Some(format!(
            "Run AI task `{task_slug}` in locale `{resolved_locale}`.\n\n```json\n{pretty_input}\n```"
        )),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
        metadata,
    }
}

pub fn publish_ai_run_stream_event(
    session_id: Uuid,
    run_id: Uuid,
    event_kind: AiRunStreamEventKind,
    content_delta: Option<String>,
    accumulated_content: Option<String>,
    error_message: Option<String>,
) {
    ai_run_stream_hub().publish(AiRunStreamEvent {
        session_id,
        run_id,
        event_kind,
        content_delta,
        accumulated_content,
        error_message,
        tool_call: None,
        usage: None,
        sequence: 0,
        created_at: Utc::now(),
    });
}

pub fn publish_ai_run_tool_call_stream_event(
    session_id: Uuid,
    run_id: Uuid,
    tool_call: crate::model::ToolCall,
) {
    ai_run_stream_hub().publish(AiRunStreamEvent {
        session_id,
        run_id,
        event_kind: AiRunStreamEventKind::ToolCall,
        content_delta: None,
        accumulated_content: None,
        error_message: None,
        tool_call: Some(tool_call),
        usage: None,
        sequence: 0,
        created_at: Utc::now(),
    });
}

pub fn publish_ai_run_usage_stream_event(
    session_id: Uuid,
    run_id: Uuid,
    usage: crate::model::ProviderUsage,
) {
    ai_run_stream_hub().publish(AiRunStreamEvent {
        session_id,
        run_id,
        event_kind: AiRunStreamEventKind::Usage,
        content_delta: None,
        accumulated_content: None,
        error_message: None,
        tool_call: None,
        usage: Some(usage),
        sequence: 0,
        created_at: Utc::now(),
    });
}

/// Normalizes the provider stream boundary into the single canonical RusToK event contract.
/// Provider-specific payload shapes must be resolved before this function is called.
pub fn publish_provider_stream_event(
    session_id: Uuid,
    run_id: Uuid,
    stream_buffer: &Arc<Mutex<String>>,
    event: ProviderStreamEvent,
) {
    let hub = ai_run_stream_hub();
    publish_provider_stream_event_to_hub(&hub, session_id, run_id, stream_buffer, event);
}

fn publish_provider_stream_event_to_hub(
    hub: &crate::streaming::AiRunStreamHub,
    session_id: Uuid,
    run_id: Uuid,
    stream_buffer: &Arc<Mutex<String>>,
    event: ProviderStreamEvent,
) {
    match event {
        ProviderStreamEvent::TextDelta(delta) => {
            let accumulated_content = {
                let mut accumulated = stream_buffer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                accumulated.push_str(&delta);
                accumulated.clone()
            };
            hub.publish(AiRunStreamEvent {
                session_id,
                run_id,
                event_kind: AiRunStreamEventKind::Delta,
                content_delta: Some(delta),
                accumulated_content: Some(accumulated_content),
                error_message: None,
                tool_call: None,
                usage: None,
                sequence: 0,
                created_at: Utc::now(),
            });
        }
        ProviderStreamEvent::ToolCall(tool_call) => {
            hub.publish(AiRunStreamEvent {
                session_id,
                run_id,
                event_kind: AiRunStreamEventKind::ToolCall,
                content_delta: None,
                accumulated_content: None,
                error_message: None,
                tool_call: Some(tool_call),
                usage: None,
                sequence: 0,
                created_at: Utc::now(),
            });
        }
        ProviderStreamEvent::Usage(usage) => {
            hub.publish(AiRunStreamEvent {
                session_id,
                run_id,
                event_kind: AiRunStreamEventKind::Usage,
                content_delta: None,
                accumulated_content: None,
                error_message: None,
                tool_call: None,
                usage: Some(usage),
                sequence: 0,
                created_at: Utc::now(),
            });
        }
    }
}

pub fn read_stream_buffer(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|value| value.clone()).unwrap_or_default()
}

pub async fn session_task_input(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    session_id: Uuid,
) -> AiResult<Option<serde_json::Value>> {
    let session = require_session(db, tenant_id, session_id).await?;
    Ok(session.metadata.get("task_input").cloned())
}

pub async fn mark_run_failed(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    run_id: Uuid,
    error_message: String,
) -> AiResult<()> {
    let run = require_run(db, tenant_id, run_id).await?;
    let mut active: ai_chat_runs::ActiveModel = run.into();
    active.status = Set("failed".to_string());
    active.error_message = Set(Some(error_message));
    active.completed_at = Set(Some(Utc::now().into()));
    active.updated_at = Set(Utc::now().into());
    active.update(db).await.map_err(db_err)?;
    Ok(())
}

pub async fn list_router_provider_profiles(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> AiResult<Vec<crate::router::RouterProviderProfile>> {
    ai_provider_profiles::Entity::find()
        .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
        .all(db)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(|model| {
            Ok(crate::router::RouterProviderProfile {
                id: model.id,
                slug: model.slug,
                provider_slug: provider_slug_from_str(&model.provider_slug)?,
                model: model.model,
                capabilities: capability_list(&model.capabilities)?,
                usage_policy: ProviderUsagePolicy {
                    allowed_task_profiles: string_list(&model.allowed_task_profiles),
                    denied_task_profiles: string_list(&model.denied_task_profiles),
                    restricted_role_slugs: string_list(&model.restricted_role_slugs),
                },
                is_active: model.is_active,
            })
        })
        .collect()
}

pub async fn require_provider_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_provider_profiles::Model> {
    ai_provider_profiles::Entity::find_by_id(id)
        .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI provider profile not found".to_string()))
}

pub async fn require_tool_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_tool_profiles::Model> {
    ai_tool_profiles::Entity::find_by_id(id)
        .filter(ai_tool_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI tool profile not found".to_string()))
}

pub async fn require_task_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_task_profiles::Model> {
    ai_task_profiles::Entity::find_by_id(id)
        .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI task profile not found".to_string()))
}

pub async fn require_session(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_chat_sessions::Model> {
    ai_chat_sessions::Entity::find_by_id(id)
        .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI chat session not found".to_string()))
}

pub async fn require_run(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_chat_runs::Model> {
    ai_chat_runs::Entity::find_by_id(id)
        .filter(ai_chat_runs::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI chat run not found".to_string()))
}

pub async fn insert_message<C>(
    db: &C,
    tenant_id: Uuid,
    session_id: Uuid,
    run_id: Option<Uuid>,
    created_by: Option<Uuid>,
    message: ChatMessage,
) -> AiResult<ai_chat_messages::Model>
where
    C: sea_orm::ConnectionTrait,
{
    ai_chat_messages::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        role: Set(role_slug(message.role).to_string()),
        content: Set(message.content),
        name: Set(message.name),
        tool_call_id: Set(message.tool_call_id),
        tool_calls: Set(serde_json::to_value(message.tool_calls).map_err(json_err)?),
        metadata: Set(message.metadata),
        created_by: Set(created_by),
        created_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .map_err(db_err)
}

pub async fn insert_tool_trace<C>(
    db: &C,
    tenant_id: Uuid,
    session_id: Uuid,
    run_id: Uuid,
    trace: &ToolTrace,
) -> AiResult<ai_tool_traces::Model>
where
    C: ConnectionTrait,
{
    ai_tool_traces::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        tool_name: Set(trace.tool_name.clone()),
        status: Set(trace.status.clone()),
        input_payload: Set(trace.input_payload.clone()),
        output_payload: Set(trace.output_payload.clone()),
        error_message: Set(trace.error_message.clone()),
        duration_ms: Set(Some(trace.duration_ms)),
        sensitive: Set(trace.sensitive),
        created_at: Set(trace.created_at.into()),
        updated_at: Set(trace.created_at.into()),
    }
    .insert(db)
    .await
    .map_err(db_err)
}

pub async fn insert_approval_request(
    db: &DatabaseConnection,
    operator: &AiOperatorContext,
    session_id: Uuid,
    run_id: Uuid,
    approval_batch_id: Uuid,
    approval: &PendingApproval,
) -> AiResult<ai_approval_requests::Model> {
    ai_approval_requests::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(operator.tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        approval_batch_id: Set(approval_batch_id.to_string()),
        tool_name: Set(approval.tool_name.clone()),
        tool_call_id: Set(approval.tool_call_id.clone()),
        tool_input: Set(approval.input_payload.clone()),
        reason: Set(Some(approval.reason.clone())),
        status: Set("pending".to_string()),
        resolved_by: Set(None),
        resolved_at: Set(None),
        metadata: Set(json!({})),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .map_err(db_err)
}

pub async fn persist_runtime_outputs(
    db: &DatabaseConnection,
    operator: &AiOperatorContext,
    session_id: Uuid,
    run_id: Uuid,
    messages: Vec<ChatMessage>,
    traces: Vec<ToolTrace>,
) -> AiResult<()> {
    for message in messages {
        insert_message(
            db,
            operator.tenant_id,
            session_id,
            Some(run_id),
            Some(operator.user_id),
            message,
        )
        .await?;
    }
    for trace in traces {
        insert_tool_trace(db, operator.tenant_id, session_id, run_id, &trace).await?;
    }
    let session = require_session(db, operator.tenant_id, session_id).await?;
    let mut active: ai_chat_sessions::ActiveModel = session.into();
    active.updated_at = Set(Utc::now().into());
    active.update(db).await.map_err(db_err)?;
    Ok(())
}

pub async fn session_has_user_messages(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    session_id: Uuid,
) -> AiResult<bool> {
    let count = ai_chat_messages::Entity::find()
        .filter(
            Condition::all()
                .add(ai_chat_messages::Column::TenantId.eq(tenant_id))
                .add(ai_chat_messages::Column::SessionId.eq(session_id))
                .add(ai_chat_messages::Column::Role.eq("user")),
        )
        .count(db)
        .await
        .map_err(db_err)?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::{
        build_task_job_user_message, enrich_decision_trace, publish_provider_stream_event,
        publish_provider_stream_event_to_hub, runtime_execution_target, task_allows_free_locale,
        validate_locale_tag, validate_provider_target_profile_contract,
    };
    use crate::model::{
        AiRunDecisionTrace, ExecutionMode, ProviderStreamEvent, ProviderUsage, ToolCall,
    };
    use crate::streaming::{
        AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub, ai_run_stream_hub,
    };
    use crate::{
        AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy, ProviderSlug,
        ProviderTargetAuth, ProviderTargetId,
    };
    use chrono::Utc;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[derive(Deserialize)]
    struct StreamCassetteDocument {
        rig_version: String,
        cassettes: Vec<StreamCassette>,
    }

    #[derive(Deserialize)]
    struct StreamCassette {
        family: String,
        events: Vec<StreamCassetteEvent>,
        terminal: StreamCassetteTerminal,
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum StreamCassetteEvent {
        Text {
            value: String,
        },
        ToolCall {
            id: String,
            name: String,
            arguments: serde_json::Value,
        },
        Usage {
            input_tokens: u64,
            output_tokens: u64,
            total_tokens: u64,
        },
    }

    #[derive(Deserialize)]
    struct StreamCassetteTerminal {
        kind: String,
        error: Option<String>,
    }

    #[test]
    fn provider_events_are_normalized_once_before_transport_publication() {
        let session_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut receiver = ai_run_stream_hub().subscribe();

        publish_provider_stream_event(
            session_id,
            run_id,
            &buffer,
            ProviderStreamEvent::TextDelta("hello".to_string()),
        );
        publish_provider_stream_event(
            session_id,
            run_id,
            &buffer,
            ProviderStreamEvent::ToolCall(ToolCall {
                id: "call-1".to_string(),
                name: "catalog.read".to_string(),
                arguments: serde_json::json!({ "id": 7 }),
            }),
        );
        publish_provider_stream_event(
            session_id,
            run_id,
            &buffer,
            ProviderStreamEvent::Usage(ProviderUsage::normalized(2, 3, None)),
        );

        let delta = receiver.try_recv().expect("delta event");
        let tool = receiver.try_recv().expect("tool event");
        let usage = receiver.try_recv().expect("usage event");
        assert_eq!(delta.event_kind, AiRunStreamEventKind::Delta);
        assert_eq!(delta.accumulated_content.as_deref(), Some("hello"));
        assert_eq!(tool.tool_call.expect("tool call").name, "catalog.read");
        assert_eq!(usage.usage.expect("usage").total_tokens, 5);
        assert_eq!([delta.sequence, tool.sequence, usage.sequence], [1, 2, 3]);
    }

    #[test]
    fn rig_stream_cassettes_cover_every_supported_protocol_family() {
        let document: StreamCassetteDocument = serde_json::from_str(include_str!(
            "../../contracts/rig-0.39-stream-cassettes.json"
        ))
        .expect("stream cassette document is valid JSON");
        assert_eq!(document.rig_version, "0.39.0");
        assert_eq!(
            document
                .cassettes
                .iter()
                .map(|cassette| cassette.family.as_str())
                .collect::<Vec<_>>(),
            [
                "openai_compatible",
                "anthropic",
                "gemini",
                "cloud_auth",
                "deployment_local"
            ]
        );
        for cassette in document.cassettes {
            let hub = AiRunStreamHub::new(32);
            let mut receiver = hub.subscribe();
            let session_id = Uuid::new_v4();
            let run_id = Uuid::new_v4();
            let buffer = Arc::new(Mutex::new(String::new()));
            for event in &cassette.events {
                let event = match event {
                    StreamCassetteEvent::Text { value } => {
                        ProviderStreamEvent::TextDelta(value.clone())
                    }
                    StreamCassetteEvent::ToolCall {
                        id,
                        name,
                        arguments,
                    } => ProviderStreamEvent::ToolCall(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    }),
                    StreamCassetteEvent::Usage {
                        input_tokens,
                        output_tokens,
                        total_tokens,
                    } => ProviderStreamEvent::Usage(ProviderUsage::normalized(
                        *input_tokens,
                        *output_tokens,
                        Some(*total_tokens),
                    )),
                };
                publish_provider_stream_event_to_hub(&hub, session_id, run_id, &buffer, event);
            }
            let terminal_kind = match cassette.terminal.kind.as_str() {
                "completed" => AiRunStreamEventKind::Completed,
                "failed" => AiRunStreamEventKind::Failed,
                "cancelled" => AiRunStreamEventKind::Cancelled,
                other => panic!("unsupported terminal fixture kind `{other}`"),
            };
            let terminal = AiRunStreamEvent {
                session_id,
                run_id,
                event_kind: terminal_kind,
                content_delta: None,
                accumulated_content: None,
                error_message: cassette.terminal.error.clone(),
                tool_call: None,
                usage: None,
                sequence: 0,
                created_at: Utc::now(),
            };
            assert!(hub.publish(terminal.clone()));
            assert!(!hub.publish(terminal));
            let events = (0..=cassette.events.len())
                .map(|_| receiver.try_recv().expect("normalized stream event"))
                .collect::<Vec<_>>();
            assert!(
                events
                    .iter()
                    .enumerate()
                    .all(|(index, event)| event.sequence == (index + 1) as u64)
            );
            assert!(
                events
                    .iter()
                    .all(|event| event.session_id == session_id && event.run_id == run_id)
            );
            assert!(
                events
                    .iter()
                    .any(|event| event.event_kind == AiRunStreamEventKind::Delta
                        && event.accumulated_content.is_some())
            );
            let terminal = events.last().expect("terminal event");
            assert_eq!(terminal.event_kind, terminal_kind);
            assert_eq!(terminal.error_message, cassette.terminal.error);
        }
    }

    #[test]
    fn validate_locale_tag_normalizes_common_bcp47_forms() {
        assert_eq!(validate_locale_tag("pt_br").unwrap(), "pt-BR");
        assert_eq!(validate_locale_tag("zh-hant").unwrap(), "zh-Hant");
        assert_eq!(validate_locale_tag("es-419").unwrap(), "es-419");
    }

    #[test]
    fn validate_locale_tag_rejects_invalid_values() {
        assert!(validate_locale_tag("").is_err());
        assert!(validate_locale_tag("en-*").is_err());
    }

    #[test]
    fn build_task_job_user_message_embeds_locale_metadata() {
        let message = build_task_job_user_message(
            "blog_draft",
            Some("de"),
            "de",
            &serde_json::json!({ "title": "Hallo" }),
        );
        assert!(
            message
                .content
                .as_deref()
                .is_some_and(|content| content.contains("blog_draft"))
        );
        assert_eq!(message.metadata["requested_locale"], "de");
        assert_eq!(message.metadata["resolved_locale"], "de");
    }

    #[test]
    fn enrich_decision_trace_sets_execution_target_from_mode() {
        let trace = enrich_decision_trace(
            AiRunDecisionTrace::default(),
            ExecutionMode::McpTooling,
            Some("fr".to_string()),
            "fr".to_string(),
        );
        assert_eq!(trace.execution_target.as_deref(), Some("mcp:rustok-mcp"));
        assert_eq!(trace.requested_locale.as_deref(), Some("fr"));
        assert_eq!(trace.resolved_locale.as_deref(), Some("fr"));
    }

    #[test]
    fn free_locale_tasks_stay_whitelisted() {
        assert!(task_allows_free_locale("alloy_code"));
        assert!(task_allows_free_locale("translation"));
        assert!(!task_allows_free_locale("product_copy"));
        assert_eq!(
            runtime_execution_target(ExecutionMode::Direct),
            "direct:runtime"
        );
    }

    #[test]
    fn deployment_target_contract_rechecks_egress_before_runtime_materialization() {
        let target_id = ProviderTargetId::new("local_ollama").unwrap();
        let targets = AiProviderTargetCatalog::new_with_egress_policy(
            vec![AiProviderTarget {
                id: target_id.clone(),
                provider_slug: ProviderSlug::new("ollama").unwrap(),
                display_name: "Local Ollama".to_string(),
                auth: ProviderTargetAuth::None,
                settings: BTreeMap::from([(
                    "base_url".to_string(),
                    serde_json::json!("http://127.0.0.1:11434"),
                )]),
            }],
            &ProviderEgressPolicy {
                allowed_origins: Vec::new(),
                allow_local_origins: true,
            },
        )
        .unwrap();

        let error = validate_provider_target_profile_contract(
            &targets,
            &target_id,
            &BTreeMap::new(),
            &ProviderEgressPolicy::default(),
        )
        .expect_err("private origin requires an explicit deployment egress policy");

        assert!(error.to_string().contains("loopback"));
    }

    #[test]
    fn target_contract_rejects_unknown_targets_and_credentials_for_non_secret_auth() {
        let target_id = ProviderTargetId::new("workload_vertex").unwrap();
        let targets = AiProviderTargetCatalog::new(vec![AiProviderTarget {
            id: target_id.clone(),
            provider_slug: ProviderSlug::new("vertex_ai").unwrap(),
            display_name: "Vertex workload identity".to_string(),
            auth: ProviderTargetAuth::WorkloadIdentity,
            settings: BTreeMap::from([(
                "project".to_string(),
                serde_json::json!("deployment-project"),
            )]),
        }])
        .unwrap();
        let policy = ProviderEgressPolicy::default();
        let unknown = ProviderTargetId::new("not_catalogued").unwrap();
        assert!(
            validate_provider_target_profile_contract(
                &targets,
                &unknown,
                &BTreeMap::new(),
                &policy,
            )
            .is_err()
        );
        let credentials = BTreeMap::from([(
            "api_key".to_string(),
            rustok_secrets::SecretRef {
                resolver: "env".to_string(),
                key: "RUSTOK_AI_FORBIDDEN".to_string(),
            },
        )]);
        let error =
            validate_provider_target_profile_contract(&targets, &target_id, &credentials, &policy)
                .expect_err("workload identity target must reject tenant credential refs");
        assert!(
            error
                .to_string()
                .contains("does not accept tenant credential references")
        );
    }
}

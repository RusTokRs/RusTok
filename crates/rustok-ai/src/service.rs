pub mod helpers;
pub mod mapping;
pub mod mcp;
pub mod types;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use rustok_api::Permission;

use crate::direct::{DirectExecutionRegistry, DirectExecutionRequest};
use crate::engine::{inference_for_slug, InferenceEngine};
use crate::entities::{
    ai_approval_requests, ai_chat_messages, ai_chat_runs, ai_chat_sessions, ai_provider_profiles,
    ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::metrics::{self as ai_metrics, AiRuntimeMetricsSnapshot};
use crate::model::{
    ChatMessage, ChatMessageRole, ExecutionMode, ExecutionOverride, ProviderStreamEmitter,
    ProviderStreamEvent, ProviderTestResult, RuntimeOutcome, ToolTrace,
};
use crate::router::AiRouter;
use crate::engine::RigAgentDriver;
use crate::streaming::{ai_run_stream_hub, AiRunStreamEvent};
use crate::{AiError, AiResult, McpClientAdapter};

pub use helpers::*;
pub use mapping::*;
pub use mcp::*;
pub use types::*;

pub struct AiManagementService;

impl AiManagementService {
    pub fn metrics_snapshot() -> AiRuntimeMetricsSnapshot {
        ai_metrics::metrics_snapshot()
    }

    pub fn recent_stream_events(session_id: Option<Uuid>, limit: usize) -> Vec<AiRunStreamEvent> {
        ai_run_stream_hub().recent_events(session_id, limit)
    }

    pub async fn list_recent_runs(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: usize,
    ) -> AiResult<Vec<AiRecentRunRecord>> {
        let limit = limit.max(1) as u64;
        let runs = ai_chat_runs::Entity::find()
            .filter(ai_chat_runs::Column::TenantId.eq(tenant_id))
            .order_by_desc(ai_chat_runs::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(db_err)?;

        if runs.is_empty() {
            return Ok(Vec::new());
        }

        let session_ids: Vec<Uuid> = runs.iter().map(|run| run.session_id).collect();
        let provider_ids: Vec<Uuid> = runs.iter().map(|run| run.provider_profile_id).collect();
        let task_ids: Vec<Uuid> = runs.iter().filter_map(|run| run.task_profile_id).collect();

        let session_map: HashMap<Uuid, ai_chat_sessions::Model> = ai_chat_sessions::Entity::find()
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .filter(ai_chat_sessions::Column::Id.is_in(session_ids))
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(|session| (session.id, session))
            .collect();

        let provider_map: HashMap<Uuid, ai_provider_profiles::Model> =
            ai_provider_profiles::Entity::find()
                .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
                .filter(ai_provider_profiles::Column::Id.is_in(provider_ids))
                .all(db)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|provider| (provider.id, provider))
                .collect();

        let task_map: HashMap<Uuid, ai_task_profiles::Model> = if task_ids.is_empty() {
            HashMap::new()
        } else {
            ai_task_profiles::Entity::find()
                .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
                .filter(ai_task_profiles::Column::Id.is_in(task_ids))
                .all(db)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|task| (task.id, task))
                .collect()
        };

        runs.into_iter()
            .map(|run| map_recent_run_record(run, &session_map, &provider_map, &task_map))
            .collect()
    }

    pub async fn list_provider_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiProviderProfileRecord>> {
        let profiles = ai_provider_profiles::Entity::find()
            .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_provider_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        profiles.into_iter().map(map_provider_profile).collect()
    }

    pub async fn get_provider_profile(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<Option<AiProviderProfileRecord>> {
        let profile = ai_provider_profiles::Entity::find_by_id(id)
            .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(db_err)?;
        profile.map(map_provider_profile).transpose()
    }

    pub async fn create_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        provider_targets: &crate::AiProviderTargetCatalog,
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        input: CreateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        validate_slug(&input.slug)?;
        let provider_slug = validate_provider_target_profile_contract(
            provider_targets,
            &input.provider_target_id,
            &input.credential_refs,
            egress_policy,
        )?;
        let profile = ai_provider_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            provider_slug: Set(provider_slug.to_string()),
            provider_target_id: Set(input.provider_target_id.to_string()),
            model: Set(input.model),
            credential_refs: Set(serde_json::to_value(input.credential_refs).map_err(json_err)?),
            temperature: Set(input.temperature),
            max_tokens: Set(input.max_tokens),
            is_active: Set(true),
            capabilities: Set(capability_json_array(input.capabilities)),
            allowed_task_profiles: Set(to_json_array(input.usage_policy.allowed_task_profiles)?),
            denied_task_profiles: Set(to_json_array(input.usage_policy.denied_task_profiles)?),
            restricted_role_slugs: Set(to_json_array(input.usage_policy.restricted_role_slugs)?),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        secrets.invalidate(None).await;
        map_provider_profile(profile)
    }

    pub async fn update_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        provider_targets: &crate::AiProviderTargetCatalog,
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        id: Uuid,
        input: UpdateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        let existing = require_provider_profile(db, operator.tenant_id, id).await?;
        let provider_slug = validate_provider_target_profile_contract(
            provider_targets,
            &input.provider_target_id,
            &input.credential_refs,
            egress_policy,
        )?;
        let mut active: ai_provider_profiles::ActiveModel = existing.into();
        active.display_name = Set(input.display_name);
        active.provider_slug = Set(provider_slug.to_string());
        active.provider_target_id = Set(input.provider_target_id.to_string());
        active.model = Set(input.model);
        active.credential_refs =
            Set(serde_json::to_value(input.credential_refs).map_err(json_err)?);
        active.temperature = Set(input.temperature);
        active.max_tokens = Set(input.max_tokens);
        active.is_active = Set(input.is_active);
        active.capabilities = Set(capability_json_array(input.capabilities));
        active.allowed_task_profiles =
            Set(to_json_array(input.usage_policy.allowed_task_profiles)?);
        active.denied_task_profiles = Set(to_json_array(input.usage_policy.denied_task_profiles)?);
        active.restricted_role_slugs =
            Set(to_json_array(input.usage_policy.restricted_role_slugs)?);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        secrets.invalidate(None).await;
        map_provider_profile(saved)
    }

    pub async fn deactivate_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
    ) -> AiResult<AiProviderProfileRecord> {
        let profile = require_provider_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_provider_profiles::ActiveModel = profile.into();
        active.is_active = Set(false);
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        map_provider_profile(saved)
    }

    pub async fn test_provider_profile(
        db: &DatabaseConnection,
        provider_targets: &crate::AiProviderTargetCatalog,
        secrets: &rustok_secrets::SecretResolverRegistry,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<ProviderTestResult> {
        let profile = require_provider_profile(db, tenant_id, id).await?;
        secrets.invalidate(None).await;
        let config = provider_config(&profile, provider_targets)?;
        if crate::provider_factory_supports(&config.provider_slug, crate::ProviderFeature::Chat) {
            let provider = inference_for_slug(&config.provider_slug, &config, secrets).await?;
            return provider.test_connection(&config).await;
        }
        let started = std::time::Instant::now();
        if crate::provider_factory_supports(&config.provider_slug, crate::ProviderFeature::Embeddings) {
            crate::embed(
                &config,
                secrets,
                crate::EmbeddingRequest {
                    model: config.model.clone(),
                    documents: vec!["RusToK connectivity test".to_string()],
                    dimensions: None,
                },
            )
            .await?;
        } else if crate::provider_factory_supports(&config.provider_slug, crate::ProviderFeature::Rerank) {
            crate::rerank(
                &config,
                secrets,
                crate::RerankRequest {
                    model: config.model.clone(),
                    query: "connectivity".to_string(),
                    documents: vec!["RusToK connectivity test".to_string()],
                    top_n: Some(1),
                },
            )
            .await?;
        } else {
            return Err(AiError::InvalidConfig(format!(
                "Rig provider `{}` has no connectivity-test entrypoint",
                config.provider_slug
            )));
        }
        Ok(ProviderTestResult {
            ok: true,
            provider: config.provider_slug.to_string(),
            model: Some(config.model),
            latency_ms: started.elapsed().as_millis() as i64,
            message: "Provider responded successfully".to_string(),
        })
    }

    pub async fn list_task_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiTaskProfileRecord>> {
        let profiles = ai_task_profiles::Entity::find()
            .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_task_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        profiles
            .into_iter()
            .map(map_task_profile)
            .collect::<AiResult<Vec<_>>>()
    }

    pub async fn create_task_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiTaskProfileInput,
    ) -> AiResult<AiTaskProfileRecord> {
        validate_slug(&input.slug)?;
        let profile = ai_task_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            description: Set(input.description),
            target_capability: Set(input.target_capability.slug().to_string()),
            system_prompt: Set(input.system_prompt),
            allowed_provider_profile_ids: Set(uuid_json_array(input.allowed_provider_profile_ids)),
            preferred_provider_profile_ids: Set(uuid_json_array(
                input.preferred_provider_profile_ids,
            )),
            fallback_strategy: Set(normalize_nonempty(input.fallback_strategy, "ordered")),
            tool_profile_id: Set(input.tool_profile_id),
            approval_policy: Set(normalize_metadata(input.approval_policy)),
            default_execution_mode: Set(input.default_execution_mode.slug().to_string()),
            is_active: Set(true),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        map_task_profile(profile)
    }

    pub async fn update_task_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiTaskProfileInput,
    ) -> AiResult<AiTaskProfileRecord> {
        let profile = require_task_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_task_profiles::ActiveModel = profile.into();
        active.display_name = Set(input.display_name);
        active.description = Set(input.description);
        active.target_capability = Set(input.target_capability.slug().to_string());
        active.system_prompt = Set(input.system_prompt);
        active.allowed_provider_profile_ids =
            Set(uuid_json_array(input.allowed_provider_profile_ids));
        active.preferred_provider_profile_ids =
            Set(uuid_json_array(input.preferred_provider_profile_ids));
        active.fallback_strategy = Set(normalize_nonempty(input.fallback_strategy, "ordered"));
        active.tool_profile_id = Set(input.tool_profile_id);
        active.approval_policy = Set(normalize_metadata(input.approval_policy));
        active.default_execution_mode = Set(input.default_execution_mode.slug().to_string());
        active.is_active = Set(input.is_active);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        map_task_profile(saved)
    }

    pub async fn list_tool_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiToolProfileRecord>> {
        let profiles = ai_tool_profiles::Entity::find()
            .filter(ai_tool_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_tool_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        Ok(profiles.into_iter().map(map_tool_profile).collect())
    }

    pub async fn create_tool_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiToolProfileInput,
    ) -> AiResult<AiToolProfileRecord> {
        validate_slug(&input.slug)?;
        let profile = ai_tool_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            description: Set(input.description),
            allowed_tools: Set(to_json_array(input.allowed_tools)?),
            denied_tools: Set(to_json_array(input.denied_tools)?),
            sensitive_tools: Set(to_json_array(input.sensitive_tools)?),
            is_active: Set(true),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        Ok(map_tool_profile(profile))
    }

    pub async fn update_tool_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiToolProfileInput,
    ) -> AiResult<AiToolProfileRecord> {
        let profile = require_tool_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_tool_profiles::ActiveModel = profile.into();
        active.display_name = Set(input.display_name);
        active.description = Set(input.description);
        active.allowed_tools = Set(to_json_array(input.allowed_tools)?);
        active.denied_tools = Set(to_json_array(input.denied_tools)?);
        active.sensitive_tools = Set(to_json_array(input.sensitive_tools)?);
        active.is_active = Set(input.is_active);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_tool_profile(saved))
    }

    pub async fn start_chat_session(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        input: StartAiChatSessionInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let task_profile = match input.task_profile_id {
            Some(task_profile_id) => {
                let task_profile =
                    require_task_profile(db, operator.tenant_id, task_profile_id).await?;
                if !task_profile.is_active {
                    return Err(AiError::Validation("task profile is inactive".to_string()));
                }
                Some(task_profile)
            }
            None => None,
        };
        enforce_task_permissions(operator, task_profile.as_ref())?;
        if input.override_config.provider_profile_id.is_some()
            || input.override_config.model.is_some()
            || input.execution_mode.is_some()
        {
            ensure_permission(operator, Permission::AI_ROUTER_OVERRIDE)?;
        }
        if let Some(tool_profile_id) = input.tool_profile_id {
            let tool_profile =
                require_tool_profile(db, operator.tenant_id, tool_profile_id).await?;
            if !tool_profile.is_active {
                return Err(AiError::Validation("tool profile is inactive".to_string()));
            }
        }
        let resolved_locale = resolve_task_locale(
            db,
            operator.tenant_id,
            operator.preferred_locale.as_deref(),
            input.locale.as_deref(),
            task_profile.as_ref().map(|profile| profile.slug.as_str()),
        )
        .await?;
        let providers = list_router_provider_profiles(db, operator.tenant_id).await?;
        let task_profile_record = match task_profile.as_ref() {
            Some(profile) => Some(map_task_profile(profile.clone())?),
            None => None,
        };
        let execution_plan = AiRouter::resolve(
            task_profile_record
                .as_ref()
                .map(task_profile_runtime)
                .as_ref(),
            &providers,
            input.provider_profile_id,
            input.tool_profile_id,
            &ExecutionOverride {
                execution_mode: input.execution_mode,
                ..input.override_config.clone()
            },
            &operator.role_slugs,
        )?;
        let decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
        ai_metrics::observe_locale_resolution(input.locale.as_deref(), resolved_locale.as_str());
        ai_metrics::observe_router_resolution("start_chat_session", &decision_trace);

        let txn = db.begin().await.map_err(db_err)?;
        let session = ai_chat_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            title: Set(input.title),
            provider_profile_id: Set(execution_plan.provider_profile_id),
            task_profile_id: Set(execution_plan.task_profile_id),
            tool_profile_id: Set(execution_plan.tool_profile_id),
            execution_mode: Set(execution_plan.execution_mode.slug().to_string()),
            requested_locale: Set(input.locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            status: Set("active".to_string()),
            created_by: Set(Some(operator.user_id)),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({ "decision_trace": decision_trace }),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(db_err)?;

        if let Some(initial) = input
            .initial_message
            .filter(|value| !value.trim().is_empty())
        {
            insert_message(
                &txn,
                operator.tenant_id,
                session.id,
                None,
                Some(operator.user_id),
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(initial),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await?;
        }

        txn.commit().await.map_err(db_err)?;

        if session_has_user_messages(db, operator.tenant_id, session.id).await? {
            Self::execute_latest_turn(runtime, operator, session.id).await
        } else {
            let detail = Self::chat_session_detail(db, operator.tenant_id, session.id)
                .await?
                .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
            Ok(AiSendMessageResult {
                run: AiChatRunRecord {
                    id: Uuid::nil(),
                    session_id: detail.session.id,
                    provider_profile_id: detail.provider_profile.id,
                    task_profile_id: detail.task_profile.as_ref().map(|value| value.id),
                    tool_profile_id: detail.tool_profile.as_ref().map(|value| value.id),
                    status: "idle".to_string(),
                    model: detail.provider_profile.model.clone(),
                    execution_mode: detail.session.execution_mode,
                    execution_path: detail.session.execution_mode,
                    requested_locale: detail.session.requested_locale.clone(),
                    resolved_locale: detail.session.resolved_locale.clone(),
                    temperature: detail.provider_profile.temperature,
                    max_tokens: detail.provider_profile.max_tokens,
                    error_message: None,
                    pending_approval_id: None,
                    decision_trace: crate::model::AiRunDecisionTrace::default(),
                    metadata: json!({}),
                    created_at: Utc::now(),
                    started_at: Utc::now(),
                    completed_at: None,
                    updated_at: Utc::now(),
                },
                session: detail,
            })
        }
    }

    pub async fn run_task_job(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        input: RunAiTaskJobInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let task_profile =
            require_task_profile(db, operator.tenant_id, input.task_profile_id).await?;
        if !task_profile.is_active {
            return Err(AiError::Validation("task profile is inactive".to_string()));
        }
        enforce_task_permissions(operator, Some(&task_profile))?;
        if input.provider_profile_id.is_some() || input.execution_mode.is_some() {
            ensure_permission(operator, Permission::AI_ROUTER_OVERRIDE)?;
        }

        let resolved_locale = resolve_task_locale(
            db,
            operator.tenant_id,
            operator.preferred_locale.as_deref(),
            input.locale.as_deref(),
            Some(task_profile.slug.as_str()),
        )
        .await?;

        let task_profile_record = map_task_profile(task_profile.clone())?;
        let providers = list_router_provider_profiles(db, operator.tenant_id).await?;
        let execution_plan = AiRouter::resolve(
            Some(&task_profile_runtime(&task_profile_record)),
            &providers,
            input.provider_profile_id,
            task_profile.tool_profile_id,
            &ExecutionOverride {
                execution_mode: input.execution_mode,
                ..ExecutionOverride::default()
            },
            &operator.role_slugs,
        )?;
        let decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
        ai_metrics::observe_locale_resolution(input.locale.as_deref(), resolved_locale.as_str());
        ai_metrics::observe_router_resolution("run_task_job", &decision_trace);

        let txn = db.begin().await.map_err(db_err)?;
        let session = ai_chat_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            title: Set(input.title),
            provider_profile_id: Set(execution_plan.provider_profile_id),
            task_profile_id: Set(Some(task_profile.id)),
            tool_profile_id: Set(execution_plan.tool_profile_id),
            execution_mode: Set(execution_plan.execution_mode.slug().to_string()),
            requested_locale: Set(input.locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            status: Set("active".to_string()),
            created_by: Set(Some(operator.user_id)),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({
                    "decision_trace": decision_trace,
                    "task_input": input.task_input_json,
                    "task_job": true,
                }),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(db_err)?;

        insert_message(
            &txn,
            operator.tenant_id,
            session.id,
            None,
            Some(operator.user_id),
            build_task_job_user_message(
                task_profile.slug.as_str(),
                input.locale.as_deref(),
                resolved_locale.as_str(),
                &input.task_input_json,
            ),
        )
        .await?;

        txn.commit().await.map_err(db_err)?;

        Self::execute_task_job_run(
            runtime,
            operator,
            session.id,
            input.task_input_json,
            input.locale,
            resolved_locale,
        )
        .await
    }

    pub async fn send_chat_message(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        input: SendAiChatMessageInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let session = require_session(db, operator.tenant_id, session_id).await?;
        insert_message(
            db,
            operator.tenant_id,
            session.id,
            None,
            Some(operator.user_id),
            ChatMessage {
                role: ChatMessageRole::User,
                content: Some(input.content),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
                metadata: json!({}),
            },
        )
        .await?;
        Self::execute_latest_turn(runtime, operator, session.id).await
    }

    pub async fn list_chat_sessions(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiChatSessionSummary>> {
        let sessions = ai_chat_sessions::Entity::find()
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .order_by_desc(ai_chat_sessions::Column::UpdatedAt)
            .all(db)
            .await
            .map_err(db_err)?;

        let mut summaries = Vec::with_capacity(sessions.len());
        for session in sessions {
            let latest_run = ai_chat_runs::Entity::find()
                .filter(
                    Condition::all()
                        .add(ai_chat_runs::Column::TenantId.eq(tenant_id))
                        .add(ai_chat_runs::Column::SessionId.eq(session.id)),
                )
                .order_by_desc(ai_chat_runs::Column::CreatedAt)
                .one(db)
                .await
                .map_err(db_err)?;
            let pending_count = ai_approval_requests::Entity::find()
                .filter(
                    Condition::all()
                        .add(ai_approval_requests::Column::TenantId.eq(tenant_id))
                        .add(ai_approval_requests::Column::SessionId.eq(session.id))
                        .add(ai_approval_requests::Column::Status.eq("pending")),
                )
                .count(db)
                .await
                .map_err(db_err)? as usize;
            summaries.push(AiChatSessionSummary {
                id: session.id,
                title: session.title,
                provider_profile_id: session.provider_profile_id,
                task_profile_id: session.task_profile_id,
                tool_profile_id: session.tool_profile_id,
                execution_mode: execution_mode_from_slug(&session.execution_mode)?,
                requested_locale: session.requested_locale,
                resolved_locale: session.resolved_locale,
                status: session.status,
                created_at: to_utc(session.created_at),
                updated_at: to_utc(session.updated_at),
                latest_run_status: latest_run.map(|value| value.status),
                pending_approvals: pending_count,
            });
        }
        Ok(summaries)
    }

    pub async fn chat_session_detail(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> AiResult<Option<AiChatSessionDetail>> {
        let Some(session) = ai_chat_sessions::Entity::find_by_id(session_id)
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
        else {
            return Ok(None);
        };

        let provider = require_provider_profile(db, tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(map_task_profile(
                require_task_profile(db, tenant_id, id).await?,
            )?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(map_tool_profile(
                require_tool_profile(db, tenant_id, id).await?,
            )),
            None => None,
        };
        let messages = ai_chat_messages::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_messages::Column::TenantId.eq(tenant_id))
                    .add(ai_chat_messages::Column::SessionId.eq(session.id)),
            )
            .order_by_asc(ai_chat_messages::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_message_record)
            .collect::<AiResult<Vec<_>>>()?;
        let runs: Vec<_> = ai_chat_runs::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_runs::Column::TenantId.eq(tenant_id))
                    .add(ai_chat_runs::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_chat_runs::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_run_record)
            .collect::<AiResult<Vec<_>>>()?;
        let tool_traces: Vec<_> = ai_tool_traces::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_tool_traces::Column::TenantId.eq(tenant_id))
                    .add(ai_tool_traces::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_tool_traces::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_trace_record)
            .collect();
        let approvals: Vec<_> = ai_approval_requests::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_approval_requests::Column::TenantId.eq(tenant_id))
                    .add(ai_approval_requests::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_approval_requests::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_approval_record)
            .collect();
        let latest_run_status = runs
            .first()
            .map(|value: &AiChatRunRecord| value.status.clone());
        let pending_approvals = approvals
            .iter()
            .filter(|approval| approval.status == "pending")
            .count();

        Ok(Some(AiChatSessionDetail {
            session: AiChatSessionSummary {
                id: session.id,
                title: session.title,
                provider_profile_id: session.provider_profile_id,
                task_profile_id: session.task_profile_id,
                tool_profile_id: session.tool_profile_id,
                execution_mode: execution_mode_from_slug(&session.execution_mode)?,
                requested_locale: session.requested_locale,
                resolved_locale: session.resolved_locale,
                status: session.status,
                created_at: to_utc(session.created_at),
                updated_at: to_utc(session.updated_at),
                latest_run_status,
                pending_approvals,
            },
            provider_profile: map_provider_profile(provider)?,
            task_profile,
            tool_profile,
            messages,
            runs,
            tool_traces,
            approvals,
        }))
    }

    pub async fn list_tool_traces(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        session_id: Option<Uuid>,
        run_id: Option<Uuid>,
    ) -> AiResult<Vec<ToolTrace>> {
        let mut query =
            ai_tool_traces::Entity::find().filter(ai_tool_traces::Column::TenantId.eq(tenant_id));
        if let Some(session_id) = session_id {
            query = query.filter(ai_tool_traces::Column::SessionId.eq(session_id));
        }
        if let Some(run_id) = run_id {
            query = query.filter(ai_tool_traces::Column::RunId.eq(run_id));
        }
        let traces = query
            .order_by_desc(ai_tool_traces::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_trace_record)
            .collect();
        Ok(traces)
    }

    pub async fn resume_approval(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        approval_id: Uuid,
        input: ResumeAiApprovalInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let approval = ai_approval_requests::Entity::find_by_id(approval_id)
            .filter(ai_approval_requests::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("approval request not found".to_string()))?;
        if approval.status != "pending" {
            return Err(AiError::Validation(
                "approval request is not pending".to_string(),
            ));
        }

        let session = require_session(db, operator.tenant_id, approval.session_id).await?;
        let provider =
            require_provider_profile(db, operator.tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(require_task_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(require_tool_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_policy = policy_from_model(tool_profile.as_ref());
        if input.approved && !tool_policy.is_tool_allowed(&approval.tool_name) {
            return Err(AiError::Validation(format!(
                "tool `{}` is no longer allowed by the execution policy",
                approval.tool_name
            )));
        }

        let mut approval_active: ai_approval_requests::ActiveModel = approval.clone().into();
        approval_active.status = Set(if input.approved {
            "approved".to_string()
        } else {
            "rejected".to_string()
        });
        approval_active.reason = Set(input.reason.clone().or(approval.reason.clone()));
        approval_active.resolved_by = Set(Some(operator.user_id));
        approval_active.resolved_at = Set(Some(Utc::now().into()));
        approval_active.updated_at = Set(Utc::now().into());
        approval_active.update(db).await.map_err(db_err)?;

        let run = require_run(db, operator.tenant_id, approval.run_id).await?;
        let mut run_active: ai_chat_runs::ActiveModel = run.clone().into();

        let access_context = access_context_for_operator(operator);
        let (tool_content, tool_metadata, trace) = if input.approved {
            let adapter = InProcessMcpAdapter::new(runtime, access_context)?;
            let started = std::time::Instant::now();
            let tool_result = adapter
                .call_tool(&approval.tool_name, approval.tool_input.clone())
                .await?;
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: approval.tool_input.clone(),
                output_payload: Some(tool_result.raw_payload.clone()),
                status: "completed".to_string(),
                duration_ms: started.elapsed().as_millis() as i64,
                sensitive: tool_policy.is_tool_sensitive(&approval.tool_name),
                error_message: None,
                created_at: Utc::now(),
            };
            (
                tool_result.content,
                json!({ "raw_payload": tool_result.raw_payload, "approval_approved": true }),
                trace,
            )
        } else {
            let content = "Tool execution was rejected by the operator.".to_string();
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: approval.tool_input.clone(),
                output_payload: Some(json!({ "reason": "approval_rejected" })),
                status: "rejected".to_string(),
                duration_ms: 0,
                sensitive: tool_policy.is_tool_sensitive(&approval.tool_name),
                error_message: None,
                created_at: Utc::now(),
            };
            (content, json!({ "approval_rejected": true }), trace)
        };

        insert_tool_trace(db, operator.tenant_id, session.id, run.id, &trace).await?;
        insert_message(
            db,
            operator.tenant_id,
            session.id,
            Some(run.id),
            Some(operator.user_id),
            ChatMessage {
                role: ChatMessageRole::Tool,
                content: Some(tool_content),
                name: Some(approval.tool_name.clone()),
                tool_call_id: Some(approval.tool_call_id.clone()),
                tool_calls: Vec::new(),
                metadata: tool_metadata,
            },
        )
        .await?;

        run_active.status = Set("running".to_string());
        run_active.pending_approval_id = Set(None);
        run_active.updated_at = Set(Utc::now().into());
        run_active.error_message = Set(None);
        run_active.update(db).await.map_err(db_err)?;

        Self::continue_run(
            runtime,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode_from_slug(&session.execution_mode)?,
            session.requested_locale.clone(),
            session.resolved_locale.clone(),
            None,
        )
        .await
    }

    pub async fn cancel_run(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        run_id: Uuid,
    ) -> AiResult<AiChatRunRecord> {
        let run = require_run(db, operator.tenant_id, run_id).await?;
        let mut active: ai_chat_runs::ActiveModel = run.into();
        active.status = Set("cancelled".to_string());
        active.completed_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_run_record(saved)?)
    }

    async fn execute_latest_turn(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let session = require_session(db, operator.tenant_id, session_id).await?;
        let provider =
            require_provider_profile(db, operator.tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(require_task_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(require_tool_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let execution_mode = execution_mode_from_slug(&session.execution_mode)?;
        let requested_locale = session.requested_locale.clone();
        let resolved_locale = session.resolved_locale.clone();

        let run = ai_chat_runs::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            session_id: Set(session.id),
            provider_profile_id: Set(provider.id),
            task_profile_id: Set(task_profile.as_ref().map(|value| value.id)),
            tool_profile_id: Set(tool_profile.as_ref().map(|value| value.id)),
            status: Set("running".to_string()),
            model: Set(provider.model.clone()),
            execution_mode: Set(execution_mode.slug().to_string()),
            execution_path: Set(execution_mode.slug().to_string()),
            requested_locale: Set(requested_locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            temperature: Set(provider.temperature),
            max_tokens: Set(provider.max_tokens),
            error_message: Set(None),
            pending_approval_id: Set(None),
            decision_trace: Set(session
                .metadata
                .get("decision_trace")
                .cloned()
                .unwrap_or_else(|| json!({}))),
            metadata: Set(json!({})),
            created_at: sea_orm::ActiveValue::NotSet,
            started_at: Set(Utc::now().into()),
            completed_at: Set(None),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;

        Self::continue_run(
            runtime,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode,
            requested_locale,
            resolved_locale,
            None,
        )
        .await
    }

    async fn execute_task_job_run(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        task_input_json: serde_json::Value,
        requested_locale: Option<String>,
        resolved_locale: String,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let session = require_session(db, operator.tenant_id, session_id).await?;
        let provider =
            require_provider_profile(db, operator.tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(require_task_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(require_tool_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let execution_mode = execution_mode_from_slug(&session.execution_mode)?;

        let run = ai_chat_runs::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            session_id: Set(session.id),
            provider_profile_id: Set(provider.id),
            task_profile_id: Set(task_profile.as_ref().map(|value| value.id)),
            tool_profile_id: Set(tool_profile.as_ref().map(|value| value.id)),
            status: Set("running".to_string()),
            model: Set(provider.model.clone()),
            execution_mode: Set(execution_mode.slug().to_string()),
            execution_path: Set(execution_mode.slug().to_string()),
            requested_locale: Set(requested_locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            temperature: Set(provider.temperature),
            max_tokens: Set(provider.max_tokens),
            error_message: Set(None),
            pending_approval_id: Set(None),
            decision_trace: Set(session
                .metadata
                .get("decision_trace")
                .cloned()
                .unwrap_or_else(|| json!({}))),
            metadata: Set(json!({ "task_input": task_input_json })),
            created_at: sea_orm::ActiveValue::NotSet,
            started_at: Set(Utc::now().into()),
            completed_at: Set(None),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;

        Self::continue_run(
            runtime,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode,
            requested_locale,
            resolved_locale,
            Some(task_input_json),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn continue_run(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        run_id: Uuid,
        provider_profile: ai_provider_profiles::Model,
        task_profile: Option<ai_task_profiles::Model>,
        tool_profile: Option<ai_tool_profiles::Model>,
        execution_mode: ExecutionMode,
        requested_locale: Option<String>,
        resolved_locale: String,
        task_input_json: Option<serde_json::Value>,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let run_started = std::time::Instant::now();
        let provider_slug = provider_slug_from_str(&provider_profile.provider_slug)?;
        publish_ai_run_stream_event(
            session_id,
            run_id,
            crate::streaming::AiRunStreamEventKind::Started,
            None,
            None,
            None,
        );
        let messages = ai_chat_messages::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_messages::Column::TenantId.eq(operator.tenant_id))
                    .add(ai_chat_messages::Column::SessionId.eq(session_id)),
            )
            .order_by_asc(ai_chat_messages::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_chat_message)
            .collect::<AiResult<Vec<_>>>()?;

        let direct_registry = DirectExecutionRegistry::with_defaults();
        if matches!(execution_mode, ExecutionMode::Direct) {
            if let (Some(task_profile), Some(handler)) = (
                task_profile.as_ref(),
                task_profile
                    .as_ref()
                    .and_then(|profile| direct_registry.handler(&profile.slug)),
            ) {
                let stream_buffer = Arc::new(Mutex::new(String::new()));
                let stream_emitter = ProviderStreamEmitter::new({
                    let stream_buffer = Arc::clone(&stream_buffer);
                    move |event| {
                        let ProviderStreamEvent::TextDelta(delta) = event;
                        let mut accumulated = stream_buffer
                            .lock()
                            .expect("AI stream buffer mutex poisoned");
                        accumulated.push_str(&delta);
                        publish_ai_run_stream_event(
                            session_id,
                            run_id,
                            crate::streaming::AiRunStreamEventKind::Delta,
                            Some(delta),
                            Some(accumulated.clone()),
                            None,
                        );
                    }
                });
                let task_input_json = match task_input_json {
                    Some(task_input_json) => task_input_json,
                    None => session_task_input(db, operator.tenant_id, session_id)
                        .await?
                        .ok_or_else(|| {
                            AiError::Validation(
                                "direct task execution requires task_input_json".to_string(),
                            )
                        })?,
                };
                let provider_config = provider_config(&provider_profile, runtime.provider_targets())?;
                let provider = Arc::<dyn InferenceEngine>::from(
                    inference_for_slug(&provider_slug, &provider_config, runtime.secret_registry())
                        .await?,
                );
                let direct_result = match handler
                    .execute(
                        runtime,
                        operator,
                        DirectExecutionRequest {
                            task_slug: task_profile.slug.clone(),
                            task_input_json,
                            requested_locale: requested_locale.clone(),
                            resolved_locale: resolved_locale.clone(),
                            system_prompt: task_profile.system_prompt.clone(),
                            provider_config: provider_config.clone(),
                            provider,
                            stream_emitter: Some(stream_emitter),
                        },
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        mark_run_failed(db, operator.tenant_id, run_id, error.to_string()).await?;
                        publish_ai_run_stream_event(
                            session_id,
                            run_id,
                            crate::streaming::AiRunStreamEventKind::Failed,
                            None,
                            Some(read_stream_buffer(&stream_buffer)),
                            Some(error.to_string()),
                        );
                        ai_metrics::observe_run_outcome(
                            ExecutionMode::Direct,
                            Some("direct"),
                            &provider_slug,
                            Some(task_profile.slug.as_str()),
                            Some(resolved_locale.as_str()),
                            "failed",
                            run_started.elapsed().as_millis() as u64,
                        );
                        return Err(error);
                    }
                };
                let mut run = require_run(db, operator.tenant_id, run_id).await?;
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    direct_result.appended_messages,
                    direct_result.traces,
                )
                .await?;
                let mut decision_trace: crate::model::AiRunDecisionTrace =
                    serde_json::from_value(run.decision_trace.clone()).unwrap_or_default();
                decision_trace = enrich_decision_trace(
                    decision_trace,
                    ExecutionMode::Direct,
                    requested_locale.clone(),
                    resolved_locale.clone(),
                );
                let execution_target = format!("direct:{}", direct_result.execution_target.slug());
                decision_trace.execution_target = Some(execution_target.clone());
                let run_metadata = run.metadata.clone();
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.execution_path = Set(ExecutionMode::Direct.slug().to_string());
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                active.decision_trace =
                    Set(serde_json::to_value(decision_trace).unwrap_or_else(|_| json!({})));
                active.metadata = Set(merge_metadata(run_metadata, direct_result.metadata));
                active.status = Set("completed".to_string());
                run = active.update(db).await.map_err(db_err)?;
                let detail = Self::chat_session_detail(db, operator.tenant_id, session_id)
                    .await?
                    .ok_or_else(|| {
                        AiError::Runtime("failed to reload AI chat session".to_string())
                    })?;
                ai_metrics::observe_run_outcome(
                    ExecutionMode::Direct,
                    Some(execution_target.as_str()),
                    &provider_slug,
                    Some(task_profile.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "completed",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Completed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    None,
                );
                return Ok(AiSendMessageResult {
                    session: detail,
                    run: map_run_record(run)?,
                });
            }
        }

        let provider_config = provider_config(&provider_profile, runtime.provider_targets())?;
        let provider = Arc::<dyn InferenceEngine>::from(
            inference_for_slug(&provider_slug, &provider_config, runtime.secret_registry()).await?,
        );
        let access_context = access_context_for_operator(operator);
        let adapter = Arc::new(InProcessMcpAdapter::new(runtime, access_context)?);
        let policy = policy_from_model(tool_profile.as_ref());
        let runtime = RigAgentDriver::new(provider, adapter, policy);
        let stream_buffer = Arc::new(Mutex::new(String::new()));
        let stream_emitter = ProviderStreamEmitter::new({
            let stream_buffer = Arc::clone(&stream_buffer);
            move |event| {
                let ProviderStreamEvent::TextDelta(delta) = event;
                let mut accumulated = stream_buffer
                    .lock()
                    .expect("AI stream buffer mutex poisoned");
                accumulated.push_str(&delta);
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Delta,
                    Some(delta),
                    Some(accumulated.clone()),
                    None,
                );
            }
        });
        let outcome = match runtime
            .run(
                &provider_config,
                crate::model::RuntimeRequest {
                    model: provider_profile.model.clone(),
                    messages,
                    temperature: provider_profile.temperature,
                    max_tokens: provider_profile.max_tokens.map(|value| value.max(0) as u32),
                    max_turns: 4,
                    execution_mode,
                    system_prompt: task_profile
                        .as_ref()
                        .and_then(|value| value.system_prompt.clone()),
                    locale: Some(resolved_locale.clone()),
                },
                Some(stream_emitter),
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                mark_run_failed(db, operator.tenant_id, run_id, error.to_string()).await?;
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Failed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    Some(error.to_string()),
                );
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "failed",
                    run_started.elapsed().as_millis() as u64,
                );
                return Err(error);
            }
        };

        let mut run = require_run(db, operator.tenant_id, run_id).await?;

        match outcome {
            RuntimeOutcome::Completed {
                appended_messages,
                traces,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("completed".to_string());
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "completed",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Completed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    None,
                );
            }
            RuntimeOutcome::Failed {
                appended_messages,
                traces,
                error_message,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("failed".to_string());
                active.error_message = Set(Some(error_message));
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "failed",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Failed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    run.error_message.clone(),
                );
            }
            RuntimeOutcome::WaitingApproval {
                appended_messages,
                traces,
                pending_approval,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let approval =
                    insert_approval_request(db, operator, session_id, run_id, &pending_approval)
                        .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("waiting_approval".to_string());
                active.pending_approval_id = Set(Some(approval.id));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "waiting_approval",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::WaitingApproval,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    None,
                );
            }
        }

        let detail = Self::chat_session_detail(db, operator.tenant_id, session_id)
            .await?
            .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
        Ok(AiSendMessageResult {
            session: detail,
            run: map_run_record(run)?,
        })
    }
}

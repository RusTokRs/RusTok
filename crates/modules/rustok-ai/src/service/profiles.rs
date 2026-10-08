use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::engine::inference_for_slug;
use crate::entities::{
    ai_chat_runs, ai_chat_sessions, ai_provider_profiles, ai_structured_budgets,
    ai_structured_provider_policies, ai_task_profiles, ai_tool_profiles,
};
use crate::metrics::{self as ai_metrics, AiRuntimeMetricsSnapshot};
use crate::model::{ProviderCapability, ProviderTestResult};
use crate::streaming::{AiRunStreamEvent, ai_run_stream_hub};
use crate::{AiError, AiResult};
use rustok_api::Permission;

use super::AiManagementService;
use super::helpers::*;
use super::mapping::*;
use super::types::*;

fn ensure_structured_accounting_manage(
    operator: &AiOperatorContext,
) -> Result<(), rustok_api::PortError> {
    if has_effective_permission(operator, Permission::AI_PROVIDERS_MANAGE) {
        Ok(())
    } else {
        Err(rustok_api::PortError::forbidden(
            "ai.structured.accounting_manage_forbidden",
            "structured accounting management requires ai:providers:manage",
        ))
    }
}

fn ensure_structured_accounting_read(
    operator: &AiOperatorContext,
) -> Result<(), rustok_api::PortError> {
    if has_effective_permission(operator, Permission::AI_PROVIDERS_READ) {
        Ok(())
    } else {
        Err(rustok_api::PortError::forbidden(
            "ai.structured.accounting_read_forbidden",
            "structured accounting reads require ai:providers:read",
        ))
    }
}

fn structured_u64(value: i64) -> Result<u64, rustok_api::PortError> {
    u64::try_from(value).map_err(|_| {
        rustok_api::PortError::invariant_violation(
            "ai.structured.accounting_invalid",
            "structured accounting contains invalid evidence",
        )
    })
}

fn structured_u32(value: i32) -> Result<u32, rustok_api::PortError> {
    u32::try_from(value).map_err(|_| {
        rustok_api::PortError::invariant_violation(
            "ai.structured.accounting_invalid",
            "structured accounting contains invalid evidence",
        )
    })
}

fn map_structured_budget_policy(
    model: ai_structured_budgets::Model,
) -> Result<AiStructuredBudgetPolicyRecord, rustok_api::PortError> {
    Ok(AiStructuredBudgetPolicyRecord {
        id: model.id,
        currency_code: model.currency_code,
        limit_minor_units: structured_u64(model.limit_minor_units)?,
        reserved_minor_units: structured_u64(model.reserved_minor_units)?,
        committed_minor_units: structured_u64(model.committed_minor_units)?,
        max_concurrent: structured_u32(model.max_concurrent)?,
        in_flight: structured_u32(model.in_flight)?,
        revision: structured_u64(model.revision)?,
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

fn map_structured_provider_policy(
    model: ai_structured_provider_policies::Model,
) -> Result<AiStructuredProviderPolicyRecord, rustok_api::PortError> {
    Ok(AiStructuredProviderPolicyRecord {
        id: model.id,
        provider_profile_id: model.provider_profile_id,
        allowed_classifications: crate::accounting::provider_allowed_classifications(&model)?,
        currency_code: model.currency_code,
        input_cost_per_million_minor: structured_u64(model.input_cost_per_million_minor)?,
        output_cost_per_million_minor: structured_u64(model.output_cost_per_million_minor)?,
        max_concurrent: structured_u32(model.max_concurrent)?,
        in_flight: structured_u32(model.in_flight)?,
        is_active: model.is_active,
        revision: structured_u64(model.revision)?,
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

fn structured_accounting_unavailable() -> rustok_api::PortError {
    rustok_api::PortError::unavailable(
        "ai.structured.accounting_unavailable",
        "structured execution accounting is unavailable",
    )
}

fn structured_provider_policy_is_eligible(
    policy_active: bool,
    profile_active: bool,
    supports_structured_generation: bool,
) -> bool {
    !policy_active || (profile_active && supports_structured_generation)
}

impl AiManagementService {
    pub async fn put_structured_budget_policy(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: PutAiStructuredBudgetPolicyInput,
    ) -> Result<AiStructuredBudgetPolicyRecord, rustok_api::PortError> {
        ensure_structured_accounting_manage(operator)?;
        let accounting = crate::accounting::StructuredAccounting::new(db.clone());
        accounting
            .put_budget(crate::accounting::BudgetPolicy {
                tenant_id: operator.tenant_id,
                currency_code: input.currency_code.clone(),
                limit_minor_units: input.limit_minor_units,
                max_concurrent: input.max_concurrent,
            })
            .await?;
        let model = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_structured_budgets::Column::CurrencyCode.eq(input.currency_code))
            .one(db)
            .await
            .map_err(|_| structured_accounting_unavailable())?
            .ok_or_else(structured_accounting_unavailable)?;
        map_structured_budget_policy(model)
    }

    pub async fn put_structured_provider_policy(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: PutAiStructuredProviderPolicyInput,
    ) -> Result<AiStructuredProviderPolicyRecord, rustok_api::PortError> {
        ensure_structured_accounting_manage(operator)?;
        let profile = Self::get_provider_profile(db, operator.tenant_id, input.provider_profile_id)
            .await
            .map_err(|_| structured_accounting_unavailable())?
            .ok_or_else(|| {
                rustok_api::PortError::not_found(
                    "ai.structured.provider_profile_not_found",
                    "structured accounting provider profile was not found",
                )
            })?;
        if !structured_provider_policy_is_eligible(
            input.is_active,
            profile.is_active,
            profile
                .capabilities
                .contains(&ProviderCapability::StructuredGeneration),
        ) {
            return Err(rustok_api::PortError::conflict(
                "ai.structured.provider_profile_ineligible",
                "structured accounting requires an active structured-generation provider profile",
            ));
        }
        let accounting = crate::accounting::StructuredAccounting::new(db.clone());
        accounting
            .put_provider_policy(crate::accounting::ProviderPolicy {
                tenant_id: operator.tenant_id,
                provider_profile_id: input.provider_profile_id,
                allowed_classifications: input.allowed_classifications,
                currency_code: input.currency_code,
                input_cost_per_million_minor: input.input_cost_per_million_minor,
                output_cost_per_million_minor: input.output_cost_per_million_minor,
                max_concurrent: input.max_concurrent,
                is_active: input.is_active,
            })
            .await?;
        let model = ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(operator.tenant_id))
            .filter(
                ai_structured_provider_policies::Column::ProviderProfileId
                    .eq(input.provider_profile_id),
            )
            .one(db)
            .await
            .map_err(|_| structured_accounting_unavailable())?
            .ok_or_else(structured_accounting_unavailable)?;
        map_structured_provider_policy(model)
    }

    pub async fn list_structured_budget_policies(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
    ) -> Result<Vec<AiStructuredBudgetPolicyRecord>, rustok_api::PortError> {
        ensure_structured_accounting_read(operator)?;
        ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(operator.tenant_id))
            .order_by_asc(ai_structured_budgets::Column::CurrencyCode)
            .all(db)
            .await
            .map_err(|_| structured_accounting_unavailable())?
            .into_iter()
            .map(map_structured_budget_policy)
            .collect()
    }

    pub async fn list_structured_provider_policies(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
    ) -> Result<Vec<AiStructuredProviderPolicyRecord>, rustok_api::PortError> {
        ensure_structured_accounting_read(operator)?;
        ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(operator.tenant_id))
            .order_by_asc(ai_structured_provider_policies::Column::ProviderProfileId)
            .all(db)
            .await
            .map_err(|_| structured_accounting_unavailable())?
            .into_iter()
            .map(map_structured_provider_policy)
            .collect()
    }

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
        for reference in input.credential_refs.values() {
            secrets
                .validate_reference_for_tenant(operator.tenant_id, reference)
                .map_err(|error| AiError::Validation(error.to_string()))?;
        }
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
            // Role restrictions are platform-owned. Do not accept a package-local
            // role vocabulary through the AI service input.
            restricted_role_slugs: Set(serde_json::json!([])),
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
        for reference in input.credential_refs.values() {
            secrets
                .validate_reference_for_tenant(operator.tenant_id, reference)
                .map_err(|error| AiError::Validation(error.to_string()))?;
        }
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
        // Updating a profile cannot introduce package-local role restrictions.
        active.restricted_role_slugs = Set(serde_json::json!([]));
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
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<ProviderTestResult> {
        let profile = require_provider_profile(db, tenant_id, id).await?;
        secrets.invalidate(None).await;
        let config = provider_config(&profile, provider_targets, egress_policy)?;
        if crate::provider_factory_supports(&config.provider_slug, crate::ProviderFeature::Chat) {
            let provider = inference_for_slug(&config.provider_slug, &config, secrets).await?;
            return provider.test_connection(&config).await;
        }
        let started = std::time::Instant::now();
        if crate::provider_factory_supports(
            &config.provider_slug,
            crate::ProviderFeature::Embeddings,
        ) {
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
        } else if crate::provider_factory_supports(
            &config.provider_slug,
            crate::ProviderFeature::Rerank,
        ) {
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
}

#[cfg(test)]
mod structured_accounting_management_tests {
    use super::{
        AiOperatorContext, ensure_structured_accounting_manage, ensure_structured_accounting_read,
        structured_provider_policy_is_eligible,
    };
    use rustok_api::Permission;
    use uuid::Uuid;

    fn operator(permissions: Vec<Permission>) -> AiOperatorContext {
        AiOperatorContext {
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            permissions,
            role_slugs: Vec::new(),
            preferred_locale: None,
        }
    }

    #[test]
    fn structured_accounting_requires_provider_permissions_at_the_service_boundary() {
        let reader = operator(vec![Permission::AI_PROVIDERS_READ]);
        assert!(ensure_structured_accounting_read(&reader).is_ok());
        assert_eq!(
            ensure_structured_accounting_manage(&reader)
                .expect_err("read permission must not mutate accounting")
                .code,
            "ai.structured.accounting_manage_forbidden"
        );

        let manager = operator(vec![Permission::AI_PROVIDERS_MANAGE]);
        assert!(ensure_structured_accounting_manage(&manager).is_ok());
        assert_eq!(
            ensure_structured_accounting_read(&operator(Vec::new()))
                .expect_err("anonymous accounting reads must fail closed")
                .code,
            "ai.structured.accounting_read_forbidden"
        );
    }

    #[test]
    fn inactive_provider_policy_can_always_be_persisted_for_shutdown() {
        assert!(structured_provider_policy_is_eligible(false, false, false));
        assert!(!structured_provider_policy_is_eligible(true, false, true));
        assert!(!structured_provider_policy_is_eligible(true, true, false));
        assert!(structured_provider_policy_is_eligible(true, true, true));
    }
}

use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::AiGraphqlRoleSlugProviderHandle;
use rustok_api::graphql::GraphQLError;
use rustok_api::{AuthContext, RequestContext};

use super::{
    ensure_ai_approval_resolve, ensure_ai_provider_manage, ensure_ai_run_cancel,
    ensure_ai_session_run, ensure_ai_task_profile_manage,
    types::{
        parse_metadata, AiChatRunGql, AiProviderProfileGql, AiProviderTestResultGql,
        AiSendMessageResultGql, AiTaskProfileGql, AiToolProfileGql,
        CreateAiProviderProfileInputGql, CreateAiTaskProfileInputGql, CreateAiToolProfileInputGql,
        ResumeAiApprovalInputGql, RunAiTaskJobInputGql, StartAiChatSessionInputGql,
        UpdateAiProviderProfileInputGql, UpdateAiTaskProfileInputGql, UpdateAiToolProfileInputGql,
    },
};

#[derive(Default)]
pub struct AiMutation;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

async fn operator_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
) -> Result<crate::AiOperatorContext> {
    let preferred_locale = ctx
        .data_opt::<RequestContext>()
        .map(|request_context| request_context.locale.clone());
    Ok(crate::AiOperatorContext {
        tenant_id: auth.tenant_id,
        user_id: auth.user_id,
        permissions: auth.permissions.clone(),
        role_slugs: ctx
            .data::<AiGraphqlRoleSlugProviderHandle>()?
            .load_role_slugs(auth.tenant_id, auth.user_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?,
        preferred_locale,
    })
}

#[Object]
impl AiMutation {
    async fn create_ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        input: CreateAiProviderProfileInputGql,
    ) -> Result<AiProviderProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let provider_target_id =
            crate::ProviderTargetId::new(input.provider_target_id).map_err(async_graphql::Error::new)?;
        let provider_slug = runtime
            .provider_targets()
            .get(&provider_target_id)
            .ok_or_else(|| async_graphql::Error::new("unknown deployment provider target"))?
            .provider_slug
            .clone();
        let capabilities = if input.capabilities.is_empty() {
            default_capabilities_for_provider(&provider_slug)
        } else {
            input.capabilities.into_iter().map(Into::into).collect()
        };
        let credential_refs = credential_refs(input.credential_refs)?;
        let item = crate::AiManagementService::create_provider_profile(
            db,
            &operator,
            runtime.provider_targets(),
            runtime.egress_policy(),
            runtime.secret_registry(),
            crate::CreateAiProviderProfileInput {
                slug: input.slug,
                display_name: input.display_name,
                provider_target_id,
                model: input.model,
                credential_refs,
                temperature: input.temperature,
                max_tokens: input.max_tokens,
                capabilities,
                usage_policy: input.usage_policy.into(),
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn update_ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateAiProviderProfileInputGql,
    ) -> Result<AiProviderProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let capabilities = input.capabilities.into_iter().map(Into::into).collect();
        let provider_target_id =
            crate::ProviderTargetId::new(input.provider_target_id).map_err(async_graphql::Error::new)?;
        let credential_refs = credential_refs(input.credential_refs)?;
        let item = crate::AiManagementService::update_provider_profile(
            db,
            &operator,
            runtime.provider_targets(),
            runtime.egress_policy(),
            runtime.secret_registry(),
            id,
            crate::UpdateAiProviderProfileInput {
                display_name: input.display_name,
                provider_target_id,
                model: input.model,
                credential_refs,
                temperature: input.temperature,
                max_tokens: input.max_tokens,
                capabilities,
                usage_policy: input.usage_policy.into(),
                metadata: parse_metadata(input.metadata)?,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn test_ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<AiProviderTestResultGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let item = crate::AiManagementService::test_provider_profile(
            db,
            runtime.provider_targets(),
            runtime.egress_policy(),
            runtime.secret_registry(),
            auth.tenant_id,
            id,
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn deactivate_ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<AiProviderProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::deactivate_provider_profile(db, &operator, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn create_ai_tool_profile(
        &self,
        ctx: &Context<'_>,
        input: CreateAiToolProfileInputGql,
    ) -> Result<AiToolProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::create_tool_profile(
            db,
            &operator,
            crate::CreateAiToolProfileInput {
                slug: input.slug,
                display_name: input.display_name,
                description: input.description,
                allowed_tools: input.allowed_tools,
                denied_tools: input.denied_tools,
                sensitive_tools: input.sensitive_tools,
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn create_ai_task_profile(
        &self,
        ctx: &Context<'_>,
        input: CreateAiTaskProfileInputGql,
    ) -> Result<AiTaskProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::create_task_profile(
            db,
            &operator,
            crate::CreateAiTaskProfileInput {
                slug: input.slug,
                display_name: input.display_name,
                description: input.description,
                target_capability: input.target_capability.into(),
                system_prompt: input.system_prompt,
                allowed_provider_profile_ids: input.allowed_provider_profile_ids,
                preferred_provider_profile_ids: input.preferred_provider_profile_ids,
                fallback_strategy: input
                    .fallback_strategy
                    .unwrap_or_else(|| "ordered".to_string()),
                tool_profile_id: input.tool_profile_id,
                approval_policy: serde_json::json!({}),
                default_execution_mode: input.default_execution_mode.into(),
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn update_ai_tool_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateAiToolProfileInputGql,
    ) -> Result<AiToolProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::update_tool_profile(
            db,
            &operator,
            id,
            crate::UpdateAiToolProfileInput {
                display_name: input.display_name,
                description: input.description,
                allowed_tools: input.allowed_tools,
                denied_tools: input.denied_tools,
                sensitive_tools: input.sensitive_tools,
                metadata: parse_metadata(input.metadata)?,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn update_ai_task_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateAiTaskProfileInputGql,
    ) -> Result<AiTaskProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::update_task_profile(
            db,
            &operator,
            id,
            crate::UpdateAiTaskProfileInput {
                display_name: input.display_name,
                description: input.description,
                target_capability: input.target_capability.into(),
                system_prompt: input.system_prompt,
                allowed_provider_profile_ids: input.allowed_provider_profile_ids,
                preferred_provider_profile_ids: input.preferred_provider_profile_ids,
                fallback_strategy: input
                    .fallback_strategy
                    .unwrap_or_else(|| "ordered".to_string()),
                tool_profile_id: input.tool_profile_id,
                approval_policy: serde_json::json!({}),
                default_execution_mode: input.default_execution_mode.into(),
                metadata: parse_metadata(input.metadata)?,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn start_ai_chat_session(
        &self,
        ctx: &Context<'_>,
        input: StartAiChatSessionInputGql,
    ) -> Result<AiSendMessageResultGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_run(auth)?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let _db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::start_chat_session(
            runtime,
            &operator,
            crate::StartAiChatSessionInput {
                title: input.title,
                provider_profile_id: input.provider_profile_id,
                task_profile_id: input.task_profile_id,
                tool_profile_id: input.tool_profile_id,
                execution_mode: None,
                override_config: crate::ExecutionOverride::default(),
                locale: input.locale,
                initial_message: input.initial_message,
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(AiSendMessageResultGql {
            session: item.session.try_into()?,
            run: item.run.into(),
        })
    }

    async fn send_ai_chat_message(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        content: String,
    ) -> Result<AiSendMessageResultGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_run(auth)?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let _db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::send_chat_message(
            runtime,
            &operator,
            session_id,
            crate::SendAiChatMessageInput { content },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(AiSendMessageResultGql {
            session: item.session.try_into()?,
            run: item.run.into(),
        })
    }

    async fn resume_ai_approval(
        &self,
        ctx: &Context<'_>,
        approval_id: Uuid,
        input: ResumeAiApprovalInputGql,
    ) -> Result<AiSendMessageResultGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_approval_resolve(auth)?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let _db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let item = crate::AiManagementService::resume_approval(
            runtime,
            &operator,
            approval_id,
            crate::ResumeAiApprovalInput {
                approved: input.approved,
                reason: input.reason,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(AiSendMessageResultGql {
            session: item.session.try_into()?,
            run: item.run.into(),
        })
    }

    async fn cancel_ai_run(&self, ctx: &Context<'_>, run_id: Uuid) -> Result<AiChatRunGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_run_cancel(auth)?;
        let operator = operator_context(ctx, auth).await?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let item = crate::AiManagementService::cancel_run(runtime, &operator, run_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.into())
    }

    async fn run_ai_task_job(
        &self,
        ctx: &Context<'_>,
        input: RunAiTaskJobInputGql,
    ) -> Result<AiSendMessageResultGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_run(auth)?;
        let runtime = ctx.data::<crate::AiHostRuntime>()?;
        let _db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let task_input_json = serde_json::from_str(&input.task_input_json)
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        let item = crate::AiManagementService::run_task_job(
            runtime,
            &operator,
            crate::RunAiTaskJobInput {
                title: input.title,
                provider_profile_id: input.provider_profile_id,
                task_profile_id: input.task_profile_id,
                execution_mode: input.execution_mode.map(Into::into),
                locale: input.locale,
                task_input_json,
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(AiSendMessageResultGql {
            session: item.session.try_into()?,
            run: item.run.into(),
        })
    }
}

fn provider_settings(
    inputs: Vec<super::types::AiProviderSettingInputGql>,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let mut settings = BTreeMap::new();
    for input in inputs {
        let values = [
            input.text_value.map(serde_json::Value::String),
            input.integer_value.map(serde_json::Value::from),
            input.boolean_value.map(serde_json::Value::from),
        ];
        let mut values = values.into_iter().flatten();
        let value = values.next().ok_or_else(|| {
            async_graphql::Error::new(format!("setting `{}` has no typed value", input.key))
        })?;
        if values.next().is_some() {
            return Err(async_graphql::Error::new(format!(
                "setting `{}` has more than one typed value",
                input.key
            )));
        }
        if settings.insert(input.key.clone(), value).is_some() {
            return Err(async_graphql::Error::new(format!(
                "setting `{}` is duplicated",
                input.key
            )));
        }
    }
    Ok(settings)
}

fn credential_refs(
    inputs: Vec<super::types::AiCredentialRefInputGql>,
) -> Result<BTreeMap<String, rustok_secrets::SecretRef>> {
    let mut refs = BTreeMap::new();
    for input in inputs {
        let reference = rustok_secrets::SecretRef {
            resolver: input.resolver,
            key: input.secret_key,
        };
        if refs.insert(input.key.clone(), reference).is_some() {
            return Err(async_graphql::Error::new(format!(
                "credential `{}` is duplicated",
                input.key
            )));
        }
    }
    Ok(refs)
}

fn default_capabilities_for_provider(
    provider_slug: &crate::ProviderSlug,
) -> Vec<crate::ProviderCapability> {
    let Some(descriptor) = crate::provider_catalog_entry(provider_slug) else {
        return Vec::new();
    };
    let mut capabilities = Vec::new();
    for feature in descriptor.features {
        let capability = match feature {
            crate::ProviderFeature::Chat => Some(crate::ProviderCapability::TextGeneration),
            crate::ProviderFeature::StructuredOutput => {
                Some(crate::ProviderCapability::StructuredGeneration)
            }
            crate::ProviderFeature::Image => Some(crate::ProviderCapability::ImageGeneration),
            crate::ProviderFeature::Multimodal => {
                Some(crate::ProviderCapability::MultimodalUnderstanding)
            }
            _ => None,
        };
        if let Some(capability) = capability.filter(|value| !capabilities.contains(value)) {
            capabilities.push(capability);
        }
    }
    capabilities
}

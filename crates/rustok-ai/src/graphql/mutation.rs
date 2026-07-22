use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use uuid::Uuid;

use rustok_api::graphql::GraphQLError;
use rustok_api::{AuthContext, RequestContext};

use super::{
    ensure_ai_approval_resolve, ensure_ai_provider_manage, ensure_ai_run_cancel,
    ensure_ai_session_run, ensure_ai_task_profile_manage,
    types::{
        AiAgentModelAssignmentGql, AiAgentPrincipalGql, AiChatRunGql, AiProviderProfileGql,
        AiProviderTestResultGql, AiSendMessageResultGql, AiTaskProfileGql, AiToolProfileGql,
        CreateAiAgentModelAssignmentInputGql, CreateAiAgentPrincipalInputGql,
        CreateAiAgentWorkflowRunInputGql, CreateAiProviderProfileInputGql,
        CreateAiTaskProfileInputGql, CreateAiToolProfileInputGql,
        ResolveAiAgentWorkflowStageApprovalInputGql, ResumeAiApprovalInputGql,
        RunAiTaskJobInputGql, StartAiChatSessionInputGql, UpdateAiAgentModelAssignmentInputGql,
        UpdateAiAgentPrincipalInputGql, UpdateAiProviderProfileInputGql,
        UpdateAiTaskProfileInputGql, UpdateAiToolProfileInputGql, parse_metadata,
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
        // Provider role restrictions are fail-closed until the platform-owned
        // TenantRbacCatalog is available through the generic host context.
        role_slugs: Vec::new(),
        preferred_locale,
    })
}

#[Object]
impl AiMutation {
    async fn create_ai_agent_principal(
        &self,
        ctx: &Context<'_>,
        input: CreateAiAgentPrincipalInputGql,
    ) -> Result<AiAgentPrincipalGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let tenant_rbac_catalog = ctx
            .data::<crate::AiGraphqlRuntimeData>()?
            .tenant_rbac_catalog()
            .ok_or_else(|| async_graphql::Error::new("tenant RBAC catalog is unavailable"))?
            .0
            .clone();
        Ok(crate::AiManagementService::create_agent_principal(
            db,
            &operator,
            tenant_rbac_catalog.as_ref(),
            crate::CreateAiAgentPrincipalInput {
                slug: input.slug,
                descriptor_owner: input.descriptor_owner,
                descriptor_slug: input.descriptor_slug,
                role_slugs: input.role_slugs,
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .into())
    }

    async fn create_ai_agent_model_assignment(
        &self,
        ctx: &Context<'_>,
        input: CreateAiAgentModelAssignmentInputGql,
    ) -> Result<AiAgentModelAssignmentGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        Ok(crate::AiManagementService::create_agent_model_assignment(
            db,
            &operator,
            crate::CreateAiAgentModelAssignmentInput {
                agent_principal_id: input.agent_principal_id,
                provider_profile_id: input.provider_profile_id,
                model_override: input.model_override,
                execution_mode: input.execution_mode.into(),
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .into())
    }

    async fn update_ai_agent_principal(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateAiAgentPrincipalInputGql,
    ) -> Result<AiAgentPrincipalGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let tenant_rbac_catalog = ctx
            .data::<crate::AiGraphqlRuntimeData>()?
            .tenant_rbac_catalog()
            .ok_or_else(|| async_graphql::Error::new("tenant RBAC catalog is unavailable"))?
            .0
            .clone();
        Ok(crate::AiManagementService::update_agent_principal(
            db,
            &operator,
            tenant_rbac_catalog.as_ref(),
            id,
            crate::UpdateAiAgentPrincipalInput {
                role_slugs: input.role_slugs,
                metadata: parse_metadata(input.metadata)?,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .into())
    }

    async fn update_ai_agent_model_assignment(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateAiAgentModelAssignmentInputGql,
    ) -> Result<AiAgentModelAssignmentGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        Ok(crate::AiManagementService::update_agent_model_assignment(
            db,
            &operator,
            id,
            crate::UpdateAiAgentModelAssignmentInput {
                model_override: input.model_override,
                execution_mode: input.execution_mode.into(),
                metadata: parse_metadata(input.metadata)?,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .into())
    }

    async fn create_ai_agent_workflow_run(
        &self,
        ctx: &Context<'_>,
        input: CreateAiAgentWorkflowRunInputGql,
    ) -> Result<Uuid> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_run(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let mut stage_principal_ids = BTreeMap::new();
        let mut stage_model_assignment_ids = BTreeMap::new();
        let mut stage_input_payloads = BTreeMap::new();
        for binding in input.stage_bindings {
            let stage_id = binding.stage_id;
            if stage_principal_ids
                .insert(stage_id.clone(), binding.agent_principal_id)
                .is_some()
                || stage_model_assignment_ids
                    .insert(stage_id.clone(), binding.model_assignment_id)
                    .is_some()
            {
                return Err(async_graphql::Error::new(
                    "agent workflow stage bindings must be unique",
                ));
            }
            let payload = parse_metadata(Some(binding.input_payload))?;
            if stage_input_payloads.insert(stage_id, payload).is_some() {
                return Err(async_graphql::Error::new(
                    "agent workflow stage bindings must be unique",
                ));
            }
        }
        crate::AiManagementService::create_agent_workflow_run(
            db,
            &operator,
            crate::CreateAiAgentWorkflowRunInput {
                workflow_owner: input.workflow_owner,
                workflow_slug: input.workflow_slug,
                stage_principal_ids,
                stage_model_assignment_ids,
                stage_input_payloads,
                input_payload: parse_metadata(Some(input.input_payload))?,
                metadata: parse_metadata(input.metadata)?,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))
    }

    async fn create_ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        input: CreateAiProviderProfileInputGql,
    ) -> Result<AiProviderProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
        let provider_target_id = crate::ProviderTargetId::new(input.provider_target_id)
            .map_err(async_graphql::Error::new)?;
        let provider_slug = runtime
            .provider_targets()
            .get(&provider_target_id)
            .ok_or_else(|| async_graphql::Error::new("unknown deployment provider target"))?
            .provider_slug
            .clone();
        let capabilities = if input.capabilities.is_empty() {
            crate::default_provider_capabilities(&provider_slug)
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
        let capabilities = input.capabilities.into_iter().map(Into::into).collect();
        let provider_target_id = crate::ProviderTargetId::new(input.provider_target_id)
            .map_err(async_graphql::Error::new)?;
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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

    async fn resolve_ai_agent_workflow_stage_approval(
        &self,
        ctx: &Context<'_>,
        stage_id: Uuid,
        input: ResolveAiAgentWorkflowStageApprovalInputGql,
    ) -> Result<bool> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_approval_resolve(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, auth).await?;
        crate::AiManagementService::resolve_agent_workflow_stage_approval(
            db,
            &operator,
            stage_id,
            crate::ResolveAiAgentWorkflowStageApprovalInput {
                approved: input.approved,
                reason: input.reason,
            },
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn cancel_ai_run(&self, ctx: &Context<'_>, run_id: Uuid) -> Result<AiChatRunGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_run_cancel(auth)?;
        let operator = operator_context(ctx, auth).await?;
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
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
                model_override: input.model_override,
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

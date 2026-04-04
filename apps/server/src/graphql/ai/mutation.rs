use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::common::RequestContext;
use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::models::_entities::{roles, user_roles};

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

async fn load_role_slugs(db: &DatabaseConnection, auth: &AuthContext) -> Result<Vec<String>> {
    let assignments = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(auth.user_id))
        .all(db)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
    if assignments.is_empty() {
        return Ok(Vec::new());
    }
    roles::Entity::find()
        .filter(roles::Column::TenantId.eq(auth.tenant_id))
        .filter(roles::Column::Id.is_in(assignments.into_iter().map(|item| item.role_id)))
        .all(db)
        .await
        .map(|items| items.into_iter().map(|item| item.slug).collect())
        .map_err(|err| async_graphql::Error::new(err.to_string()))
}

async fn operator_context(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    auth: &AuthContext,
) -> Result<rustok_ai::AiOperatorContext> {
    let preferred_locale = ctx
        .data_opt::<RequestContext>()
        .map(|request_context| request_context.locale.clone());
    Ok(rustok_ai::AiOperatorContext {
        tenant_id: auth.tenant_id,
        user_id: auth.user_id,
        permissions: auth.permissions.clone(),
        role_slugs: load_role_slugs(db, auth).await?,
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
        let operator = operator_context(ctx, db, auth).await?;
        let provider_kind: rustok_ai::ProviderKind = input.provider_kind.into();
        let capabilities = if input.capabilities.is_empty() {
            default_capabilities_for_kind(provider_kind)
        } else {
            input.capabilities.into_iter().map(Into::into).collect()
        };
        let item = rustok_ai::AiManagementService::create_provider_profile(
            db,
            &operator,
            rustok_ai::CreateAiProviderProfileInput {
                slug: input.slug,
                display_name: input.display_name,
                provider_kind,
                base_url: input.base_url,
                model: input.model,
                api_key_secret: input.api_key_secret,
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
        let operator = operator_context(ctx, db, auth).await?;
        let capabilities = input.capabilities.into_iter().map(Into::into).collect();
        let item = rustok_ai::AiManagementService::update_provider_profile(
            db,
            &operator,
            id,
            rustok_ai::UpdateAiProviderProfileInput {
                display_name: input.display_name,
                base_url: input.base_url,
                model: input.model,
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

    async fn rotate_ai_provider_secret(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        secret: Option<String>,
    ) -> Result<AiProviderProfileGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_manage(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let item =
            rustok_ai::AiManagementService::rotate_provider_secret(db, &operator, id, secret)
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
        let item = rustok_ai::AiManagementService::test_provider_profile(db, auth.tenant_id, id)
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
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::deactivate_provider_profile(db, &operator, id)
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
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::create_tool_profile(
            db,
            &operator,
            rustok_ai::CreateAiToolProfileInput {
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
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::create_task_profile(
            db,
            &operator,
            rustok_ai::CreateAiTaskProfileInput {
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
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::update_tool_profile(
            db,
            &operator,
            id,
            rustok_ai::UpdateAiToolProfileInput {
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
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::update_task_profile(
            db,
            &operator,
            id,
            rustok_ai::UpdateAiTaskProfileInput {
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
        let app_ctx = ctx.data::<AppContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::start_chat_session(
            app_ctx,
            &operator,
            rustok_ai::StartAiChatSessionInput {
                title: input.title,
                provider_profile_id: input.provider_profile_id,
                task_profile_id: input.task_profile_id,
                tool_profile_id: input.tool_profile_id,
                execution_mode: None,
                override_config: rustok_ai::ExecutionOverride::default(),
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
        let app_ctx = ctx.data::<AppContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::send_chat_message(
            app_ctx,
            &operator,
            session_id,
            rustok_ai::SendAiChatMessageInput { content },
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
        let app_ctx = ctx.data::<AppContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::resume_approval(
            app_ctx,
            &operator,
            approval_id,
            rustok_ai::ResumeAiApprovalInput {
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
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let item = rustok_ai::AiManagementService::cancel_run(db, &operator, run_id)
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
        let app_ctx = ctx.data::<AppContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let operator = operator_context(ctx, db, auth).await?;
        let task_input_json = serde_json::from_str(&input.task_input_json)
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        let item = rustok_ai::AiManagementService::run_task_job(
            app_ctx,
            &operator,
            rustok_ai::RunAiTaskJobInput {
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

fn default_capabilities_for_kind(
    provider_kind: rustok_ai::ProviderKind,
) -> Vec<rustok_ai::ProviderCapability> {
    match provider_kind {
        rustok_ai::ProviderKind::OpenAiCompatible => vec![
            rustok_ai::ProviderCapability::TextGeneration,
            rustok_ai::ProviderCapability::StructuredGeneration,
            rustok_ai::ProviderCapability::ImageGeneration,
            rustok_ai::ProviderCapability::CodeGeneration,
        ],
        rustok_ai::ProviderKind::Anthropic => vec![
            rustok_ai::ProviderCapability::TextGeneration,
            rustok_ai::ProviderCapability::CodeGeneration,
            rustok_ai::ProviderCapability::AlloyAssist,
        ],
        rustok_ai::ProviderKind::Gemini => vec![
            rustok_ai::ProviderCapability::TextGeneration,
            rustok_ai::ProviderCapability::ImageGeneration,
            rustok_ai::ProviderCapability::MultimodalUnderstanding,
        ],
    }
}

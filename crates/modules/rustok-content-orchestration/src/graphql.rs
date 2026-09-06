use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_content::ContentError;
use uuid::Uuid;

use crate::{SharedContentOrchestrationService, content_orchestration_from_shared};

#[derive(Default)]
pub struct ContentOrchestrationMutation;

#[Object]
impl ContentOrchestrationMutation {
    async fn promote_topic_to_post(
        &self,
        ctx: &Context<'_>,
        input: PromoteTopicToPostInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_modules(ctx, &["content", "blog", "forum", "comments"]).await?;
        let (auth, tenant, orchestration) = request_context(ctx)?;
        let result = content_orchestration_from_shared(orchestration)
            .promote_topic_to_post(
                tenant.id,
                security_context(auth),
                rustok_content::PromoteTopicToPostInput {
                    topic_id: input.topic_id,
                    locale: input.locale,
                    blog_category_id: input.blog_category_id,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;
        Ok(result.into())
    }

    async fn demote_post_to_topic(
        &self,
        ctx: &Context<'_>,
        input: DemotePostToTopicInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_modules(ctx, &["content", "blog", "forum", "comments"]).await?;
        let (auth, tenant, orchestration) = request_context(ctx)?;
        let result = content_orchestration_from_shared(orchestration)
            .demote_post_to_topic(
                tenant.id,
                security_context(auth),
                rustok_content::DemotePostToTopicInput {
                    post_id: input.post_id,
                    locale: input.locale,
                    forum_category_id: input.forum_category_id,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;
        Ok(result.into())
    }

    async fn split_topic(
        &self,
        ctx: &Context<'_>,
        input: SplitTopicInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_modules(ctx, &["content", "forum"]).await?;
        let (auth, tenant, orchestration) = request_context(ctx)?;
        let result = content_orchestration_from_shared(orchestration)
            .split_topic(
                tenant.id,
                security_context(auth),
                rustok_content::SplitTopicInput {
                    topic_id: input.topic_id,
                    locale: input.locale,
                    reply_ids: input.reply_ids,
                    new_title: input.new_title,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;
        Ok(result.into())
    }

    async fn merge_topics(
        &self,
        ctx: &Context<'_>,
        input: MergeTopicsInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_modules(ctx, &["content", "forum"]).await?;
        let (auth, tenant, orchestration) = request_context(ctx)?;
        let result = content_orchestration_from_shared(orchestration)
            .merge_topics(
                tenant.id,
                security_context(auth),
                rustok_content::MergeTopicsInput {
                    target_topic_id: input.target_topic_id,
                    source_topic_ids: input.source_topic_ids,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;
        Ok(result.into())
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct PromoteTopicToPostInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub blog_category_id: Option<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(InputObject, Debug, Clone)]
pub struct DemotePostToTopicInput {
    pub post_id: Uuid,
    pub locale: String,
    pub forum_category_id: Uuid,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(InputObject, Debug, Clone)]
pub struct SplitTopicInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub reply_ids: Vec<Uuid>,
    pub new_title: String,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(InputObject, Debug, Clone)]
pub struct MergeTopicsInput {
    pub target_topic_id: Uuid,
    pub source_topic_ids: Vec<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct ContentOrchestrationPayload {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub moved_comments: u64,
}

impl From<rustok_content::OrchestrationResult> for ContentOrchestrationPayload {
    fn from(value: rustok_content::OrchestrationResult) -> Self {
        Self {
            source_id: value.source_id,
            target_id: value.target_id,
            moved_comments: value.moved_comments,
        }
    }
}

async fn require_modules(ctx: &Context<'_>, slugs: &[&str]) -> Result<()> {
    for slug in slugs {
        require_module_enabled(ctx, slug).await?;
    }
    Ok(())
}

fn request_context<'a>(
    ctx: &'a Context<'_>,
) -> Result<(
    &'a AuthContext,
    &'a TenantContext,
    &'a SharedContentOrchestrationService,
)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    Ok((
        auth,
        ctx.data::<TenantContext>()?,
        ctx.data::<SharedContentOrchestrationService>()?,
    ))
}

fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn map_content_error(err: ContentError) -> FieldError {
    match err {
        ContentError::Validation(message) | ContentError::Forbidden(message) => {
            FieldError::new(message)
        }
        ContentError::NodeNotFound(_)
        | ContentError::CategoryNotFound(_)
        | ContentError::TranslationNotFound { .. }
        | ContentError::DuplicateSlug { .. }
        | ContentError::ConcurrentModification { .. } => FieldError::new(err.to_string()),
        ContentError::Database(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        ContentError::Core(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        ContentError::Rich(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
    }
}

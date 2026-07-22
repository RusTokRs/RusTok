use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_core::CONTENT_FORMAT_MARKDOWN;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{ProfileService, ProfilesReader, graphql::GqlProfileSummary};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ForumQuoteReferenceInput,
    ForumQuoteTargetKindInput, ReplyResponse, ReplyService, TopicResponse, TopicService,
    UpdateReplyCommandInput, UpdateTopicCommandInput,
};

use super::{GqlForumQuoteReferenceInput, GqlForumQuoteTargetKind, GqlForumReply, GqlForumTopic};

const MODULE_SLUG: &str = "forum";

#[derive(InputObject)]
pub struct CreateForumTopicWithQuotesInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub metadata: Option<Value>,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
    #[graphql(default)]
    pub quotes: Vec<GqlForumQuoteReferenceInput>,
}

#[derive(InputObject)]
pub struct UpdateForumTopicWithQuotesInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
    pub quotes: Option<Vec<GqlForumQuoteReferenceInput>>,
}

#[derive(InputObject)]
pub struct CreateForumReplyWithQuotesInput {
    pub locale: String,
    pub content: String,
    pub content_format: Option<String>,
    pub content_json: Option<Value>,
    pub parent_reply_id: Option<Uuid>,
    #[graphql(default)]
    pub quotes: Vec<GqlForumQuoteReferenceInput>,
}

#[derive(InputObject)]
pub struct UpdateForumReplyWithQuotesInput {
    pub locale: String,
    pub content: Option<String>,
    pub content_format: Option<String>,
    pub content_json: Option<Value>,
    pub quotes: Option<Vec<GqlForumQuoteReferenceInput>>,
}

#[derive(Default)]
pub struct ForumContentCommandMutation;

#[Object]
impl ForumContentCommandMutation {
    async fn create_forum_topic_with_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateForumTopicWithQuotesInput,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_TOPICS_CREATE,
            "Permission denied: forum_topics:create required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create_command(
                tenant_id,
                security(&auth),
                CreateTopicCommandInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;
        let author_profile = load_author_profile(db, tenant_id, topic.author_id, &topic.effective_locale).await?;
        Ok(map_topic(topic, author_profile))
    }

    async fn update_forum_topic_with_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: UpdateForumTopicWithQuotesInput,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_TOPICS_UPDATE,
            "Permission denied: forum_topics:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let topic = TopicService::new(db.clone(), event_bus.clone())
            .update_command(
                tenant_id,
                topic_id,
                security(&auth),
                UpdateTopicCommandInput {
                    locale: input.locale,
                    title: input.title,
                    body: input.body,
                    body_format: input.body_format,
                    content_json: input.content_json,
                    metadata: input.metadata,
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: input.quotes.map(map_quotes),
                },
            )
            .await?;
        let author_profile = load_author_profile(db, tenant_id, topic.author_id, &topic.effective_locale).await?;
        Ok(map_topic(topic, author_profile))
    }

    async fn create_forum_reply_with_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: CreateForumReplyWithQuotesInput,
    ) -> Result<GqlForumReply> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_REPLIES_CREATE,
            "Permission denied: forum_replies:create required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let reply = ReplyService::new(db.clone(), event_bus.clone())
            .create_command(
                tenant_id,
                security(&auth),
                topic_id,
                CreateReplyCommandInput {
                    locale: input.locale,
                    content: input.content,
                    content_format: input
                        .content_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    parent_reply_id: input.parent_reply_id,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;
        let author_profile = load_author_profile(db, tenant_id, reply.author_id, &reply.effective_locale).await?;
        Ok(map_reply(reply, author_profile))
    }

    async fn update_forum_reply_with_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        reply_id: Uuid,
        input: UpdateForumReplyWithQuotesInput,
    ) -> Result<GqlForumReply> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_REPLIES_UPDATE,
            "Permission denied: forum_replies:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let reply = ReplyService::new(db.clone(), event_bus.clone())
            .update_command(
                tenant_id,
                reply_id,
                security(&auth),
                UpdateReplyCommandInput {
                    locale: input.locale,
                    content: input.content,
                    content_format: input.content_format,
                    content_json: input.content_json,
                    quotes: input.quotes.map(map_quotes),
                },
            )
            .await?;
        let author_profile = load_author_profile(db, tenant_id, reply.author_id, &reply.effective_locale).await?;
        Ok(map_reply(reply, author_profile))
    }
}

fn require_permission(
    ctx: &Context<'_>,
    permission: Permission,
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }
    Ok(auth)
}

fn resolve_tenant_scope(tenant: &TenantContext, requested: Option<Uuid>) -> Result<Uuid> {
    match requested {
        Some(requested) if requested != tenant.id => Err(
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ),
        ),
        Some(requested) => Ok(requested),
        None => Ok(tenant.id),
    }
}

fn security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

fn map_quotes(input: Vec<GqlForumQuoteReferenceInput>) -> Vec<ForumQuoteReferenceInput> {
    input
        .into_iter()
        .map(|quote| ForumQuoteReferenceInput {
            target_kind: match quote.target_kind {
                GqlForumQuoteTargetKind::Topic => ForumQuoteTargetKindInput::Topic,
                GqlForumQuoteTargetKind::Reply => ForumQuoteTargetKindInput::Reply,
            },
            target_id: quote.target_id,
            revision_id: quote.revision_id,
        })
        .collect()
}

async fn load_author_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    author_id: Option<Uuid>,
    locale: &str,
) -> Result<Option<GqlProfileSummary>> {
    let Some(author_id) = author_id else {
        return Ok(None);
    };
    Ok(ProfileService::new(db.clone())
        .find_profile_summary(tenant_id, author_id, Some(locale), None)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .map(Into::into))
}

fn map_topic(topic: TopicResponse, author_profile: Option<GqlProfileSummary>) -> GqlForumTopic {
    GqlForumTopic {
        id: topic.id,
        requested_locale: topic.requested_locale,
        locale: topic.locale,
        effective_locale: topic.effective_locale,
        available_locales: topic.available_locales,
        category_id: topic.category_id,
        author_id: topic.author_id,
        author_profile,
        title: topic.title,
        slug: topic.slug,
        body: topic.body,
        body_format: topic.body_format,
        content_json: topic.content_json,
        metadata: topic.metadata,
        status: topic.status,
        tags: topic.tags,
        channel_slugs: topic.channel_slugs,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
        updated_at: topic.updated_at,
    }
}

fn map_reply(reply: ReplyResponse, author_profile: Option<GqlProfileSummary>) -> GqlForumReply {
    GqlForumReply {
        id: reply.id,
        requested_locale: reply.requested_locale,
        locale: reply.locale,
        effective_locale: reply.effective_locale,
        topic_id: reply.topic_id,
        author_id: reply.author_id,
        author_profile,
        content: reply.content,
        content_format: reply.content_format,
        status: reply.status,
        vote_score: reply.vote_score,
        current_user_vote: reply.current_user_vote,
        is_solution: reply.is_solution,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }
}

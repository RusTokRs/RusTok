use async_graphql::{Context, InputObject, Object, Result};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, Permission, RichTextDocument, TenantContext};
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{ProfileService, ProfilesReader, graphql::GqlProfileSummary};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::reply_create_transport::{
    ForumReplyCreateTransport, reply_create_audience_port_context,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};
use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ForumQuoteReferenceInput,
    ForumQuoteTargetKindInput, ReplyResponse, ReplyService, TopicResponse, TopicService,
    UpdateReplyCommandInput, UpdateTopicCommandInput,
};

use super::{
    ForumGraphqlRuntimeData, GqlForumQuoteReferenceInput, GqlForumQuoteTargetKind, GqlForumReply,
    GqlForumTopic,
};

const MODULE_SLUG: &str = "forum";

#[derive(InputObject)]
pub struct CreateForumTopicWithQuotesInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: RichTextDocument,
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
    pub body: Option<RichTextDocument>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
    pub quotes: Option<Vec<GqlForumQuoteReferenceInput>>,
}

#[derive(InputObject)]
pub struct CreateForumReplyWithQuotesInput {
    pub locale: String,
    pub content: RichTextDocument,
    pub parent_reply_id: Option<Uuid>,
    #[graphql(default)]
    pub quotes: Vec<GqlForumQuoteReferenceInput>,
}

#[derive(InputObject)]
pub struct UpdateForumReplyWithQuotesInput {
    pub locale: String,
    pub content: Option<RichTextDocument>,
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_CREATE],
            "Permission denied: forum_topics:create required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Graphql,
            tenant_id,
            auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let topic = runtime
            .topic_service(db.clone(), event_bus.clone())
            .create_command_with_audience_context(
                tenant_id,
                security(auth),
                audience_context,
                CreateTopicCommandInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;
        let author_profile =
            load_author_profile(db, tenant_id, topic.author_id, &topic.effective_locale).await?;
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_UPDATE],
            "Permission denied: forum_topics:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let topic = TopicService::new(db.clone(), event_bus.clone())
            .update_command(
                tenant_id,
                topic_id,
                security(auth),
                UpdateTopicCommandInput {
                    locale: input.locale,
                    title: input.title,
                    body: input.body,
                    metadata: input.metadata,
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: input.quotes.map(map_quotes),
                },
            )
            .await?;
        let author_profile =
            load_author_profile(db, tenant_id, topic.author_id, &topic.effective_locale).await?;
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_CREATE],
            "Permission denied: forum_replies:create required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = reply_create_audience_port_context(
            ForumReplyCreateTransport::Graphql,
            tenant_id,
            auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let reply = runtime
            .reply_service(db.clone(), event_bus.clone())
            .create_command_with_audience_context(
                tenant_id,
                security(auth),
                topic_id,
                audience_context,
                CreateReplyCommandInput {
                    locale: input.locale,
                    content: input.content,
                    parent_reply_id: input.parent_reply_id,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;
        let author_profile =
            load_author_profile(db, tenant_id, reply.author_id, &reply.effective_locale).await?;
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_UPDATE],
            "Permission denied: forum_replies:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let reply = ReplyService::new(db.clone(), event_bus.clone())
            .update_command(
                tenant_id,
                reply_id,
                security(auth),
                UpdateReplyCommandInput {
                    locale: input.locale,
                    content: input.content,
                    quotes: input.quotes.map(map_quotes),
                },
            )
            .await?;
        let author_profile =
            load_author_profile(db, tenant_id, reply.author_id, &reply.effective_locale).await?;
        Ok(map_reply(reply, author_profile))
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
        body_plain_text: topic.body_plain_text,
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
        content_plain_text: reply.content_plain_text,
        status: reply.status,
        vote_score: reply.vote_score,
        current_user_vote: reply.current_user_vote,
        is_solution: reply.is_solution,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }
}

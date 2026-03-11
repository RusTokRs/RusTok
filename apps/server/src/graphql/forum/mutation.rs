use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::auth::AuthService;
use rustok_core::Permission;
use rustok_forum::{CategoryService, ReplyService, TopicService};
use rustok_outbox::TransactionalEventBus;

use super::types::*;

#[derive(Default)]
pub struct ForumMutation;

#[Object]
impl ForumMutation {
    async fn create_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateForumTopicInput,
    ) -> Result<GqlForumTopic> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_TOPICS_CREATE,
                Permission::FORUM_TOPICS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_topics:create required",
            ));
        }

        let security = auth.security_context();
        let service = TopicService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_forum::CreateTopicInput {
            locale: input.locale,
            category_id: input.category_id,
            title: input.title,
            slug: input.slug,
            body: input.body,
            body_format: input.body_format,
            content_json: input.content_json,
            tags: input.tags,
        };

        let topic = service.create(tenant_id, security, domain_input).await?;

        Ok(GqlForumTopic {
            id: topic.id,
            locale: topic.locale,
            effective_locale: topic.effective_locale,
            category_id: topic.category_id,
            author_id: topic.author_id,
            title: topic.title,
            slug: topic.slug,
            body: topic.body,
            status: topic.status,
            tags: topic.tags,
            is_pinned: topic.is_pinned,
            is_locked: topic.is_locked,
            reply_count: topic.reply_count,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        })
    }

    async fn update_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateForumTopicInput,
    ) -> Result<GqlForumTopic> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_TOPICS_UPDATE,
                Permission::FORUM_TOPICS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_topics:update required",
            ));
        }

        let security = auth.security_context();
        let service = TopicService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_forum::UpdateTopicInput {
            locale: input.locale,
            title: input.title,
            body: input.body,
            body_format: input.body_format,
            content_json: input.content_json,
            tags: input.tags,
        };

        let topic = service
            .update(tenant_id, id, security, domain_input)
            .await?;

        Ok(GqlForumTopic {
            id: topic.id,
            locale: topic.locale,
            effective_locale: topic.effective_locale,
            category_id: topic.category_id,
            author_id: topic.author_id,
            title: topic.title,
            slug: topic.slug,
            body: topic.body,
            status: topic.status,
            tags: topic.tags,
            is_pinned: topic.is_pinned,
            is_locked: topic.is_locked,
            reply_count: topic.reply_count,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        })
    }

    async fn delete_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_TOPICS_DELETE,
                Permission::FORUM_TOPICS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_topics:delete required",
            ));
        }

        let security = auth.security_context();
        let service = TopicService::new(db.clone(), event_bus.clone());
        service.delete(tenant_id, id, security).await?;

        Ok(true)
    }

    async fn create_forum_reply(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        input: CreateForumReplyInput,
    ) -> Result<GqlForumReply> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_REPLIES_CREATE,
                Permission::FORUM_REPLIES_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_replies:create required",
            ));
        }

        let security = auth.security_context();
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_forum::CreateReplyInput {
            locale: input.locale,
            content: input.content,
            content_format: input.content_format,
            content_json: input.content_json,
            parent_reply_id: input.parent_reply_id,
        };

        let reply = service
            .create(tenant_id, security, topic_id, domain_input)
            .await?;

        Ok(GqlForumReply {
            id: reply.id,
            locale: reply.locale,
            effective_locale: reply.effective_locale,
            topic_id: reply.topic_id,
            author_id: reply.author_id,
            content: reply.content,
            status: reply.status,
            parent_reply_id: reply.parent_reply_id,
            created_at: reply.created_at,
            updated_at: reply.updated_at,
        })
    }

    async fn create_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateForumCategoryInput,
    ) -> Result<GqlForumCategory> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_CATEGORIES_CREATE,
                Permission::FORUM_CATEGORIES_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_categories:create required",
            ));
        }

        let security = auth.security_context();
        let service = CategoryService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_forum::CreateCategoryInput {
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            description: input.description,
            icon: input.icon,
            color: input.color,
            parent_id: input.parent_id,
            position: input.position,
            moderated: input.moderated,
        };

        let category = service.create(tenant_id, security, domain_input).await?;

        Ok(GqlForumCategory {
            id: category.id,
            locale: category.locale,
            effective_locale: category.effective_locale,
            name: category.name,
            slug: category.slug,
            description: category.description,
            icon: category.icon,
            color: category.color,
            topic_count: category.topic_count,
            reply_count: category.reply_count,
        })
    }
}

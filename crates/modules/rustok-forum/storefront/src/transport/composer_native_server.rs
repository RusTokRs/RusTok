//! Composer write transport for the native `#[server]` storefront path.
//!
//! SSR handlers call the forum owner services directly with an audience context
//! pinned to `ForumTopicCreateTransport::NativeServer` /
//! `ForumReplyCreateTransport::NativeServer`, which is the same contract the
//! host REST controllers use, so neither transport can bypass the owner
//! audience gate. Tenant, actor, and permission identity come only from the
//! authenticated request extensions.

use leptos::prelude::*;

use super::graphql_adapter::ApiError;
use crate::model::{
    CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, StorefrontForumCreatedReply,
    StorefrontForumCreatedTopic, StorefrontForumComposerQuote,
};

pub async fn create_storefront_topic_server(
    draft: CreateStorefrontForumTopicDraft,
) -> Result<StorefrontForumCreatedTopic, ApiError> {
    storefront_topic_create_native(draft)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

pub async fn create_storefront_reply_server(
    draft: CreateStorefrontForumReplyDraft,
) -> Result<StorefrontForumCreatedReply, ApiError> {
    storefront_reply_create_native(draft)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[cfg(feature = "ssr")]
fn composer_quotes(
    quotes: Vec<StorefrontForumComposerQuote>,
) -> Result<Vec<rustok_forum::ForumQuoteReferenceInput>, ServerFnError> {
    use rustok_forum::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput};

    quotes
        .into_iter()
        .map(|quote| {
            let target_kind = match quote.target_kind.trim().to_ascii_uppercase().as_str() {
                "TOPIC" => ForumQuoteTargetKindInput::Topic,
                "REPLY" => ForumQuoteTargetKindInput::Reply,
                _ => return Err(ServerFnError::new("unsupported quote target kind")),
            };
            let target_id = uuid::Uuid::parse_str(quote.target_id.trim())
                .map_err(|_| ServerFnError::new("quote target_id must be a valid UUID"))?;
            Ok(ForumQuoteReferenceInput {
                target_kind,
                target_id,
                revision_id: quote.revision_id,
            })
        })
        .collect()
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-topic-create")]
async fn storefront_topic_create_native(
    draft: CreateStorefrontForumTopicDraft,
) -> Result<StorefrontForumCreatedTopic, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::native_server_adapter::{normalize_locale, server_error};
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, Permission, RequestContext, RichTextDocument,
            SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
            TenantContext, has_any_effective_permission,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::{
            CreateTopicCommandInput, ForumSettingsProviders, ForumTopicCreateTransport,
            SharedForumAudienceFactsPort, TopicService, topic_create_audience_port_context,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0
            .ok_or_else(|| ServerFnError::new("Authentication required"))?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-topic-create requires TransactionalEventBus in host runtime context",
                )
            })?;
        if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_CREATE]) {
            return Err(ServerFnError::new(
                "Permission denied: forum_topics:create required",
            ));
        }
        let category_id = uuid::Uuid::parse_str(draft.category_id.trim())
            .map_err(|_| ServerFnError::new("category_id must be a valid UUID"))?;
        let effective_locale = normalize_locale(Some(draft.locale.as_str()));
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = topic_create_audience_port_context(
            ForumTopicCreateTransport::NativeServer,
            tenant.id,
            &auth,
            Some(&request),
            tenant.default_locale.as_str(),
        )
        .map_err(server_error)?;
        let body = RichTextDocument::from_plain_text(draft.body_text.as_str());
        if !body.has_text() {
            return Err(ServerFnError::new("forum composer body must carry text"));
        }
        let db = runtime_ctx.db_clone();
        // The owner settings (pre-moderation, content limits, and create
        // cooldowns) resolve through the same host-published static readers the
        // GraphQL and REST runtime data use, so no transport can skip a
        // configured policy. Absent readers yield owner defaults, not a bypass.
        let settings_providers = match (
            runtime_ctx.shared_get::<SharedStaticModuleSettingsReader>(),
            runtime_ctx.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
        ) {
            (Some(static_reader), Some(transactional_reader)) => ForumSettingsProviders::default()
                .with_static_readers(static_reader, transactional_reader),
            _ => ForumSettingsProviders::default(),
        };
        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => TopicService::with_audience_facts(db, event_bus, facts),
            None => TopicService::new(db, event_bus),
        }
        .with_settings_providers(settings_providers);
        let topic = service
            .create_command_with_audience_context(
                tenant.id,
                security,
                audience_context,
                CreateTopicCommandInput {
                    locale: effective_locale,
                    category_id,
                    title: draft.title,
                    slug: None,
                    body,
                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                    tags: Vec::new(),
                    channel_slugs: None,
                    quotes: composer_quotes(draft.quotes)?,
                },
            )
            .await
            .map_err(server_error)?;

        Ok(StorefrontForumCreatedTopic {
            topic_id: topic.id.to_string(),
            slug: topic.slug,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = draft;
        Err(ServerFnError::new(
            "forum/storefront-topic-create requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-reply-create")]
async fn storefront_reply_create_native(
    draft: CreateStorefrontForumReplyDraft,
) -> Result<StorefrontForumCreatedReply, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, Permission, RequestContext, RichTextDocument,
            SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
            TenantContext, has_any_effective_permission,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::{
            CreateReplyCommandInput, ForumReplyCreateTransport, ForumSettingsProviders,
            ReplyService, SharedForumAudienceFactsPort, reply_create_audience_port_context,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0
            .ok_or_else(|| ServerFnError::new("Authentication required"))?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-reply-create requires TransactionalEventBus in host runtime context",
                )
            })?;
        if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_REPLIES_CREATE]) {
            return Err(ServerFnError::new(
                "Permission denied: forum_replies:create required",
            ));
        }
        let topic_id = uuid::Uuid::parse_str(draft.topic_id.trim())
            .map_err(|_| ServerFnError::new("topic_id must be a valid UUID"))?;
        let parent_reply_id = draft
            .parent_reply_id
            .as_deref()
            .map(|value| {
                uuid::Uuid::parse_str(value.trim())
                    .map_err(|_| ServerFnError::new("parent_reply_id must be a valid UUID"))
            })
            .transpose()?;
        let effective_locale = normalize_locale(Some(draft.locale.as_str()));
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = reply_create_audience_port_context(
            ForumReplyCreateTransport::NativeServer,
            tenant.id,
            &auth,
            Some(&request),
            tenant.default_locale.as_str(),
        )
        .map_err(server_error)?;
        let content = RichTextDocument::from_plain_text(draft.body_text.as_str());
        if !content.has_text() {
            return Err(ServerFnError::new("forum composer body must carry text"));
        }
        let db = runtime_ctx.db_clone();
        // The owner settings (pre-moderation, content limits, and create
        // cooldowns) resolve through the same host-published static readers the
        // GraphQL and REST runtime data use, so no transport can skip a
        // configured policy. Absent readers yield owner defaults, not a bypass.
        let settings_providers = match (
            runtime_ctx.shared_get::<SharedStaticModuleSettingsReader>(),
            runtime_ctx.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
        ) {
            (Some(static_reader), Some(transactional_reader)) => ForumSettingsProviders::default()
                .with_static_readers(static_reader, transactional_reader),
            _ => ForumSettingsProviders::default(),
        };
        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => ReplyService::with_audience_facts(db, event_bus, facts),
            None => ReplyService::new(db, event_bus),
        }
        .with_settings_providers(settings_providers);
        let reply = service
            .create_command_with_audience_context(
                tenant.id,
                security,
                topic_id,
                audience_context,
                CreateReplyCommandInput {
                    locale: effective_locale,
                    content,
                    parent_reply_id,
                    quotes: composer_quotes(draft.quotes)?,
                },
            )
            .await
            .map_err(server_error)?;

        Ok(StorefrontForumCreatedReply {
            reply_id: reply.id.to_string(),
            topic_id: reply.topic_id.to_string(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = draft;
        Err(ServerFnError::new(
            "forum/storefront-reply-create requires the `ssr` feature",
        ))
    }
}

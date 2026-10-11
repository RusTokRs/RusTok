mod graphql_adapter {
    include!("graphql_adapter.rs");
    include!("category_route_graphql_adapter.rs");
    include!("topic_route_graphql_adapter.rs");
    include!("revision_graphql_adapter.rs");
}
mod composer_graphql;
mod composer_native_server;
mod native_server_adapter {
    include!("native_server_adapter.rs");
    include!("native_server_adapter_bulk.rs");
    include!("native_server_adapter_category_route.rs");
    include!("native_server_adapter_topic_route.rs");
    include!("native_server_adapter_revision.rs");
}
mod native_server_adapter_pages;

use crate::model::{
    CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, ForumReplyPage,
    ForumTopicPage, StorefrontForumCategoryRouteResolution, StorefrontForumCreatedReply,
    StorefrontForumCreatedTopic, StorefrontForumData, StorefrontForumTopicRouteResolution,
};
use serde::{Deserialize, Serialize};

pub type TransportError = graphql_adapter::ApiError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontForumBulkReadResult {
    pub processed: u64,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(rename = "snapshotAt")]
    pub snapshot_at: String,
}

fn use_native_transport() -> bool {
    cfg!(any(feature = "ssr", feature = "hydrate"))
}

pub async fn fetch_storefront_forum(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, TransportError> {
    if use_native_transport() {
        native_server_adapter::fetch_storefront_forum_server(
            selected_category_id,
            selected_topic_id,
            locale,
        )
        .await
    } else {
        graphql_adapter::fetch_storefront_forum_graphql(
            selected_category_id,
            selected_topic_id,
            locale,
        )
        .await
    }
}

/// Fetches the next topic page for the list variant the surface is holding.
///
/// The cursor is opaque and variant-specific, so `read_state_available` is part of the page request:
/// both transports continue on the service that produced the cursor instead of re-pointing it.
/// A failed page never falls back to the other transport.
pub async fn fetch_storefront_forum_topic_page(
    selected_category_id: Option<String>,
    after: String,
    locale: Option<String>,
    read_state_available: bool,
) -> Result<ForumTopicPage, TransportError> {
    if use_native_transport() {
        native_server_adapter_pages::fetch_storefront_forum_topic_page_server(
            selected_category_id,
            after,
            locale,
            read_state_available,
        )
        .await
    } else {
        graphql_adapter::fetch_storefront_forum_topic_page_graphql(
            selected_category_id,
            after,
            locale,
            read_state_available,
        )
        .await
    }
}

/// Fetches the next reply page of one topic through the selected transport.
pub async fn fetch_storefront_forum_reply_page(
    topic_id: String,
    after: String,
    locale: Option<String>,
) -> Result<ForumReplyPage, TransportError> {
    if use_native_transport() {
        native_server_adapter_pages::fetch_storefront_forum_reply_page_server(
            topic_id, after, locale,
        )
        .await
    } else {
        graphql_adapter::fetch_storefront_forum_reply_page_graphql(topic_id, after, locale).await
    }
}

pub async fn fetch_storefront_topic_current_revision(
    topic_id: String,
    locale: Option<String>,
) -> Result<Option<String>, TransportError> {
    if use_native_transport() {
        native_server_adapter::fetch_storefront_topic_current_revision_server(topic_id, locale)
            .await
    } else {
        graphql_adapter::fetch_storefront_topic_current_revision_graphql(topic_id, locale).await
    }
}

pub async fn fetch_storefront_reply_current_revision(
    reply_id: String,
    locale: Option<String>,
) -> Result<Option<String>, TransportError> {
    if use_native_transport() {
        native_server_adapter::fetch_storefront_reply_current_revision_server(reply_id, locale)
            .await
    } else {
        graphql_adapter::fetch_storefront_reply_current_revision_graphql(reply_id, locale).await
    }
}

pub async fn resolve_storefront_category_route(
    locale: String,
    slug: String,
) -> Result<Option<StorefrontForumCategoryRouteResolution>, TransportError> {
    if use_native_transport() {
        native_server_adapter::resolve_storefront_category_route_server(locale, slug).await
    } else {
        graphql_adapter::resolve_storefront_category_route_graphql(locale, slug).await
    }
}

pub async fn resolve_storefront_topic_route(
    locale: String,
    short_id: String,
    slug: String,
) -> Result<Option<StorefrontForumTopicRouteResolution>, TransportError> {
    if use_native_transport() {
        native_server_adapter::resolve_storefront_topic_route_server(locale, short_id, slug).await
    } else {
        graphql_adapter::resolve_storefront_topic_route_graphql(locale, short_id, slug).await
    }
}

/// Creates a topic through the single selected storefront transport.
///
/// The native `#[server]` path and the GraphQL path carry the same owner command
/// contract; a failed selected transport never falls back to the other path.
pub async fn create_storefront_forum_topic(
    draft: CreateStorefrontForumTopicDraft,
) -> Result<StorefrontForumCreatedTopic, TransportError> {
    if use_native_transport() {
        composer_native_server::create_storefront_topic_server(draft).await
    } else {
        composer_graphql::create_storefront_topic_graphql(draft).await
    }
}

pub async fn create_storefront_forum_reply(
    draft: CreateStorefrontForumReplyDraft,
) -> Result<StorefrontForumCreatedReply, TransportError> {
    if use_native_transport() {
        composer_native_server::create_storefront_reply_server(draft).await
    } else {
        composer_graphql::create_storefront_reply_graphql(draft).await
    }
}

pub async fn mark_storefront_topic_read(
    topic_id: String,
    locale: Option<String>,
) -> Result<(), TransportError> {
    if use_native_transport() {
        native_server_adapter::mark_storefront_topic_read_server(topic_id, locale).await
    } else {
        graphql_adapter::mark_storefront_topic_read_graphql(topic_id, locale).await
    }
}

pub async fn mark_storefront_category_read(
    category_id: String,
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<StorefrontForumBulkReadResult, TransportError> {
    if use_native_transport() {
        native_server_adapter::mark_storefront_category_read_server(
            category_id,
            cursor,
            limit,
            locale,
        )
        .await
    } else {
        graphql_adapter::mark_storefront_category_read_graphql(category_id, cursor, limit, locale)
            .await
    }
}

pub async fn mark_all_storefront_topics_read(
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<StorefrontForumBulkReadResult, TransportError> {
    if use_native_transport() {
        native_server_adapter::mark_all_storefront_topics_read_server(cursor, limit, locale).await
    } else {
        graphql_adapter::mark_all_storefront_topics_read_graphql(cursor, limit, locale).await
    }
}

use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::graphql_adapter::ApiError;
use crate::model::{
    ForumCategoryConnection, ForumReplyConnection, ForumTopicConnection, ForumTopicDetail,
    StorefrontForumData,
};

const STOREFRONT_FORUM_CATEGORIES_QUERY: &str = "query StorefrontForumCategories($tenantId: UUID, $locale: String, $pagination: PaginationInput) { forumStorefrontCategories(tenantId: $tenantId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale name slug description icon color topicCount replyCount } } }";
const STOREFRONT_FORUM_AUDIENCE_TOPICS_QUERY: &str = "query StorefrontForumAudienceTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $pagination: PaginationInput) { forumStorefrontAudienceTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale categoryId title slug status isPinned isLocked replyCount createdAt } } }";
const STOREFRONT_FORUM_UNREAD_TOPICS_QUERY: &str = "query StorefrontForumUnreadTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $limit: Int) { forumStorefrontUnreadTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, limit: $limit) { total items { id effectiveLocale categoryId title slug status isPinned isLocked replyCount createdAt readStateExplicit lastReadPosition lastReadRevision unreadCount hasUnreadTopicRevision isUnread } } }";
const STOREFRONT_FORUM_TOPIC_QUERY: &str = "query StorefrontForumTopic($tenantId: UUID, $id: UUID!, $locale: String) { forumStorefrontAudienceTopic(tenantId: $tenantId, id: $id, locale: $locale) { id effectiveLocale availableLocales categoryId title slug body bodyFormat status tags isPinned isLocked replyCount createdAt updatedAt } }";
const STOREFRONT_FORUM_REPLIES_QUERY: &str = "query StorefrontForumReplies($tenantId: UUID, $topicId: UUID!, $locale: String, $pagination: PaginationInput) { forumStorefrontReplies(tenantId: $tenantId, topicId: $topicId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale topicId content contentFormat status parentReplyId createdAt updatedAt } } }";

#[derive(Debug, Deserialize)]
struct StorefrontForumCategoriesResponse {
    #[serde(rename = "forumStorefrontCategories")]
    forum_storefront_categories: ForumCategoryConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumAudienceTopicsResponse {
    #[serde(rename = "forumStorefrontAudienceTopics")]
    forum_storefront_audience_topics: ForumTopicConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumUnreadTopicsResponse {
    #[serde(rename = "forumStorefrontUnreadTopics")]
    forum_storefront_unread_topics: ForumTopicConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumTopicResponse {
    #[serde(rename = "forumStorefrontAudienceTopic")]
    forum_storefront_topic: Option<ForumTopicDetail>,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumRepliesResponse {
    #[serde(rename = "forumStorefrontReplies")]
    forum_storefront_replies: ForumReplyConnection,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct CategoriesVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct TopicsVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct UnreadTopicsVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    locale: Option<String>,
    limit: i32,
}

#[derive(Debug, Serialize)]
struct TopicVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct RepliesVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "topicId")]
    topic_id: String,
    locale: Option<String>,
    pagination: PaginationInput,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request_raw<V, T>(query: &str, variables: V) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    request_raw(query, variables)
        .await
        .map_err(|error| ApiError::Graphql(error.to_string()))
}

fn personalization_unavailable(error: &GraphqlHttpError) -> bool {
    match error {
        GraphqlHttpError::Unauthorized => true,
        GraphqlHttpError::Graphql(message) => {
            message == "Authentication required" || message.starts_with("Permission denied:")
        }
        GraphqlHttpError::Network | GraphqlHttpError::Http(_) => false,
    }
}

pub async fn fetch_storefront_forum_graphql(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, ApiError> {
    let categories_response: StorefrontForumCategoriesResponse = request(
        STOREFRONT_FORUM_CATEGORIES_QUERY,
        CategoriesVariables {
            tenant_id: None,
            locale: locale.clone(),
            pagination: PaginationInput {
                offset: 0,
                limit: 12,
            },
        },
    )
    .await?;

    let mut selected_topic = if let Some(topic_id) = selected_topic_id.clone() {
        let response: StorefrontForumTopicResponse = request(
            STOREFRONT_FORUM_TOPIC_QUERY,
            TopicVariables {
                tenant_id: None,
                id: topic_id,
                locale: locale.clone(),
            },
        )
        .await?;
        response.forum_storefront_topic
    } else {
        None
    };

    let resolved_category_id = selected_category_id
        .or_else(|| {
            selected_topic
                .as_ref()
                .map(|topic| topic.category_id.clone())
        })
        .or_else(|| {
            categories_response
                .forum_storefront_categories
                .items
                .first()
                .map(|item| item.id.clone())
        });

    let personalized = request_raw::<_, StorefrontForumUnreadTopicsResponse>(
        STOREFRONT_FORUM_UNREAD_TOPICS_QUERY,
        UnreadTopicsVariables {
            tenant_id: None,
            category_id: resolved_category_id.clone(),
            locale: locale.clone(),
            limit: 20,
        },
    )
    .await;
    let (topics, read_state_available) = match personalized {
        Ok(response) => (response.forum_storefront_unread_topics, true),
        Err(error) if personalization_unavailable(&error) => {
            let response: StorefrontForumAudienceTopicsResponse = request(
                STOREFRONT_FORUM_AUDIENCE_TOPICS_QUERY,
                TopicsVariables {
                    tenant_id: None,
                    category_id: resolved_category_id.clone(),
                    locale: locale.clone(),
                    pagination: PaginationInput {
                        offset: 0,
                        limit: 20,
                    },
                },
            )
            .await?;
            (response.forum_storefront_audience_topics, false)
        }
        Err(error) => return Err(ApiError::Graphql(error.to_string())),
    };

    let resolved_topic_id =
        selected_topic_id.or_else(|| topics.items.first().map(|item| item.id.clone()));

    if selected_topic.is_none() {
        if let Some(topic_id) = resolved_topic_id.clone() {
            let response: StorefrontForumTopicResponse = request(
                STOREFRONT_FORUM_TOPIC_QUERY,
                TopicVariables {
                    tenant_id: None,
                    id: topic_id,
                    locale: locale.clone(),
                },
            )
            .await?;
            selected_topic = response.forum_storefront_topic;
        }
    }

    let replies = if selected_topic.is_some() {
        if let Some(topic_id) = resolved_topic_id.clone() {
            let response: StorefrontForumRepliesResponse = request(
                STOREFRONT_FORUM_REPLIES_QUERY,
                RepliesVariables {
                    tenant_id: None,
                    topic_id,
                    locale,
                    pagination: PaginationInput {
                        offset: 0,
                        limit: 20,
                    },
                },
            )
            .await?;
            response.forum_storefront_replies
        } else {
            empty_replies()
        }
    } else {
        empty_replies()
    };

    Ok(StorefrontForumData {
        categories: categories_response.forum_storefront_categories,
        topics,
        selected_category_id: resolved_category_id,
        selected_topic_id: resolved_topic_id,
        selected_topic,
        replies,
        read_state_available,
    })
}

fn empty_replies() -> ForumReplyConnection {
    ForumReplyConnection {
        items: Vec::new(),
        total: 0,
    }
}

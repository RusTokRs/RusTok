use rustok_graphql::{
    GraphqlHttpError, GraphqlRequest, execute as execute_graphql,
};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    ForumCategoryConnection, ForumReplyConnection, ForumTopicConnection, ForumTopicDetail,
    StorefrontForumData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(_) => f.write_str("Forum GraphQL request failed"),
            Self::ServerFn(_) => f.write_str("Forum server request failed"),
        }
    }
}

impl std::error::Error for ApiError {}

const STOREFRONT_FORUM_CATEGORIES_QUERY: &str = "query StorefrontForumCategories($tenantId: UUID, $locale: String, $pagination: PaginationInput) { forumStorefrontCategories(tenantId: $tenantId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale name slug description icon color topicCount replyCount } } }";
const STOREFRONT_FORUM_TOPICS_QUERY: &str = "query StorefrontForumTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $pagination: PaginationInput) { forumStorefrontTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale categoryId title slug status isPinned isLocked replyCount createdAt } } }";
const STOREFRONT_FORUM_UNREAD_TOPICS_QUERY: &str = "query StorefrontForumUnreadTopics($tenantId: UUID, $categoryId: UUID, $locale: String, $limit: Int) { forumStorefrontUnreadTopics(tenantId: $tenantId, categoryId: $categoryId, locale: $locale, limit: $limit) { total items { id effectiveLocale categoryId title slug status isPinned isLocked replyCount createdAt readStateExplicit lastReadPosition lastReadRevision unreadCount hasUnreadTopicRevision isUnread } } }";
const STOREFRONT_FORUM_TOPIC_QUERY: &str = "query StorefrontForumTopic($tenantId: UUID, $id: UUID!, $locale: String) { forumStorefrontTopic(tenantId: $tenantId, id: $id, locale: $locale) { id effectiveLocale availableLocales categoryId title slug body bodyFormat status tags isPinned isLocked replyCount createdAt updatedAt } }";
const STOREFRONT_FORUM_REPLIES_QUERY: &str = "query StorefrontForumReplies($tenantId: UUID, $topicId: UUID!, $locale: String, $pagination: PaginationInput) { forumStorefrontReplies(tenantId: $tenantId, topicId: $topicId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale topicId content contentFormat status parentReplyId createdAt updatedAt } } }";
const MARK_STOREFRONT_FORUM_TOPIC_READ_MUTATION: &str = "mutation MarkStorefrontForumTopicRead($tenantId: UUID, $topicId: UUID!, $locale: String) { markForumStorefrontTopicRead(tenantId: $tenantId, topicId: $topicId, locale: $locale) { topicId } }";

#[derive(Debug, Deserialize)]
struct StorefrontForumCategoriesResponse {
    #[serde(rename = "forumStorefrontCategories")]
    forum_storefront_categories: ForumCategoryConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumTopicsResponse {
    #[serde(rename = "forumStorefrontTopics")]
    forum_storefront_topics: ForumTopicConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumUnreadTopicsResponse {
    #[serde(rename = "forumStorefrontUnreadTopics")]
    forum_storefront_unread_topics: ForumTopicConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumTopicResponse {
    #[serde(rename = "forumStorefrontTopic")]
    forum_storefront_topic: Option<ForumTopicDetail>,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumRepliesResponse {
    #[serde(rename = "forumStorefrontReplies")]
    forum_storefront_replies: ForumReplyConnection,
}

#[derive(Debug, Deserialize)]
struct MarkStorefrontForumTopicReadResponse {
    #[serde(rename = "markForumStorefrontTopicRead")]
    mark_forum_storefront_topic_read: MarkStorefrontForumTopicReadPayload,
}

#[derive(Debug, Deserialize)]
struct MarkStorefrontForumTopicReadPayload {
    #[serde(rename = "topicId")]
    topic_id: String,
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

#[derive(Debug, Serialize)]
struct MarkTopicReadVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "topicId")]
    topic_id: String,
    locale: Option<String>,
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
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
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
            let response: StorefrontForumTopicsResponse = request(
                STOREFRONT_FORUM_TOPICS_QUERY,
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
            (response.forum_storefront_topics, false)
        }
        Err(error) => return Err(ApiError::Graphql(error.to_string())),
    };

    let resolved_topic_id = selected_topic_id.or_else(|| {
        topics
            .items
            .first()
            .map(|item| item.id.clone())
    });

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

    let replies = if let Some(topic_id) = resolved_topic_id.clone() {
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
        ForumReplyConnection {
            items: Vec::new(),
            total: 0,
        }
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

pub async fn mark_storefront_topic_read_graphql(
    topic_id: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    let response: MarkStorefrontForumTopicReadResponse = request(
        MARK_STOREFRONT_FORUM_TOPIC_READ_MUTATION,
        MarkTopicReadVariables {
            tenant_id: None,
            topic_id: topic_id.clone(),
            locale,
        },
    )
    .await?;
    if response.mark_forum_storefront_topic_read.topic_id != topic_id {
        return Err(ApiError::Graphql(
            "Forum topic read mutation returned a mismatched topic identity".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_graphql::GraphqlHttpError;

    use super::{ApiError, personalization_unavailable};

    #[test]
    fn storefront_transport_errors_redact_public_display_but_keep_debug_detail() {
        let secret = "https://internal.example/api/graphql: database password=private";
        for (error, expected) in [
            (
                ApiError::Graphql(secret.to_string()),
                "Forum GraphQL request failed",
            ),
            (
                ApiError::ServerFn(secret.to_string()),
                "Forum server request failed",
            ),
        ] {
            let display = error.to_string();
            assert_eq!(display, expected);
            assert!(!display.contains(secret));
            assert!(format!("{error:?}").contains(secret));
        }
    }

    #[test]
    fn personalization_degrades_only_for_explicit_auth_failures() {
        assert!(personalization_unavailable(&GraphqlHttpError::Unauthorized));
        assert!(personalization_unavailable(&GraphqlHttpError::Graphql(
            "Authentication required".to_string()
        )));
        assert!(personalization_unavailable(&GraphqlHttpError::Graphql(
            "Permission denied: forum_topics:list required".to_string()
        )));
        assert!(!personalization_unavailable(&GraphqlHttpError::Network));
        assert!(!personalization_unavailable(&GraphqlHttpError::Http(
            "500".to_string()
        )));
    }
}

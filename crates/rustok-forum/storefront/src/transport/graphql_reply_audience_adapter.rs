use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::graphql_adapter::ApiError;
use crate::model::ForumReplyConnection;

const STOREFRONT_FORUM_AUDIENCE_REPLIES_QUERY: &str = "query StorefrontForumAudienceReplies($tenantId: UUID, $topicId: UUID!, $locale: String, $pagination: PaginationInput) { forumStorefrontAudienceReplies(tenantId: $tenantId, topicId: $topicId, locale: $locale, pagination: $pagination) { total items { id effectiveLocale topicId content contentFormat status parentReplyId createdAt updatedAt } } }";

#[derive(Debug, Deserialize)]
struct StorefrontForumAudienceRepliesResponse {
    #[serde(rename = "forumStorefrontAudienceReplies")]
    forum_storefront_audience_replies: ForumReplyConnection,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    offset: i64,
    limit: i64,
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

pub async fn fetch_storefront_replies_graphql(
    topic_id: Option<String>,
    locale: Option<String>,
) -> Result<ForumReplyConnection, ApiError> {
    let Some(topic_id) = topic_id else {
        return Ok(empty_replies());
    };

    let response: StorefrontForumAudienceRepliesResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            STOREFRONT_FORUM_AUDIENCE_REPLIES_QUERY,
            Some(RepliesVariables {
                tenant_id: None,
                topic_id,
                locale,
                pagination: PaginationInput {
                    offset: 0,
                    limit: 20,
                },
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(response.forum_storefront_audience_replies)
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

fn empty_replies() -> ForumReplyConnection {
    ForumReplyConnection {
        items: Vec::new(),
        total: 0,
    }
}

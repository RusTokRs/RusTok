#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::topic_fork_model::{
    ForumTopicForkCandidate, ForumTopicForkCommand, ForumTopicForkReceipt, ForumTopicForkReplyPage,
};

pub type ApiError = String;

const FORK_CANDIDATES_QUERY: &str = "query ForumAdminForkCandidates($locale: String, $pagination: PaginationInput!) { forumTopics(locale: $locale, pagination: $pagination) { items { id locale title category_id: categoryId reply_count: replyCount } } }";
const FORK_REPLIES_QUERY: &str = "query ForumAdminForkReplies($topicId: UUID!, $locale: String, $pagination: PaginationInput!) { forumReplies(topicId: $topicId, locale: $locale, pagination: $pagination) { total items { id content_preview: contentPlainText status parent_reply_id: parentReplyId created_at: createdAt } } }";
const FORK_TOPIC_MUTATION: &str = "mutation ForumAdminForkTopic($sourceTopicId: UUID!, $input: ForkForumTopicReplyBranchGraphqlInput!) { forkForumTopicReplyBranch(sourceTopicId: $sourceTopicId, input: $input) { operation_id: operationId event_id: eventId source_topic_id: sourceTopicId target_topic_id: targetTopicId root_reply_id: rootReplyId category_id: categoryId actor_id: actorId reason copied_reply_count: copiedReplyCount copied_published_reply_count: copiedPublishedReplyCount copied_body_count: copiedBodyCount copied_reply_revision_count: copiedReplyRevisionCount copied_relation_revision_count: copiedRelationRevisionCount copied_mention_count: copiedMentionCount copied_quote_count: copiedQuoteCount forked_at: forkedAt } }";

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumTopicForkCandidate>,
}

#[derive(Debug, Deserialize)]
struct RepliesResponse {
    #[serde(rename = "forumReplies")]
    forum_replies: ForumTopicForkReplyPage,
}

#[derive(Debug, Deserialize)]
struct ForkResponse {
    #[serde(rename = "forkForumTopicReplyBranch")]
    fork_forum_topic_reply_branch: ForumTopicForkReceipt,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct CandidatesVariables {
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct RepliesVariables {
    #[serde(rename = "topicId")]
    topic_id: String,
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct ForkVariables {
    #[serde(rename = "sourceTopicId")]
    source_topic_id: String,
    input: ForkInput,
}

#[derive(Debug, Serialize)]
struct ForkInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "targetTopicId")]
    target_topic_id: String,
    #[serde(rename = "rootReplyId")]
    root_reply_id: String,
    locale: String,
    title: String,
    slug: Option<String>,
    reason: String,
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

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn fetch_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumTopicForkCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        FORK_CANDIDATES_QUERY,
        CandidatesVariables {
            locale: Some(locale),
            pagination: PaginationInput {
                offset: 0,
                limit: 100,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_topics.items)
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    source_topic_id: String,
    locale: String,
) -> Result<ForumTopicForkReplyPage, ApiError> {
    let response: RepliesResponse = request(
        FORK_REPLIES_QUERY,
        RepliesVariables {
            topic_id: source_topic_id,
            locale: Some(locale),
            pagination: PaginationInput {
                offset: 0,
                limit: 500,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_replies)
}

pub async fn fork_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicForkCommand,
) -> Result<ForumTopicForkReceipt, ApiError> {
    let response: ForkResponse = request(
        FORK_TOPIC_MUTATION,
        ForkVariables {
            source_topic_id: command.source_topic_id,
            input: ForkInput {
                operation_id: command.operation_id,
                target_topic_id: command.target_topic_id,
                root_reply_id: command.root_reply_id,
                locale: command.locale,
                title: command.title,
                slug: command.slug,
                reason: command.reason,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.fork_forum_topic_reply_branch)
}

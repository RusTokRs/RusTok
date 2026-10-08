use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::topic_split_model::{
    ForumTopicSplitCandidate, ForumTopicSplitCommand, ForumTopicSplitReceipt,
    ForumTopicSplitReply, ForumTopicSplitReplyPage,
};

pub type ApiError = String;

const SPLIT_CANDIDATES_QUERY: &str = "query ForumAdminSplitCandidates($locale: String, $pagination: PaginationInput!) { forumTopics(locale: $locale, pagination: $pagination) { items { id title category_id: categoryId reply_count: replyCount } } }";
const SPLIT_REPLIES_QUERY: &str = "query ForumAdminSplitReplies($topicId: UUID!, $locale: String, $after: String, $perPage: Int) { forumReplies(topicId: $topicId, locale: $locale, after: $after, perPage: $perPage) { nextCursor items { id content_preview: contentPlainText status parent_reply_id: parentReplyId created_at: createdAt } } }";
const SPLIT_TOPIC_MUTATION: &str = "mutation ForumAdminSplitTopic($sourceTopicId: UUID!, $input: SplitForumTopicRepliesGraphqlInput!) { splitForumTopicReplies(sourceTopicId: $sourceTopicId, input: $input) { operation_id: operationId event_id: eventId source_topic_id: sourceTopicId target_topic_id: targetTopicId category_id: categoryId actor_id: actorId reason moved_reply_count: movedReplyCount moved_published_reply_count: movedPublishedReplyCount source_resulting_published_reply_count: sourceResultingPublishedReplyCount target_resulting_published_reply_count: targetResultingPublishedReplyCount solution_reply_id: solutionReplyId split_at: splitAt } }";

const ADMIN_REPLY_PAGE_SIZE: i32 = 100;
const ADMIN_REPLY_LOAD_LIMIT: usize = 5_000;

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumTopicSplitCandidate>,
}

#[derive(Debug, Deserialize)]
struct RepliesResponse {
    #[serde(rename = "forumReplies")]
    forum_replies: ReplyCursorPage,
}

#[derive(Debug, Deserialize)]
struct ReplyCursorPage {
    items: Vec<ForumTopicSplitReply>,
    #[serde(rename = "nextCursor", default)]
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SplitResponse {
    #[serde(rename = "splitForumTopicReplies")]
    split_forum_topic_replies: ForumTopicSplitReceipt,
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
    after: Option<String>,
    #[serde(rename = "perPage")]
    per_page: i32,
}

#[derive(Debug, Serialize)]
struct SplitVariables {
    #[serde(rename = "sourceTopicId")]
    source_topic_id: String,
    input: SplitInput,
}

#[derive(Debug, Serialize)]
struct SplitInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "targetTopicId")]
    target_topic_id: String,
    #[serde(rename = "replyIds")]
    reply_ids: Vec<String>,
    locale: String,
    title: String,
    slug: Option<String>,
    reason: String,
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
) -> Result<Vec<ForumTopicSplitCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        SPLIT_CANDIDATES_QUERY,
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
) -> Result<ForumTopicSplitReplyPage, ApiError> {
    // Operations on a whole thread need every reply, so walk the keyset cursor to
    // the end. The load is capped: a thread above the cap is refused, not truncated.
    let mut items = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let response: RepliesResponse = request(
            SPLIT_REPLIES_QUERY,
            RepliesVariables {
                topic_id: source_topic_id.clone(),
                locale: Some(locale.clone()),
                after: after.take(),
                per_page: ADMIN_REPLY_PAGE_SIZE,
            },
            token.clone(),
            tenant_slug.clone(),
        )
        .await?;
        items.extend(response.forum_replies.items);
        if items.len() > ADMIN_REPLY_LOAD_LIMIT {
            return Err(format!(
                "Topic has more than {ADMIN_REPLY_LOAD_LIMIT} replies and cannot be loaded for this operation"
            ));
        }
        match response.forum_replies.next_cursor {
            Some(cursor) => after = Some(cursor),
            None => return Ok(ForumTopicSplitReplyPage { items }),
        }
    }
}

pub async fn split_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicSplitCommand,
) -> Result<ForumTopicSplitReceipt, ApiError> {
    let response: SplitResponse = request(
        SPLIT_TOPIC_MUTATION,
        SplitVariables {
            source_topic_id: command.source_topic_id,
            input: SplitInput {
                operation_id: command.operation_id,
                target_topic_id: command.target_topic_id,
                reply_ids: command.reply_ids,
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
    Ok(response.split_forum_topic_replies)
}

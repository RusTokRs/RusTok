#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::topic_reply_range_model::{
    ForumReplyRangeMoveCandidate, ForumReplyRangeMoveCommand, ForumReplyRangeMoveReceipt,
};

pub type ApiError = String;

const CANDIDATES_QUERY: &str = "query ForumAdminReplyRangeCandidates($locale: String, $pagination: PaginationInput!) { forumTopics(locale: $locale, pagination: $pagination) { items { id locale title category_id: categoryId reply_count: replyCount } } }";
const MOVE_REPLY_RANGE_MUTATION: &str = "mutation ForumAdminMoveReplyRange($sourceTopicId: UUID!, $input: MoveForumTopicReplyRangeGraphqlInput!) { moveForumTopicReplyRange(sourceTopicId: $sourceTopicId, input: $input) { operation_id: operationId event_id: eventId source_topic_id: sourceTopicId target_topic_id: targetTopicId source_category_id: sourceCategoryId target_category_id: targetCategoryId actor_id: actorId reason source_start_position: sourceStartPosition source_end_position: sourceEndPosition target_start_position: targetStartPosition target_end_position: targetEndPosition moved_reply_count: movedReplyCount moved_published_reply_count: movedPublishedReplyCount source_resulting_published_reply_count: sourceResultingPublishedReplyCount target_resulting_published_reply_count: targetResultingPublishedReplyCount moved_solution_reply_id: movedSolutionReplyId source_resulting_solution_reply_id: sourceResultingSolutionReplyId target_resulting_solution_reply_id: targetResultingSolutionReplyId moved_at: movedAt } }";

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumReplyRangeMoveCandidate>,
}

#[derive(Debug, Deserialize)]
struct MoveResponse {
    #[serde(rename = "moveForumTopicReplyRange")]
    move_forum_topic_reply_range: ForumReplyRangeMoveReceipt,
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
struct MoveVariables {
    #[serde(rename = "sourceTopicId")]
    source_topic_id: String,
    input: MoveInput,
}

#[derive(Debug, Serialize)]
struct MoveInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "targetTopicId")]
    target_topic_id: String,
    #[serde(rename = "startPosition")]
    start_position: i64,
    #[serde(rename = "endPosition")]
    end_position: i64,
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
) -> Result<Vec<ForumReplyRangeMoveCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        CANDIDATES_QUERY,
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

pub async fn move_reply_range(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumReplyRangeMoveCommand,
) -> Result<ForumReplyRangeMoveReceipt, ApiError> {
    let response: MoveResponse = request(
        MOVE_REPLY_RANGE_MUTATION,
        MoveVariables {
            source_topic_id: command.source_topic_id,
            input: MoveInput {
                operation_id: command.operation_id,
                target_topic_id: command.target_topic_id,
                start_position: command.start_position,
                end_position: command.end_position,
                reason: command.reason,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.move_forum_topic_reply_range)
}

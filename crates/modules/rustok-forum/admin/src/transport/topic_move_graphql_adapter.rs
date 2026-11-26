use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::topic_move_model::{
    ForumTopicMoveCandidate, ForumTopicMoveCategory, ForumTopicMoveCommand, ForumTopicMoveReceipt,
};

pub type ApiError = String;

const MOVE_CANDIDATES_QUERY: &str = "query ForumAdminTopicMoveCandidates($locale: String, $perPage: Int, $pagination: PaginationInput) { forumTopics(locale: $locale, perPage: $perPage) { items { id title category_id: categoryId } } forumCategories(locale: $locale, pagination: $pagination) { items { id name } } }";
const MOVE_TOPIC_MUTATION: &str = "mutation ForumAdminMoveTopic($topicId: UUID!, $input: MoveForumTopicGraphqlInput!) { moveForumTopic(topicId: $topicId, input: $input) { operation_id: operationId event_id: eventId topic_id: topicId source_category_id: sourceCategoryId target_category_id: targetCategoryId actor_id: actorId reason published_reply_count: publishedReplyCount moved_at: movedAt } }";

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
    #[serde(rename = "forumCategories")]
    forum_categories: CategoryConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumTopicMoveCandidate>,
}

#[derive(Debug, Deserialize)]
struct CategoryConnection {
    items: Vec<ForumTopicMoveCategory>,
}

#[derive(Debug, Deserialize)]
struct MoveResponse {
    #[serde(rename = "moveForumTopic")]
    move_forum_topic: ForumTopicMoveReceipt,
}

#[derive(Debug, Serialize)]
struct CandidatesVariables {
    locale: Option<String>,
    #[serde(rename = "perPage")]
    per_page: i64,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct MoveVariables {
    #[serde(rename = "topicId")]
    topic_id: String,
    input: MoveInput,
}

#[derive(Debug, Serialize)]
struct MoveInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "targetCategoryId")]
    target_category_id: String,
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
) -> Result<(Vec<ForumTopicMoveCandidate>, Vec<ForumTopicMoveCategory>), ApiError> {
    let response: CandidatesResponse = request(
        MOVE_CANDIDATES_QUERY,
        CandidatesVariables {
            locale: Some(locale),
            per_page: 100,
            pagination: PaginationInput {
                offset: 0,
                limit: 100,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok((response.forum_topics.items, response.forum_categories.items))
}

pub async fn move_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicMoveCommand,
) -> Result<ForumTopicMoveReceipt, ApiError> {
    let response: MoveResponse = request(
        MOVE_TOPIC_MUTATION,
        MoveVariables {
            topic_id: command.topic_id,
            input: MoveInput {
                operation_id: command.operation_id,
                target_category_id: command.target_category_id,
                reason: command.reason,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.move_forum_topic)
}

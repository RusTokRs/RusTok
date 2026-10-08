use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::topic_merge_model::{
    ForumTopicMergeCandidate, ForumTopicMergeCommand, ForumTopicMergeReceipt,
};

pub type ApiError = String;

const MERGE_CANDIDATES_QUERY: &str = "query ForumAdminMergeCandidates($locale: String, $perPage: Int) { forumTopics(locale: $locale, perPage: $perPage) { items { id title category_id: categoryId reply_count: replyCount solution_reply_id: solutionReplyId } } }";
const MERGE_TOPIC_MUTATION: &str = "mutation ForumAdminMergeTopic($targetTopicId: UUID!, $input: MergeForumTopicGraphqlInput!) { mergeForumTopic(targetTopicId: $targetTopicId, input: $input) { operation_id: operationId event_id: eventId source_topic_id: sourceTopicId target_topic_id: targetTopicId category_id: categoryId actor_id: actorId reason moved_reply_count: movedReplyCount moved_published_reply_count: movedPublishedReplyCount resulting_published_reply_count: resultingPublishedReplyCount position_offset: positionOffset merged_at: mergedAt } }";
const MERGE_TOPIC_RESOLVING_SOLUTION_MUTATION: &str = "mutation ForumAdminMergeTopicResolvingSolution($targetTopicId: UUID!, $input: ResolveForumTopicMergeSolutionGraphqlInput!) { mergeForumTopicResolvingSolution(targetTopicId: $targetTopicId, input: $input) { selected_solution_reply_id: selectedSolutionReplyId merge { operation_id: operationId event_id: eventId source_topic_id: sourceTopicId target_topic_id: targetTopicId category_id: categoryId actor_id: actorId reason moved_reply_count: movedReplyCount moved_published_reply_count: movedPublishedReplyCount resulting_published_reply_count: resultingPublishedReplyCount position_offset: positionOffset merged_at: mergedAt } } }";

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumTopicMergeCandidate>,
}

#[derive(Debug, Deserialize)]
struct MergeResponse {
    #[serde(rename = "mergeForumTopic")]
    merge_forum_topic: ForumTopicMergeReceipt,
}

#[derive(Debug, Deserialize)]
struct ResolvedMergeResponse {
    #[serde(rename = "mergeForumTopicResolvingSolution")]
    merge_forum_topic_resolving_solution: ResolvedMerge,
}

#[derive(Debug, Deserialize)]
struct ResolvedMerge {
    merge: ForumTopicMergeReceipt,
}

#[derive(Debug, Serialize)]
struct CandidatesVariables {
    locale: Option<String>,
    #[serde(rename = "perPage")]
    per_page: i64,
}

#[derive(Debug, Serialize)]
struct MergeVariables<T> {
    #[serde(rename = "targetTopicId")]
    target_topic_id: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct MergeInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "sourceTopicId")]
    source_topic_id: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ResolvedMergeInput {
    #[serde(rename = "operationId")]
    operation_id: String,
    #[serde(rename = "sourceTopicId")]
    source_topic_id: String,
    #[serde(rename = "selectedSolutionReplyId")]
    selected_solution_reply_id: String,
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
) -> Result<Vec<ForumTopicMergeCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        MERGE_CANDIDATES_QUERY,
        CandidatesVariables {
            locale: Some(locale),
            per_page: 100,
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_topics.items)
}

pub async fn merge_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicMergeCommand,
) -> Result<ForumTopicMergeReceipt, ApiError> {
    if let Some(selected_solution_reply_id) = command.selected_solution_reply_id {
        let response: ResolvedMergeResponse = request(
            MERGE_TOPIC_RESOLVING_SOLUTION_MUTATION,
            MergeVariables {
                target_topic_id: command.target_topic_id,
                input: ResolvedMergeInput {
                    operation_id: command.operation_id,
                    source_topic_id: command.source_topic_id,
                    selected_solution_reply_id,
                    reason: command.reason,
                },
            },
            token,
            tenant_slug,
        )
        .await?;
        Ok(response.merge_forum_topic_resolving_solution.merge)
    } else {
        let response: MergeResponse = request(
            MERGE_TOPIC_MUTATION,
            MergeVariables {
                target_topic_id: command.target_topic_id,
                input: MergeInput {
                    operation_id: command.operation_id,
                    source_topic_id: command.source_topic_id,
                    reason: command.reason,
                },
            },
            token,
            tenant_slug,
        )
        .await?;
        Ok(response.merge_forum_topic)
    }
}

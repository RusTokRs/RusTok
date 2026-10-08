use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::topic_slug_rename_model::{
    ForumTopicSlugRenameCandidate, ForumTopicSlugRenameCommand, ForumTopicSlugRenameReceipt,
};

pub type ApiError = String;

const RENAME_CANDIDATES_QUERY: &str = "query ForumAdminTopicSlugRenameCandidates($locale: String, $perPage: Int) { forumTopics(locale: $locale, perPage: $perPage) { items { id title locale slug } } }";
const RENAME_TOPIC_SLUG_MUTATION: &str = "mutation ForumAdminRenameTopicSlug($topicId: UUID!, $input: RenameForumTopicSlugGraphqlInput!) { renameForumTopicSlug(topicId: $topicId, input: $input) { topic_id: topicId locale previous_slug: previousSlug slug previous_path: previousPath canonical { topic_id: topicId locale short_id: shortId slug path } alias_id: aliasId changed } }";

#[derive(Debug, Deserialize)]
struct CandidatesResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: CandidateConnection,
}

#[derive(Debug, Deserialize)]
struct CandidateConnection {
    items: Vec<ForumTopicSlugRenameCandidate>,
}

#[derive(Debug, Deserialize)]
struct RenameResponse {
    #[serde(rename = "renameForumTopicSlug")]
    rename_forum_topic_slug: ForumTopicSlugRenameReceipt,
}

#[derive(Debug, Serialize)]
struct CandidatesVariables {
    locale: Option<String>,
    #[serde(rename = "perPage")]
    per_page: i64,
}

#[derive(Debug, Serialize)]
struct RenameVariables {
    #[serde(rename = "topicId")]
    topic_id: String,
    input: RenameInput,
}

#[derive(Debug, Serialize)]
struct RenameInput {
    locale: String,
    slug: String,
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
) -> Result<Vec<ForumTopicSlugRenameCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        RENAME_CANDIDATES_QUERY,
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

pub async fn rename_topic_slug(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicSlugRenameCommand,
) -> Result<ForumTopicSlugRenameReceipt, ApiError> {
    let response: RenameResponse = request(
        RENAME_TOPIC_SLUG_MUTATION,
        RenameVariables {
            topic_id: command.topic_id,
            input: RenameInput {
                locale: command.locale,
                slug: command.slug,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.rename_forum_topic_slug)
}

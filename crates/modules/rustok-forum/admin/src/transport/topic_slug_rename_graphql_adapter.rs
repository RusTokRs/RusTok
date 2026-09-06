#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::topic_slug_rename_model::{
    ForumTopicSlugRenameCandidate, ForumTopicSlugRenameCommand, ForumTopicSlugRenameReceipt,
};

pub type ApiError = String;

const RENAME_CANDIDATES_QUERY: &str = "query ForumAdminTopicSlugRenameCandidates($locale: String, $pagination: PaginationInput!) { forumTopics(locale: $locale, pagination: $pagination) { items { id title locale slug } } }";
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
) -> Result<Vec<ForumTopicSlugRenameCandidate>, ApiError> {
    let response: CandidatesResponse = request(
        RENAME_CANDIDATES_QUERY,
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

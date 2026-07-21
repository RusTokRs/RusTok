#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{BlogModerationCommentList, BlogModerationStatus};

const BLOG_MODERATION_COMMENTS_QUERY: &str = "query BlogModerationComments($postId: UUID!, $locale: String) { post(id: $postId, locale: $locale) { moderationComments(locale: $locale, page: 1, perPage: 100) { total items { id effectiveLocale authorId contentPreview status parentCommentId createdAt } } } }";
const MODERATE_BLOG_COMMENT_MUTATION: &str = "mutation ModerateBlogComment($id: UUID!, $status: BlogCommentModerationStatus!, $locale: String) { moderateComment(id: $id, status: $status, locale: $locale) }";

#[derive(Debug, Deserialize)]
struct ModerationCommentsResponse {
    post: Option<ModerationPostPayload>,
}

#[derive(Debug, Deserialize)]
struct ModerationPostPayload {
    #[serde(rename = "moderationComments")]
    moderation_comments: BlogModerationCommentList,
}

#[derive(Debug, Deserialize)]
struct ModerateCommentResponse {
    #[serde(rename = "moderateComment")]
    moderate_comment: bool,
}

#[derive(Debug, Serialize)]
struct ModerationCommentsVariables {
    #[serde(rename = "postId")]
    post_id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct ModerateCommentVariables {
    id: String,
    status: String,
    locale: Option<String>,
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
) -> Result<T, GraphqlHttpError>
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
}

pub async fn fetch_comments(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    locale: Option<String>,
) -> Result<BlogModerationCommentList, GraphqlHttpError> {
    let response: ModerationCommentsResponse = request(
        BLOG_MODERATION_COMMENTS_QUERY,
        ModerationCommentsVariables { post_id, locale },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response
        .post
        .map(|post| post.moderation_comments)
        .unwrap_or_default())
}

pub async fn moderate_comment(
    token: Option<String>,
    tenant_slug: Option<String>,
    comment_id: String,
    status: BlogModerationStatus,
    locale: Option<String>,
) -> Result<bool, GraphqlHttpError> {
    let response: ModerateCommentResponse = request(
        MODERATE_BLOG_COMMENT_MUTATION,
        ModerateCommentVariables {
            id: comment_id,
            status: status.graphql_value().to_string(),
            locale,
        },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.moderate_comment)
}

pub fn is_contract_unavailable(error: &GraphqlHttpError) -> bool {
    let message = error.to_string();
    message.contains("Unknown field \"moderationComments\"")
        || message.contains("Unknown field \"moderateComment\"")
        || message.contains("Unknown type \"BlogCommentModerationStatus\"")
}

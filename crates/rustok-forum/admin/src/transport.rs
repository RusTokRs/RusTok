#![allow(dead_code)]

mod category_tree_graphql_adapter;
mod category_tree_rest_adapter;
mod graphql_adapter;
mod rest_adapter;

use rustok_graphql::GraphqlHttpError;
use std::str::FromStr;

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

pub type ApiError = String;

pub async fn fetch_category_tree(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    match category_tree_graphql_adapter::fetch_category_tree(
        token.clone(),
        tenant_slug.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(tree) => Ok(tree.into_flat_items()),
        Err(error) if should_fallback_to_rest(error.as_str()) => redact_rest_fallback(
            category_tree_rest_adapter::fetch_category_tree(token, tenant_slug, locale)
                .await
                .map(|tree| tree.into_flat_items()),
        ),
        Err(error) => Err(error),
    }
}

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    match graphql_adapter::fetch_categories(token.clone(), tenant_slug.clone(), locale.clone())
        .await
    {
        Ok(categories) => Ok(categories),
        Err(error) if should_fallback_to_rest(error.as_str()) => {
            redact_rest_fallback(rest_adapter::fetch_categories(token, tenant_slug, locale).await)
        }
        Err(error) => Err(error),
    }
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    match graphql_adapter::fetch_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(category) => Ok(category),
        Err(error) if should_fallback_to_rest(error.as_str()) => {
            redact_rest_fallback(rest_adapter::fetch_category(token, tenant_slug, id, locale).await)
        }
        Err(error) => Err(error),
    }
}

/// Execute category creation through exactly one write transport.
///
/// Retrying a failed GraphQL mutation over REST is unsafe because the GraphQL
/// write may have committed before its response was lost or rejected by the
/// client. Reads retain their compatibility fallback, but writes fail closed.
pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;
    let category =
        graphql_adapter::create_category(token.clone(), tenant_slug.clone(), draft).await?;

    move_category(
        token.clone(),
        tenant_slug.clone(),
        category.id.clone(),
        category.parent_id.clone(),
        requested_position,
    )
    .await?;
    fetch_category(token, tenant_slug, category.id, locale).await
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;
    let category = graphql_adapter::update_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        draft.clone(),
    )
    .await?;

    if category.position != draft.position {
        move_category(
            token.clone(),
            tenant_slug.clone(),
            id.clone(),
            category.parent_id,
            requested_position,
        )
        .await?;
        fetch_category(token, tenant_slug, id, locale).await
    } else {
        Ok(category)
    }
}

pub async fn move_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    parent_id: Option<String>,
    position: u32,
) -> Result<(), ApiError> {
    graphql_adapter::move_category(token, tenant_slug, id, parent_id, position).await
}

pub async fn reorder_category_siblings(
    token: Option<String>,
    tenant_slug: Option<String>,
    parent_id: Option<String>,
    ordered_category_ids: Vec<String>,
) -> Result<(), ApiError> {
    graphql_adapter::reorder_category_siblings(token, tenant_slug, parent_id, ordered_category_ids)
        .await
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    graphql_adapter::delete_category(token, tenant_slug, id).await
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    match graphql_adapter::fetch_topics(
        token.clone(),
        tenant_slug.clone(),
        locale.clone(),
        category_id.clone(),
    )
    .await
    {
        Ok(topics) => Ok(topics),
        Err(error) if should_fallback_to_rest(error.as_str()) => redact_rest_fallback(
            rest_adapter::fetch_topics(token, tenant_slug, locale, category_id).await,
        ),
        Err(error) => Err(error),
    }
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::fetch_topic(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(topic) => Ok(topic),
        Err(error) if should_fallback_to_rest(error.as_str()) => {
            redact_rest_fallback(rest_adapter::fetch_topic(token, tenant_slug, id, locale).await)
        }
        Err(error) => Err(error),
    }
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    graphql_adapter::create_topic(token, tenant_slug, draft).await
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    graphql_adapter::update_topic(token, tenant_slug, id, draft).await
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    graphql_adapter::delete_topic(token, tenant_slug, id).await
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    match graphql_adapter::fetch_replies(
        token.clone(),
        tenant_slug.clone(),
        topic_id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(replies) => Ok(replies),
        Err(error) if should_fallback_to_rest(error.as_str()) => redact_rest_fallback(
            rest_adapter::fetch_replies(token, tenant_slug, topic_id, locale).await,
        ),
        Err(error) => Err(error),
    }
}

fn should_fallback_to_rest(error: &str) -> bool {
    matches!(
        GraphqlHttpError::from_str(error),
        Ok(GraphqlHttpError::Network)
    )
}

fn redact_rest_fallback<T>(result: Result<T, String>) -> Result<T, String> {
    result.map_err(|_| "Forum REST fallback failed".to_string())
}

fn placement_position(position: i32) -> Result<u32, ApiError> {
    u32::try_from(position).map_err(|_| "Category position must be zero or greater".to_string())
}

#[cfg(test)]
mod tests {
    use super::{redact_rest_fallback, should_fallback_to_rest};

    const SOURCE: &str = include_str!("transport.rs");

    fn function_source(name: &str) -> &str {
        let marker = format!("pub async fn {name}(");
        let start = SOURCE
            .find(marker.as_str())
            .unwrap_or_else(|| panic!("missing transport function {name}"));
        let after_start = &SOURCE[start + marker.len()..];
        let end = after_start
            .find("\npub async fn ")
            .unwrap_or(after_start.len());
        &SOURCE[start..start + marker.len() + end]
    }

    #[test]
    fn forum_admin_writes_do_not_retry_through_rest() {
        for operation in [
            "create_category",
            "update_category",
            "move_category",
            "reorder_category_siblings",
            "delete_category",
            "create_topic",
            "update_topic",
            "delete_topic",
        ] {
            let source = function_source(operation);
            assert!(
                !source.contains("rest_adapter::"),
                "{operation} must not retry a possibly committed GraphQL write through REST"
            );
            assert!(
                source.contains("graphql_adapter::"),
                "{operation} must keep an explicit owner transport"
            );
        }
    }

    #[test]
    fn forum_admin_reads_guard_and_redact_compatibility_fallbacks() {
        for operation in [
            "fetch_category_tree",
            "fetch_categories",
            "fetch_category",
            "fetch_topics",
            "fetch_topic",
            "fetch_replies",
        ] {
            let source = function_source(operation);
            assert!(
                source.contains("should_fallback_to_rest"),
                "{operation} must classify the GraphQL failure before using REST"
            );
            assert!(
                source.contains("redact_rest_fallback"),
                "{operation} must redact REST response and network details"
            );
            assert!(
                source.contains("rest_adapter::")
                    || source.contains("category_tree_rest_adapter::"),
                "{operation} should retain the network-only compatibility fallback"
            );
            assert!(
                source.contains("Err(error) => Err(error)"),
                "{operation} must preserve non-network GraphQL errors"
            );
        }
    }

    #[test]
    fn forum_admin_read_fallback_is_network_only() {
        assert!(should_fallback_to_rest("Network error"));
        assert!(!should_fallback_to_rest("Unauthorized"));
        assert!(!should_fallback_to_rest("GraphQL error: permission denied"));
        assert!(!should_fallback_to_rest(
            "Http error: 503 Service Unavailable"
        ));
        assert!(!should_fallback_to_rest("unknown adapter error"));
    }

    #[test]
    fn forum_admin_rest_fallback_errors_are_publicly_redacted() {
        let secret = "HTTP 500: database password=private host=internal";
        let error = redact_rest_fallback::<()>(Err(secret.to_string()))
            .expect_err("REST fallback failure must stay an error");
        assert_eq!(error, "Forum REST fallback failed");
        assert!(!error.contains(secret));
    }
}

#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::CategoryTreeResponse;

use super::ApiError;

const MAX_CATEGORY_TREE_DEPTH: u8 = 16;

#[derive(Debug, Deserialize)]
struct CategoryTreeData {
    #[serde(rename = "forumCategoryTree")]
    forum_category_tree: CategoryTreeResponse,
}

#[derive(Debug, Serialize)]
struct CategoryTreeVariables {
    locale: Option<String>,
}

pub async fn fetch_category_tree(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<CategoryTreeResponse, ApiError> {
    let query = format!(
        "query ForumAdminCategoryTree($locale: String) {{ forumCategoryTree(locale: $locale) {{ total_nodes: totalNodes max_depth: maxDepth roots {{ {} }} }} }}",
        category_node_selection(MAX_CATEGORY_TREE_DEPTH)
    );
    let response: CategoryTreeData = execute_graphql(
        graphql_url().as_str(),
        GraphqlRequest::new(
            query,
            Some(CategoryTreeVariables {
                locale: Some(locale.clone()),
            }),
        ),
        token,
        tenant_slug,
        Some(locale),
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.forum_category_tree)
}

fn category_node_selection(remaining_depth: u8) -> String {
    let fields = "id parent_id: parentId depth position requested_locale: requestedLocale effective_locale: effectiveLocale name slug description icon color moderated allows_topics: allowsTopics archived_at: archivedAt is_archived: isArchived topic_count: topicCount reply_count: replyCount";
    if remaining_depth == 0 {
        return fields.to_string();
    }
    format!(
        "{fields} children {{ {} }}",
        category_node_selection(remaining_depth - 1)
    )
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

#[cfg(test)]
mod tests {
    use super::{category_node_selection, MAX_CATEGORY_TREE_DEPTH};

    #[test]
    fn category_tree_query_covers_owner_depth_bound() {
        let selection = category_node_selection(MAX_CATEGORY_TREE_DEPTH);
        assert_eq!(selection.matches("children {").count(), 16);
        assert!(selection.contains("archived_at: archivedAt"));
        assert!(selection.contains("allows_topics: allowsTopics"));
    }
}

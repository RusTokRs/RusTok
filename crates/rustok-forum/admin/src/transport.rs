mod category_tree_graphql_adapter;
mod graphql_adapter;
mod native_server_support;
mod reply_create_native_server_adapter;
mod topic_fork_graphql_adapter;
mod topic_merge_graphql_adapter;
mod topic_merge_native_server_adapter;
mod topic_reply_range_graphql_adapter;
mod topic_slug_rename_graphql_adapter;
mod topic_split_graphql_adapter;

use rustok_ui_transport::{UiTransportPath, execute_selected_transport};

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyDraft, ReplyListItem, TopicDetail,
    TopicDraft, TopicListItem,
};
use crate::topic_fork_model::{
    ForumTopicForkCandidate, ForumTopicForkCommand, ForumTopicForkReceipt,
    ForumTopicForkReplyPage,
};
use crate::topic_merge_model::{
    ForumTopicMergeCandidate, ForumTopicMergeCommand, ForumTopicMergeReceipt,
};
use crate::topic_reply_range_model::{
    ForumReplyRangeMoveCandidate, ForumReplyRangeMoveCommand, ForumReplyRangeMoveReceipt,
};
use crate::topic_slug_rename_model::{
    ForumTopicSlugRenameCandidate, ForumTopicSlugRenameCommand, ForumTopicSlugRenameReceipt,
};
use crate::topic_split_model::{
    ForumTopicSplitCandidate, ForumTopicSplitCommand, ForumTopicSplitReceipt,
    ForumTopicSplitReplyPage,
};

pub type ApiError = String;

fn selected_admin_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_category_tree(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    category_tree_graphql_adapter::fetch_category_tree(token, tenant_slug, locale)
        .await
        .map(|tree| tree.into_flat_items())
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    graphql_adapter::fetch_category(token, tenant_slug, id, locale).await
}

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
    graphql_adapter::fetch_topics(token, tenant_slug, locale, category_id).await
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    graphql_adapter::fetch_topic(token, tenant_slug, id, locale).await
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
    graphql_adapter::fetch_replies(token, tenant_slug, topic_id, locale).await
}

pub async fn create_reply(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    draft: ReplyDraft,
) -> Result<ReplyListItem, ApiError> {
    let native_topic_id = topic_id.clone();
    let native_draft = draft.clone();
    execute_selected_transport(
        "forum_reply_create_admin",
        selected_admin_transport_path(),
        move || {
            reply_create_native_server_adapter::create_reply_native(native_topic_id, native_draft)
        },
        move || graphql_adapter::create_reply(token, tenant_slug, topic_id, draft),
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn fetch_topic_merge_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumTopicMergeCandidate>, ApiError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "forum_topic_merge_admin",
        selected_admin_transport_path(),
        move || {
            topic_merge_native_server_adapter::fetch_topic_merge_candidates_native(native_locale)
        },
        move || topic_merge_graphql_adapter::fetch_candidates(token, tenant_slug, locale),
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn merge_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicMergeCommand,
) -> Result<ForumTopicMergeReceipt, ApiError> {
    let native_command = command.clone();
    execute_selected_transport(
        "forum_topic_merge_admin",
        selected_admin_transport_path(),
        move || topic_merge_native_server_adapter::merge_topic_native(native_command),
        move || topic_merge_graphql_adapter::merge_topic(token, tenant_slug, command),
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn fetch_topic_slug_rename_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumTopicSlugRenameCandidate>, ApiError> {
    topic_slug_rename_graphql_adapter::fetch_candidates(token, tenant_slug, locale).await
}

pub async fn rename_topic_slug(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicSlugRenameCommand,
) -> Result<ForumTopicSlugRenameReceipt, ApiError> {
    topic_slug_rename_graphql_adapter::rename_topic_slug(token, tenant_slug, command).await
}

pub async fn fetch_topic_fork_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumTopicForkCandidate>, ApiError> {
    topic_fork_graphql_adapter::fetch_candidates(token, tenant_slug, locale).await
}

pub async fn fetch_topic_fork_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    source_topic_id: String,
    locale: String,
) -> Result<ForumTopicForkReplyPage, ApiError> {
    topic_fork_graphql_adapter::fetch_replies(token, tenant_slug, source_topic_id, locale).await
}

pub async fn fork_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicForkCommand,
) -> Result<ForumTopicForkReceipt, ApiError> {
    topic_fork_graphql_adapter::fork_topic(token, tenant_slug, command).await
}

pub async fn fetch_reply_range_move_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumReplyRangeMoveCandidate>, ApiError> {
    topic_reply_range_graphql_adapter::fetch_candidates(token, tenant_slug, locale).await
}

pub async fn move_reply_range(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumReplyRangeMoveCommand,
) -> Result<ForumReplyRangeMoveReceipt, ApiError> {
    topic_reply_range_graphql_adapter::move_reply_range(token, tenant_slug, command).await
}

pub async fn fetch_topic_split_candidates(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<ForumTopicSplitCandidate>, ApiError> {
    topic_split_graphql_adapter::fetch_candidates(token, tenant_slug, locale).await
}

pub async fn fetch_topic_split_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    source_topic_id: String,
    locale: String,
) -> Result<ForumTopicSplitReplyPage, ApiError> {
    topic_split_graphql_adapter::fetch_replies(token, tenant_slug, source_topic_id, locale).await
}

pub async fn split_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ForumTopicSplitCommand,
) -> Result<ForumTopicSplitReceipt, ApiError> {
    topic_split_graphql_adapter::split_topic(token, tenant_slug, command).await
}

fn placement_position(position: i32) -> Result<u32, ApiError> {
    u32::try_from(position).map_err(|_| "Category position must be zero or greater".to_string())
}

#[cfg(test)]
mod tests {
    const SOURCE: &str = include_str!("transport.rs");

    fn function_source(name: &str) -> &str {
        let marker = format!("pub async fn {name}(");
        let start = SOURCE
            .find(marker.as_str())
            .unwrap_or_else(|| panic!("missing transport function {name}"));
        let after_start = &SOURCE[start + marker.len()..];
        let end = ["\npub async fn ", "\nfn ", "\n#[cfg(test)]"]
            .into_iter()
            .filter_map(|next| after_start.find(next))
            .min()
            .unwrap_or(after_start.len());
        &SOURCE[start..start + marker.len() + end]
    }

    #[test]
    fn established_forum_admin_operations_keep_one_explicit_graphql_transport() {
        for operation in [
            "fetch_category_tree",
            "fetch_category",
            "create_category",
            "update_category",
            "move_category",
            "delete_category",
            "fetch_topics",
            "fetch_topic",
            "create_topic",
            "update_topic",
            "delete_topic",
            "fetch_replies",
        ] {
            let source = function_source(operation);
            assert!(!source.contains("rest_adapter"));
            assert!(!source.contains("native_server_adapter"));
            assert!(
                source.contains("graphql_adapter::")
                    || source.contains("category_tree_graphql_adapter::"),
                "{operation} must keep one explicit GraphQL owner transport"
            );
        }
    }

    #[test]
    fn reply_create_selects_native_or_graphql_without_fallback() {
        let source = function_source("create_reply");
        assert!(source.contains("execute_selected_transport"));
        assert!(source.contains("reply_create_native_server_adapter::"));
        assert!(source.contains("graphql_adapter::create_reply"));
        assert!(!source.contains("or_else"));
        assert!(!source.contains("fallback"));
    }

    #[test]
    fn topic_merge_selects_native_or_graphql_without_fallback() {
        for operation in ["fetch_topic_merge_candidates", "merge_topic"] {
            let source = function_source(operation);
            assert!(source.contains("execute_selected_transport"));
            assert!(source.contains("topic_merge_native_server_adapter::"));
            assert!(source.contains("topic_merge_graphql_adapter::"));
            assert!(!source.contains("or_else"));
            assert!(!source.contains("fallback"));
        }

        assert!(SOURCE.contains("cfg(any(feature = \"ssr\", feature = \"hydrate\"))"));
        assert!(SOURCE.contains("UiTransportPath::NativeServer"));
        assert!(SOURCE.contains("UiTransportPath::Graphql"));
    }

    #[test]
    fn topic_slug_rename_uses_the_update_graphql_transport_without_fallback() {
        for operation in ["fetch_topic_slug_rename_candidates", "rename_topic_slug"] {
            let source = function_source(operation);
            assert!(source.contains("topic_slug_rename_graphql_adapter::"));
            assert!(!source.contains("native_server_adapter"));
            assert!(!source.contains("execute_selected_transport"));
            assert!(!source.contains("fallback"));
        }
    }

    #[test]
    fn topic_fork_uses_the_manager_graphql_transport_without_fallback() {
        for operation in [
            "fetch_topic_fork_candidates",
            "fetch_topic_fork_replies",
            "fork_topic",
        ] {
            let source = function_source(operation);
            assert!(source.contains("topic_fork_graphql_adapter::"));
            assert!(!source.contains("native_server_adapter"));
            assert!(!source.contains("execute_selected_transport"));
            assert!(!source.contains("fallback"));
        }
    }

    #[test]
    fn reply_range_move_uses_the_manager_graphql_transport_without_fallback() {
        for operation in ["fetch_reply_range_move_candidates", "move_reply_range"] {
            let source = function_source(operation);
            assert!(source.contains("topic_reply_range_graphql_adapter::"));
            assert!(!source.contains("native_server_adapter"));
            assert!(!source.contains("execute_selected_transport"));
            assert!(!source.contains("fallback"));
        }
    }

    #[test]
    fn topic_split_uses_the_manager_graphql_transport_without_fallback() {
        for operation in [
            "fetch_topic_split_candidates",
            "fetch_topic_split_replies",
            "split_topic",
        ] {
            let source = function_source(operation);
            assert!(source.contains("topic_split_graphql_adapter::"));
            assert!(!source.contains("native_server_adapter"));
            assert!(!source.contains("execute_selected_transport"));
            assert!(!source.contains("fallback"));
        }
    }
}

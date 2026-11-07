pub(crate) mod creation_journal;
mod graphql_adapter;
mod moderation_adapter;
mod native_server_adapter;
mod widget;
pub use widget::{
    load_blog_page_builder_widget_property_schema, preview_blog_page_builder_widget,
    validate_blog_page_builder_widget_properties,
};

use crate::core::BlogPostAdminSort;
use crate::core::creation::CreationScope;
use crate::model::{
    BlogCategoryDetail, BlogCategoryDraft, BlogCategoryOption, BlogCategoryUpdate,
    BlogModerationCommentList, BlogModerationStatus, BlogPostDetail, BlogPostDraft, BlogPostList,
    BlogPostUpdate, BlogPreviewToken,
};
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};

pub type ApiError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<Vec<BlogCategoryOption>, ApiError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/categories",
        selected_transport_path(),
        move || native_server_adapter::fetch_categories(native_locale),
        move || graphql_adapter::fetch_categories(token, tenant_slug, locale),
    )
    .await
}

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    after: Option<String>,
    sort: BlogPostAdminSort,
) -> Result<BlogPostList, ApiError> {
    let native_locale = locale.clone();
    let native_after = after.clone();
    execute_selected_transport(
        "blog/admin/posts",
        selected_transport_path(),
        move || native_server_adapter::fetch_posts(native_locale, native_after, sort),
        move || graphql_adapter::fetch_posts(token, tenant_slug, locale, after, sort),
    )
    .await
}

pub fn is_posts_contract_unavailable(error: &ApiError) -> bool {
    error.failed_path == UiTransportPath::Graphql
        && error
            .graphql_error
            .as_deref()
            .is_some_and(graphql_adapter::is_posts_contract_unavailable_message)
}

pub async fn fetch_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ApiError> {
    let native_id = id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/post",
        selected_transport_path(),
        move || native_server_adapter::fetch_post(native_id, native_locale),
        move || graphql_adapter::fetch_post(token, tenant_slug, id, locale),
    )
    .await
}

pub(crate) async fn fetch_creation_scope(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CreationScope, ApiError> {
    execute_selected_transport(
        "blog/admin/creation-scope",
        selected_transport_path(),
        native_server_adapter::fetch_creation_scope,
        move || graphql_adapter::fetch_creation_scope(token, tenant_slug),
    )
    .await
}

pub(crate) async fn create_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    scope: CreationScope,
    draft: BlogPostDraft,
) -> Result<crate::model::BlogPostCreationOutcome, ApiError> {
    let native_draft = draft.clone();
    execute_selected_transport(
        "blog/admin/create-post",
        selected_transport_path(),
        move || native_server_adapter::create_post(scope, native_draft),
        move || graphql_adapter::create_post(token, tenant_slug, scope, draft),
    )
    .await
}

pub async fn update_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    update: BlogPostUpdate,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    let native_update = update.clone();
    execute_selected_transport(
        "blog/admin/update-post",
        selected_transport_path(),
        move || native_server_adapter::update_post(native_id, native_update),
        move || graphql_adapter::update_post(token, tenant_slug, id, update),
    )
    .await
}

pub async fn publish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/publish-post",
        selected_transport_path(),
        move || native_server_adapter::publish_post(native_id),
        move || graphql_adapter::publish_post(token, tenant_slug, id),
    )
    .await
}

pub async fn unpublish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/unpublish-post",
        selected_transport_path(),
        move || native_server_adapter::unpublish_post(native_id),
        move || graphql_adapter::unpublish_post(token, tenant_slug, id),
    )
    .await
}

pub async fn pin_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/pin-post",
        selected_transport_path(),
        move || native_server_adapter::pin_post(native_id),
        move || graphql_adapter::pin_post(token, tenant_slug, id),
    )
    .await
}

pub async fn unpin_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/unpin-post",
        selected_transport_path(),
        move || native_server_adapter::unpin_post(native_id),
        move || graphql_adapter::unpin_post(token, tenant_slug, id),
    )
    .await
}

pub async fn archive_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/archive-post",
        selected_transport_path(),
        move || native_server_adapter::archive_post(native_id),
        move || graphql_adapter::archive_post(token, tenant_slug, id),
    )
    .await
}

pub async fn restore_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/restore-post",
        selected_transport_path(),
        move || native_server_adapter::restore_post(native_id),
        move || graphql_adapter::restore_post(token, tenant_slug, id),
    )
    .await
}

pub async fn delete_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/delete-post",
        selected_transport_path(),
        move || native_server_adapter::delete_post(native_id),
        move || graphql_adapter::delete_post(token, tenant_slug, id),
    )
    .await
}

/// Mints a draft preview token for a post. The plaintext secret is returned
/// exactly once; the backend keeps only its SHA-256 digest.
pub async fn create_preview_token(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    ttl_hours: Option<i64>,
) -> Result<BlogPreviewToken, ApiError> {
    let native_post_id = post_id.clone();
    execute_selected_transport(
        "blog/admin/create-preview-token",
        selected_transport_path(),
        move || native_server_adapter::create_preview_token(native_post_id, ttl_hours),
        move || graphql_adapter::create_preview_token(token, tenant_slug, post_id, ttl_hours),
    )
    .await
}

/// Full category projection for the category management workspace. Pages
/// through the owner list (100 per page) like the posts options loader.
pub async fn fetch_category_details(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<Vec<BlogCategoryDetail>, ApiError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/category-details",
        selected_transport_path(),
        move || native_server_adapter::fetch_category_details(native_locale),
        move || graphql_adapter::fetch_category_details(token, tenant_slug, locale),
    )
    .await
}

/// Creates a category and returns the new id.
pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BlogCategoryDraft,
) -> Result<String, ApiError> {
    let native_draft = draft.clone();
    execute_selected_transport(
        "blog/admin/create-category",
        selected_transport_path(),
        move || native_server_adapter::create_category(native_draft),
        move || graphql_adapter::create_category(token, tenant_slug, draft),
    )
    .await
}

/// Updates a category's localized copy and returns the refreshed projection.
pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    update: BlogCategoryUpdate,
) -> Result<BlogCategoryDetail, ApiError> {
    let native_id = id.clone();
    let native_update = update.clone();
    execute_selected_transport(
        "blog/admin/update-category",
        selected_transport_path(),
        move || native_server_adapter::update_category(native_id, native_update),
        move || graphql_adapter::update_category(token, tenant_slug, id, update),
    )
    .await
}

/// Deletes a category. The owner fails closed unless the host composed the
/// Taxonomy capability cleanup; posts fall back to the uncategorized state.
pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/delete-category",
        selected_transport_path(),
        move || native_server_adapter::delete_category(native_id),
        move || graphql_adapter::delete_category(token, tenant_slug, id),
    )
    .await
}

pub async fn fetch_moderation_comments(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    locale: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<BlogModerationCommentList, ApiError> {
    let native_post_id = post_id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/moderation-comments",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_moderation_comments(
                native_post_id,
                native_locale,
                page,
                per_page,
            )
        },
        move || {
            moderation_adapter::fetch_comments(token, tenant_slug, post_id, locale, page, per_page)
        },
    )
    .await
}

pub async fn moderate_comment(
    token: Option<String>,
    tenant_slug: Option<String>,
    comment_id: String,
    command_id: String,
    status: BlogModerationStatus,
    locale: Option<String>,
) -> Result<bool, ApiError> {
    let native_comment_id = comment_id.clone();
    let native_command_id = command_id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/moderate-comment",
        selected_transport_path(),
        move || {
            native_server_adapter::moderate_comment(
                native_comment_id,
                native_command_id,
                status,
                native_locale,
            )
        },
        move || {
            moderation_adapter::moderate_comment(
                token,
                tenant_slug,
                comment_id,
                command_id,
                status,
                locale,
            )
        },
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BlogAdminContractUnavailable {
    Posts,
    Moderation,
}

#[allow(dead_code)]
pub fn blog_admin_contract_unavailable(error: &ApiError) -> Option<BlogAdminContractUnavailable> {
    if is_posts_contract_unavailable(error) {
        Some(BlogAdminContractUnavailable::Posts)
    } else if is_moderation_contract_unavailable(error) {
        Some(BlogAdminContractUnavailable::Moderation)
    } else {
        None
    }
}

pub fn is_moderation_contract_unavailable(error: &ApiError) -> bool {
    error.failed_path == UiTransportPath::Graphql
        && error
            .graphql_error
            .as_deref()
            .is_some_and(moderation_adapter::is_contract_unavailable_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_selects_graphql_without_fallback() {
        #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
        #[cfg(any(feature = "ssr", feature = "hydrate"))]
        assert_eq!(selected_transport_path(), UiTransportPath::NativeServer);
    }

    #[test]
    fn contract_unavailable_classifies_typed_errors() {
        let posts_err = UiTransportError::graphql(
            "blog/admin/posts",
            "CONTRACT_UNAVAILABLE: posts schema missing",
        );
        assert!(is_posts_contract_unavailable(&posts_err));
        assert_eq!(
            blog_admin_contract_unavailable(&posts_err),
            Some(BlogAdminContractUnavailable::Posts)
        );

        let mod_err = UiTransportError::graphql(
            "blog/admin/moderation",
            "Unknown field \"moderationComments\"",
        );
        assert!(is_moderation_contract_unavailable(&mod_err));
        assert_eq!(
            blog_admin_contract_unavailable(&mod_err),
            Some(BlogAdminContractUnavailable::Moderation)
        );

        let unrelated_err =
            UiTransportError::graphql("blog/admin/posts", "Network connection reset");
        assert!(!is_posts_contract_unavailable(&unrelated_err));
        assert_eq!(blog_admin_contract_unavailable(&unrelated_err), None);
    }
}

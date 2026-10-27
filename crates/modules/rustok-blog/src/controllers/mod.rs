use anyhow::Context;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::{Router, http::StatusCode};
use rustok_api::{HostRuntimeContext, SharedStaticModuleSettingsReader};
use rustok_comments::CommentsThreadPort;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::services::MAX_POST_ARTICLE_BYTES;
use crate::{CategoryService, CommentService};

/// Upper bound of a Blog HTTP request body.
///
/// The largest legitimate payload is one rich-text article, which the domain
/// separately bounds at [`MAX_POST_ARTICLE_BYTES`]; the transport limit leaves
/// room for JSON framing, metadata and the other input fields, and rejects an
/// oversized body while it is still a stream instead of after it has been read
/// and parsed.
const MAX_BLOG_REQUEST_BODY_BYTES: usize = 2 * MAX_POST_ARTICLE_BYTES;

/// Bulk endpoints carry at most 100 UUIDs, so a few kilobytes is a legitimate
/// body; the tight limit means a huge `post_ids` array is refused before the
/// service-level bound is reached, not after it is deserialized.
const MAX_BLOG_BULK_BODY_BYTES: usize = 64 * 1024;

pub mod categories;
#[cfg(feature = "comment-assets")]
mod comment_assets;
pub mod comments;
pub mod openapi;
pub mod posts;
pub mod tags;

#[derive(Clone)]
pub struct BlogHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
    settings_reader: Option<SharedStaticModuleSettingsReader>,
    category_delete_cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
}

impl BlogHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn category_service(&self) -> CategoryService {
        CategoryService::new(self.db_clone(), self.event_bus())
            .with_category_delete_cleanup(self.category_delete_cleanup.clone())
    }

    fn comment_service(&self) -> CommentService {
        CommentService::from_runtime_capabilities(
            self.db_clone(),
            self.comments_thread_port.clone(),
            self.settings_reader.clone(),
        )
    }
}

impl BlogHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("blog HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        let category_delete_cleanup = runtime
            .shared_get::<Arc<dyn TaxonomyCategoryDeleteCleanupPort>>()
            .context(
                "blog HTTP routes require Taxonomy Category delete cleanup in HostRuntimeContext",
            )?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            comments_thread_port: runtime.shared_get::<Arc<dyn CommentsThreadPort>>(),
            settings_reader: runtime.shared_get::<SharedStaticModuleSettingsReader>(),
            category_delete_cleanup,
        })
    }
}

pub(super) async fn ensure_blog_module_enabled(
    runtime: &BlogHttpRuntime,
    tenant_id: Uuid,
) -> HttpResult<()> {
    match rustok_api::is_tenant_module_enabled(&runtime.db_clone(), tenant_id, "blog").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpError::new(
            StatusCode::FORBIDDEN,
            "MODULE_NOT_ENABLED",
            "Module 'blog' is not enabled for this tenant",
        )),
        Err(_) => Err(HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_SERVER_ERROR",
            "The Blog operation could not be completed",
        )),
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let state = BlogHttpRuntime::from_host(runtime)?;
    let router = Router::new()
        .route(
            "/api/blog/posts",
            get(posts::list_posts).post(posts::create_post),
        )
        .route(
            "/api/blog/posts/{id}",
            get(posts::get_post)
                .put(posts::update_post)
                .delete(posts::delete_post),
        )
        .route("/api/blog/posts/{id}/publish", post(posts::publish_post))
        .route(
            "/api/blog/posts/{id}/unpublish",
            post(posts::unpublish_post),
        )
        .route("/api/blog/posts/{id}/archive", post(posts::archive_post))
        .route("/api/blog/posts/{id}/restore", post(posts::restore_post))
        .route("/api/blog/posts/{id}/pin", post(posts::pin_post))
        .route("/api/blog/posts/{id}/unpin", post(posts::unpin_post))
        .route("/api/blog/preview", get(posts::preview_post))
        .route(
            "/api/blog/categories",
            get(categories::list_categories).post(categories::create_category),
        )
        .route(
            "/api/blog/categories/{id}",
            get(categories::get_category)
                .put(categories::update_category)
                .delete(categories::delete_category),
        )
        .route(
            "/api/blog/categories/{id}/move",
            post(categories::move_category),
        )
        .route(
            "/api/blog/comments/{id}/moderate",
            post(comments::moderate_comment),
        )
        .route(
            "/api/blog/posts/{id}/comments",
            get(comments::list_comments).post(comments::create_comment),
        )
        .route(
            "/api/blog/comments/{id}",
            get(comments::get_comment)
                .put(comments::update_comment)
                .delete(comments::delete_comment),
        )
        .route(
            "/api/blog/tags",
            get(tags::list_tags).post(tags::create_tag),
        )
        .route(
            "/api/blog/tags/{id}",
            get(tags::get_tag).put(tags::update_tag).delete(tags::delete_tag),
        )
        .route(
            "/api/blog/posts/{id}/preview-tokens",
            get(posts::list_preview_tokens).post(posts::create_preview_token),
        )
        .route(
            "/api/blog/preview-tokens/{id}",
            axum::routing::delete(posts::revoke_preview_token),
        )
        .layer(DefaultBodyLimit::max(MAX_BLOG_REQUEST_BODY_BYTES))
        .merge(
            Router::new()
                .route(
                    "/api/blog/posts/bulk/publish",
                    post(posts::bulk_publish_posts),
                )
                .route(
                    "/api/blog/posts/bulk/unpublish",
                    post(posts::bulk_unpublish_posts),
                )
                .route(
                    "/api/blog/posts/bulk/archive",
                    post(posts::bulk_archive_posts),
                )
                .route(
                    "/api/blog/posts/bulk/restore",
                    post(posts::bulk_restore_posts),
                )
                .route(
                    "/api/blog/posts/bulk/delete",
                    post(posts::bulk_delete_posts),
                )
                .layer(DefaultBodyLimit::max(MAX_BLOG_BULK_BODY_BYTES)),
        )
        .with_state(state);
    #[cfg(feature = "comment-assets")]
    let router = router.merge(comment_assets::router());
    Ok(router)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blog_http_runtime_exposes_comments_port_selection() {
        let selector: fn(&BlogHttpRuntime) -> CommentService = BlogHttpRuntime::comment_service;
        let _ = selector;
    }
}

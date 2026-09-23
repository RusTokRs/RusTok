use anyhow::Context;
use axum::{Router, http::StatusCode};
use axum::routing::{get, post};
use rustok_api::HostRuntimeContext;
use rustok_comments_api::CommentsThreadPort;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{CategoryService, CommentService};

pub mod categories;
#[cfg(feature = "comment-assets")]
mod comment_assets;
pub mod comments;
pub mod openapi;
pub mod posts;

#[derive(Clone)]
pub struct BlogHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
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
        CommentService::from_optional_comments_thread_port(
            self.db_clone(),
            self.comments_thread_port.clone(),
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

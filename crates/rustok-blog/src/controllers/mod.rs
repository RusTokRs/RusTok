use anyhow::Context;
use axum::Router;
use axum::routing::{get, post};
use rustok_api::HostRuntimeContext;
use rustok_comments::CommentsThreadPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::CommentService;

pub mod categories;
#[cfg(feature = "comment-assets")]
mod comment_assets;
pub mod comments;
pub mod posts;

#[derive(Clone)]
pub struct BlogHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
}

impl BlogHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn comment_service(&self) -> CommentService {
        if let Some(comments_thread_port) = self.comments_thread_port.clone() {
            CommentService::with_comments_thread_port(self.db_clone(), comments_thread_port)
        } else {
            CommentService::new(self.db_clone(), self.event_bus())
        }
    }
}

impl BlogHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("blog HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            comments_thread_port: runtime.shared_get::<Arc<dyn CommentsThreadPort>>(),
        })
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

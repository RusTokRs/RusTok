use anyhow::Context;
use axum::Router;
use axum::routing::{get, post};
use rustok_api::HostRuntimeContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

pub mod categories;
pub mod comments;
pub mod posts;

#[derive(Clone)]
pub struct BlogHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl BlogHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
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
        })
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let state = BlogHttpRuntime::from_host(runtime)?;
    Ok(Router::new()
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
        .with_state(state))
}

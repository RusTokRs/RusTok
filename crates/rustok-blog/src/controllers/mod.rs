use axum::routing::{get, post};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

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

impl axum::extract::FromRef<AppContext> for BlogHttpRuntime {
    fn from_ref(input: &AppContext) -> Self {
        let transport = Arc::new(OutboxTransport::new(input.db.clone()));
        Self {
            db: input.db.clone(),
            event_bus: TransactionalEventBus::new(transport),
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/blog")
        .add("/posts", get(posts::list_posts).post(posts::create_post))
        .add(
            "/posts/{id}",
            get(posts::get_post)
                .put(posts::update_post)
                .delete(posts::delete_post),
        )
        .add("/posts/{id}/publish", post(posts::publish_post))
        .add("/posts/{id}/unpublish", post(posts::unpublish_post))
        .add("/comments/{id}/moderate", post(comments::moderate_comment))
}

use loco_rs::controller::Routes;
use axum::routing::{get, post};

pub mod posts;

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/blog")
        .add("/health", get(super::health::health))
        .add("/posts", get(posts::list_posts).post(posts::create_post))
        .add(
            "/posts/{id}",
            get(posts::get_post)
                .put(posts::update_post)
                .delete(posts::delete_post),
        )
        .add("/posts/{id}/publish", post(posts::publish_post))
        .add("/posts/{id}/unpublish", post(posts::unpublish_post))
}

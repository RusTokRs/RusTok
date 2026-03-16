use loco_rs::controller::Routes;
use axum::routing::{get, post};

pub mod categories;
pub mod replies;
pub mod topics;

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/forum")
        .add("/health", get(super::health::health))
        .add(
            "/categories",
            get(categories::list_categories).post(categories::create_category),
        )
        .add(
            "/categories/{id}",
            get(categories::get_category)
                .put(categories::update_category)
                .delete(categories::delete_category),
        )
        .add(
            "/topics",
            get(topics::list_topics).post(topics::create_topic),
        )
        .add(
            "/topics/{id}",
            get(topics::get_topic)
                .put(topics::update_topic)
                .delete(topics::delete_topic),
        )
        .add(
            "/topics/{id}/replies",
            get(replies::list_replies).post(replies::create_reply),
        )
        .add(
            "/replies/{id}",
            get(replies::get_reply)
                .put(replies::update_reply)
                .delete(replies::delete_reply),
        )
}

use axum::routing::get;
use loco_rs::controller::Routes;

pub mod categories;
pub mod replies;
pub mod topics;
pub mod users;
pub mod widgets;

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/forum")
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
            "/categories/{id}/subscription",
            axum::routing::post(categories::subscribe_category)
                .delete(categories::unsubscribe_category),
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
            "/topics/{topic_id}/solution/{reply_id}",
            axum::routing::post(topics::mark_topic_solution),
        )
        .add(
            "/topics/{topic_id}/solution",
            axum::routing::delete(topics::clear_topic_solution),
        )
        .add(
            "/topics/{topic_id}/vote/{value}",
            axum::routing::post(topics::set_topic_vote),
        )
        .add(
            "/topics/{topic_id}/vote",
            axum::routing::delete(topics::clear_topic_vote),
        )
        .add(
            "/topics/{topic_id}/subscription",
            axum::routing::post(topics::subscribe_topic).delete(topics::unsubscribe_topic),
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
        .add(
            "/replies/{reply_id}/vote/{value}",
            axum::routing::post(replies::set_reply_vote),
        )
        .add(
            "/replies/{reply_id}/vote",
            axum::routing::delete(replies::clear_reply_vote),
        )
        .add("/widgets/catalog", get(widgets::get_widget_catalog))
        .add(
            "/widgets/validate",
            axum::routing::post(widgets::validate_widget_props),
        )
        .add("/users/{user_id}/stats", get(users::get_user_stats))
}

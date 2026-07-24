use anyhow::Context;
use axum::{Router, http::StatusCode};
use axum::routing::get;
use rustok_api::HostRuntimeContext;
use rustok_outbox::TransactionalEventBus;
use rustok_web::HttpError;
use sea_orm::DatabaseConnection;

pub mod categories;
pub mod category_commands;
pub mod category_lifecycle;
pub mod category_policy;
pub mod category_tree;
pub mod content_commands;
pub mod quote_commands;
pub mod read_state;
pub mod replies;
pub mod subscriptions;
pub mod topics;
pub mod users;
pub mod widgets;

#[derive(Clone)]
pub struct ForumHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }
}

impl ForumHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("forum HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })
    }
}

/// Map forum domain failures to stable HTTP semantics without exposing storage,
/// connector or internal implementation details.
pub(crate) fn map_forum_error(error: crate::ForumError) -> HttpError {
    use crate::ForumError;

    let code = error.stable_code();
    match error {
        ForumError::CategoryNotFound(_)
        | ForumError::TopicNotFound(_)
        | ForumError::ReplyNotFound(_)
        | ForumError::SolutionNotFound(_) => HttpError::not_found(
            code,
            "The requested forum resource was not found",
        ),
        ForumError::Forbidden(_) => HttpError::forbidden(code, "Permission denied"),
        ForumError::RelationRevisionConflict => HttpError::new(
            StatusCode::CONFLICT,
            code,
            "Forum relation revision changed concurrently",
        ),
        ForumError::TopicClosed
        | ForumError::TopicArchived
        | ForumError::TopicLocked
        | ForumError::TopicDeleted
        | ForumError::ReplyDeleted => HttpError::new(
            StatusCode::CONFLICT,
            code,
            "The forum resource state does not allow this operation",
        ),
        ForumError::CapabilityUnavailable { .. } | ForumError::CapabilityFailure { .. } => {
            HttpError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                code,
                "A required forum capability is temporarily unavailable",
            )
        }
        ForumError::Database(_) | ForumError::Content(_) | ForumError::Internal(_) => {
            HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                code,
                "The forum operation could not be completed",
            )
        }
        error => HttpError::bad_request(code, error.to_string()),
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    let state = ForumHttpRuntime::from_host(runtime)?;
    Ok(Router::new()
        .route(
            "/api/forum/categories",
            get(categories::list_categories).post(categories::create_category),
        )
        .route(
            "/api/forum/categories/tree",
            get(category_tree::get_category_tree),
        )
        .route(
            "/api/forum/categories/reorder",
            axum::routing::put(category_commands::reorder_category_siblings),
        )
        .route(
            "/api/forum/categories/{id}/move",
            axum::routing::put(category_commands::move_category),
        )
        .route(
            "/api/forum/categories/{id}/archive-subtree",
            axum::routing::post(category_lifecycle::archive_category_subtree),
        )
        .route(
            "/api/forum/categories/{id}/restore-subtree",
            axum::routing::post(category_lifecycle::restore_category_subtree),
        )
        .route(
            "/api/forum/categories/{id}/mark-read",
            axum::routing::post(read_state::mark_category_read),
        )
        .route(
            "/api/forum/categories/{id}/topic-policy",
            get(category_policy::get_category_topic_policy)
                .put(category_policy::update_category_topic_policy),
        )
        .route(
            "/api/forum/categories/{id}",
            get(categories::get_category)
                .put(categories::update_category)
                .delete(categories::delete_category),
        )
        .route(
            "/api/forum/categories/{id}/subscription",
            get(subscriptions::get_category_subscription_settings)
                .put(subscriptions::update_category_subscription_settings)
                .post(categories::subscribe_category)
                .delete(categories::unsubscribe_category),
        )
        .route(
            "/api/forum/topics",
            get(topics::list_topics).post(content_commands::create_topic),
        )
        .route(
            "/api/forum/topics/unread",
            get(read_state::list_unread_topics),
        )
        .route(
            "/api/forum/topics/mark-all-read",
            axum::routing::post(read_state::mark_all_topics_read),
        )
        .route(
            "/api/forum/topics/{id}/read-state",
            get(read_state::get_topic_read_state).put(read_state::mark_topic_read),
        )
        .route(
            "/api/forum/topics/{id}",
            get(topics::get_topic)
                .put(content_commands::update_topic)
                .delete(topics::delete_topic),
        )
        .route(
            "/api/forum/topics/{id}/quotes",
            axum::routing::put(quote_commands::set_topic_quotes),
        )
        .route(
            "/api/forum/topics/{topic_id}/solution/{reply_id}",
            axum::routing::post(topics::mark_topic_solution),
        )
        .route(
            "/api/forum/topics/{topic_id}/solution",
            axum::routing::delete(topics::clear_topic_solution),
        )
        .route(
            "/api/forum/topics/{topic_id}/vote/{value}",
            axum::routing::post(topics::set_topic_vote),
        )
        .route(
            "/api/forum/topics/{topic_id}/vote",
            axum::routing::delete(topics::clear_topic_vote),
        )
        .route(
            "/api/forum/topics/{topic_id}/subscription",
            get(subscriptions::get_topic_subscription_settings)
                .put(subscriptions::update_topic_subscription_settings)
                .post(topics::subscribe_topic)
                .delete(topics::unsubscribe_topic),
        )
        .route(
            "/api/forum/subscription-policy",
            get(subscriptions::get_subscription_policy)
                .put(subscriptions::update_subscription_policy),
        )
        .route(
            "/api/forum/topics/{id}/replies",
            get(replies::list_replies).post(content_commands::create_reply),
        )
        .route(
            "/api/forum/replies/{id}",
            get(replies::get_reply)
                .put(content_commands::update_reply)
                .delete(replies::delete_reply),
        )
        .route(
            "/api/forum/replies/{id}/quotes",
            axum::routing::put(quote_commands::set_reply_quotes),
        )
        .route(
            "/api/forum/replies/{reply_id}/vote/{value}",
            axum::routing::post(replies::set_reply_vote),
        )
        .route(
            "/api/forum/replies/{reply_id}/vote",
            axum::routing::delete(replies::clear_reply_vote),
        )
        .route(
            "/api/forum/widgets/catalog",
            get(widgets::get_widget_catalog),
        )
        .route(
            "/api/forum/widgets/validate",
            axum::routing::post(widgets::validate_widget_props),
        )
        .route(
            "/api/forum/users/{user_id}/stats",
            get(users::get_user_stats),
        )
        .with_state(state))
}

use anyhow::Context;
use axum::routing::get;
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
use rustok_api::{
    HostRuntimeContext, SharedStaticModuleSettingsReader,
    SharedStaticModuleSettingsTransactionReader,
};
use rustok_outbox::TransactionalEventBus;
use rustok_api::TenantContext;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;

use crate::{ForumSettingsProviders, SharedForumAudienceFactsPort};

pub mod categories;
pub mod category_commands;
pub mod category_lifecycle;
pub mod category_policy;
pub mod category_tree;
pub mod content_commands;
pub mod moderation;
pub mod quote_commands;
pub mod read_state;
pub mod replies;
pub mod subscriptions;
pub(crate) mod topic_redirect;
pub mod topics;
pub mod users;
pub mod widgets;

#[derive(Clone)]
pub struct ForumHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    audience_facts: Option<SharedForumAudienceFactsPort>,
    settings_providers: ForumSettingsProviders,
}

impl ForumHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn read_model_service(&self) -> crate::ForumReadModelService {
        crate::ForumReadModelService::new(self.db_clone())
            .with_settings_providers(self.settings_providers.clone())
    }

    fn topic_service(&self) -> crate::TopicService {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                crate::TopicService::with_audience_facts(self.db_clone(), self.event_bus(), facts)
            }
            None => crate::TopicService::new(self.db_clone(), self.event_bus()),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    fn reply_service(&self) -> crate::ReplyService {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                crate::ReplyService::with_audience_facts(self.db_clone(), self.event_bus(), facts)
            }
            None => crate::ReplyService::new(self.db_clone(), self.event_bus()),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    fn vote_service(&self) -> crate::VoteService {
        crate::VoteService::new(self.db_clone())
            .with_settings_providers(self.settings_providers.clone())
    }

    fn moderation_service(&self) -> crate::ModerationService {
        match self.audience_facts.clone() {
            Some(facts) => crate::ModerationService::with_audience_facts(
                self.db_clone(),
                self.event_bus(),
                facts,
            ),
            None => crate::ModerationService::new(self.db_clone(), self.event_bus()),
        }
    }
}

impl ForumHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("forum HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        let settings_providers = match (
            runtime.shared_get::<SharedStaticModuleSettingsReader>(),
            runtime.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
        ) {
            (Some(reader), Some(transactional_reader)) => {
                ForumSettingsProviders::default().with_static_readers(reader, transactional_reader)
            }
            _ => ForumSettingsProviders::default(),
        };

        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            audience_facts: runtime.shared_get::<SharedForumAudienceFactsPort>(),
            settings_providers,
        })
    }
}


async fn ensure_forum_module_enabled(
    runtime: &ForumHttpRuntime,
    tenant_id: uuid::Uuid,
) -> HttpResult<()> {
    match rustok_api::is_tenant_module_enabled(&runtime.db_clone(), tenant_id, "forum").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpError::new(
            StatusCode::FORBIDDEN,
            "MODULE_NOT_ENABLED",
            "Module 'forum' is not enabled for this tenant",
        )),
        Err(error) => {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "failed to verify Forum tenant-module lifecycle state"
            );
            Err(HttpError::internal("The Forum operation could not be completed"))
        }
    }
}

/// Enforces tenant-module lifecycle admission once at the owner HTTP entrypoint,
/// before any Forum handler performs authorization or domain work.
async fn enforce_forum_module_enabled(
    State(runtime): State<ForumHttpRuntime>,
    tenant: TenantContext,
    request: Request,
    next: Next,
) -> Response {
    match ensure_forum_module_enabled(&runtime, tenant.id).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
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
        | ForumError::SolutionNotFound(_)
        | ForumError::TopicRouteNotFound => {
            HttpError::not_found(code, "The requested forum resource was not found")
        }
        ForumError::Forbidden(_) => HttpError::forbidden(code, "Permission denied"),
        ForumError::RelationRevisionConflict
        | ForumError::AttachmentSourceRevisionConflict { .. } => HttpError::new(
            StatusCode::CONFLICT,
            code,
            "Forum attachment/content revision changed concurrently",
        ),
        ForumError::TopicClosed
        | ForumError::TopicArchived
        | ForumError::TopicLocked
        | ForumError::TopicDeleted
        | ForumError::TopicRestoreUnavailable(_)
        | ForumError::ReplyDeleted
        | ForumError::ReplyRestoreUnavailable(_)
        | ForumError::InternalVotingDisabled => HttpError::new(
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
        ForumError::Database(_)
        | ForumError::Content(_)
        | ForumError::Internal(_)
        | ForumError::TopicCanonicalResolutionConflict(_)
        | ForumError::TopicRouteResolutionConflict => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            code,
            "The forum operation could not be completed",
        ),
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
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    topic_redirect::redirect_merged_topic,
                ))
                .put(content_commands::update_topic)
                .delete(topics::delete_topic),
        )
        .route(
            "/api/forum/topics/{id}/restore",
            axum::routing::post(topics::restore_topic),
        )
        .route(
            "/api/forum/topics/{id}/quotes",
            axum::routing::put(quote_commands::set_topic_quotes),
        )
        .route(
            "/api/forum/topics/{topic_id}/solution/{reply_id}",
            axum::routing::post(moderation::mark_topic_solution),
        )
        .route(
            "/api/forum/topics/{topic_id}/solution",
            axum::routing::delete(moderation::clear_topic_solution),
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
            "/api/forum/replies/{id}/restore",
            axum::routing::post(replies::restore_reply),
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
            "/api/forum/widgets/preview",
            axum::routing::post(widgets::preview_widget),
        )
        .route(
            "/api/forum/users/{user_id}/stats",
            get(users::get_user_stats),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            enforce_forum_module_enabled,
        ))
        .with_state(state))
}


#[cfg(test)]
mod tests {
    use super::ensure_forum_module_enabled;
    use super::ForumHttpRuntime;
    use sea_orm::{ConnectionTrait, Database};
    use uuid::Uuid;

    async fn runtime_with_module_table() -> ForumHttpRuntime {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Forum HTTP lifecycle evidence DB should connect");
        db.execute_unprepared(
            "CREATE TABLE tenant_modules (             tenant_id TEXT NOT NULL,             module_slug TEXT NOT NULL,             enabled INTEGER NOT NULL,             settings TEXT NOT NULL DEFAULT '{}',             PRIMARY KEY (tenant_id, module_slug))",
        )
        .await
        .expect("tenant module evidence table should exist");

        let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));

        ForumHttpRuntime {
            db,
            event_bus,
            audience_facts: None,
            settings_providers: crate::ForumSettingsProviders::default(),
        }
    }

    #[tokio::test]
    async fn forum_rest_lifecycle_gate_rejects_disabled_tenant() {
        let runtime = runtime_with_module_table().await;
        let tenant_id = Uuid::new_v4();
        runtime
            .db_clone()
            .execute_unprepared(&format!(
                "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES ('{tenant_id}', 'forum', 0)"
            ))
            .await
            .expect("disabled Forum row should insert");

        let error = ensure_forum_module_enabled(&runtime, tenant_id)
            .await
            .expect_err("disabled Forum must reject REST admission");
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "MODULE_NOT_ENABLED");
    }

    #[tokio::test]
    async fn forum_rest_lifecycle_gate_rejects_missing_tenant_module() {
        let runtime = runtime_with_module_table().await;
        let error = ensure_forum_module_enabled(&runtime, Uuid::new_v4())
            .await
            .expect_err("missing Forum lifecycle state must fail closed");
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "MODULE_NOT_ENABLED");
    }

    #[tokio::test]
    async fn forum_rest_lifecycle_gate_accepts_enabled_tenant() {
        let runtime = runtime_with_module_table().await;
        let tenant_id = Uuid::new_v4();
        runtime
            .db_clone()
            .execute_unprepared(&format!(
                "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES ('{tenant_id}', 'forum', 1)"
            ))
            .await
            .expect("enabled Forum row should insert");

        ensure_forum_module_enabled(&runtime, tenant_id)
            .await
            .expect("enabled Forum should admit REST dispatch");
    }
}

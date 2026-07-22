#![allow(clippy::items_after_test_module)]

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::Error;
use rustok_core::events::{EventHandler, HandlerResult};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_telemetry::metrics;
use tracing::Instrument;

use crate::blog_projector::BlogSearchProjector;
use crate::projector::SearchProjector;

#[derive(Clone)]
pub struct SearchIngestionHandler {
    projector: SearchProjector,
    blog_projector: BlogSearchProjector,
}

impl SearchIngestionHandler {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            projector: SearchProjector::new(db.clone()),
            blog_projector: BlogSearchProjector::new(db),
        }
    }

    async fn rebuild_tenant(&self, tenant_id: Uuid) -> HandlerResult {
        self.projector.rebuild_tenant(tenant_id).await?;
        self.blog_projector.rebuild_tenant(tenant_id).await
    }

    async fn handle_reindex_request(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Option<Uuid>,
    ) -> HandlerResult {
        match (target_type, target_id) {
            ("search", _) => self.rebuild_tenant(tenant_id).await,
            ("content", Some(node_id)) => self.projector.upsert_node(tenant_id, node_id).await,
            ("content", None) => self.projector.rebuild_content_scope(tenant_id).await,
            ("product", Some(product_id)) => {
                self.projector.upsert_product(tenant_id, product_id).await
            }
            ("product", None) => self.projector.rebuild_product_scope(tenant_id).await,
            ("blog", Some(post_id)) => self.blog_projector.upsert_post(tenant_id, post_id).await,
            ("blog", None) => self.blog_projector.rebuild_tenant(tenant_id).await,
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl EventHandler for SearchIngestionHandler {
    fn name(&self) -> &'static str {
        "search_ingestion"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        match event {
            DomainEvent::NodeCreated { .. }
            | DomainEvent::NodeUpdated { .. }
            | DomainEvent::NodeTranslationUpdated { .. }
            | DomainEvent::NodePublished { .. }
            | DomainEvent::NodeUnpublished { .. }
            | DomainEvent::NodeDeleted { .. }
            | DomainEvent::BodyUpdated { .. }
            | DomainEvent::CategoryUpdated { .. }
            | DomainEvent::ProductCreated { .. }
            | DomainEvent::ProductUpdated { .. }
            | DomainEvent::ProductPublished { .. }
            | DomainEvent::ProductDeleted { .. }
            | DomainEvent::VariantCreated { .. }
            | DomainEvent::VariantUpdated { .. }
            | DomainEvent::VariantDeleted { .. }
            | DomainEvent::InventoryUpdated { .. }
            | DomainEvent::PriceUpdated { .. }
            | DomainEvent::BlogPostCreated { .. }
            | DomainEvent::BlogPostPublished { .. }
            | DomainEvent::BlogPostUnpublished { .. }
            | DomainEvent::BlogPostUpdated { .. }
            | DomainEvent::BlogPostArchived { .. }
            | DomainEvent::BlogPostDeleted { .. }
            | DomainEvent::LocaleEnabled { .. }
            | DomainEvent::LocaleDisabled { .. }
            | DomainEvent::TenantCreated { .. }
            | DomainEvent::TenantUpdated { .. } => true,
            DomainEvent::TagAttached { target_type, .. }
            | DomainEvent::TagDetached { target_type, .. } => target_type == "node",
            DomainEvent::ReindexRequested { target_type, .. } => {
                target_type == "search"
                    || target_type == "content"
                    || target_type == "product"
                    || target_type == "blog"
            }
            _ => false,
        }
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let operation = projector_operation_for_event(&envelope.event);
        let span = tracing::info_span!(
            "search.projector.dispatch",
            handler = self.name(),
            operation,
            event_id = %envelope.id,
            event_type = envelope.event.event_type(),
            tenant_id = %envelope.tenant_id,
            correlation_id = %envelope.correlation_id,
            causation_id = ?envelope.causation_id,
            trace_id = envelope.trace_id.as_deref().unwrap_or("")
        );

        async {
            match &envelope.event {
                DomainEvent::NodeCreated { node_id, .. }
                | DomainEvent::NodeUpdated { node_id, .. }
                | DomainEvent::NodePublished { node_id, .. }
                | DomainEvent::NodeUnpublished { node_id, .. } => {
                    self.projector
                        .upsert_node(envelope.tenant_id, *node_id)
                        .await
                }
                DomainEvent::NodeTranslationUpdated { node_id, locale }
                | DomainEvent::BodyUpdated { node_id, locale } => {
                    self.projector
                        .upsert_node_locale(envelope.tenant_id, *node_id, locale)
                        .await
                }
                DomainEvent::NodeDeleted { node_id, .. } => {
                    self.projector
                        .delete_node(envelope.tenant_id, *node_id)
                        .await
                }
                DomainEvent::TagAttached { target_id, .. }
                | DomainEvent::TagDetached { target_id, .. } => {
                    self.projector
                        .upsert_node(envelope.tenant_id, *target_id)
                        .await
                }
                DomainEvent::CategoryUpdated { category_id } => {
                    self.projector
                        .reindex_category(envelope.tenant_id, *category_id)
                        .await
                }
                DomainEvent::ProductCreated { product_id }
                | DomainEvent::ProductUpdated { product_id }
                | DomainEvent::ProductPublished { product_id } => {
                    self.projector
                        .upsert_product(envelope.tenant_id, *product_id)
                        .await
                }
                DomainEvent::ProductDeleted { product_id } => {
                    self.projector
                        .delete_product(envelope.tenant_id, *product_id)
                        .await
                }
                DomainEvent::VariantCreated { product_id, .. }
                | DomainEvent::VariantUpdated { product_id, .. }
                | DomainEvent::VariantDeleted { product_id, .. }
                | DomainEvent::InventoryUpdated { product_id, .. }
                | DomainEvent::PriceUpdated { product_id, .. } => {
                    self.projector
                        .upsert_product(envelope.tenant_id, *product_id)
                        .await
                }
                DomainEvent::BlogPostCreated { post_id, .. }
                | DomainEvent::BlogPostPublished { post_id, .. }
                | DomainEvent::BlogPostUnpublished { post_id }
                | DomainEvent::BlogPostUpdated { post_id, .. }
                | DomainEvent::BlogPostArchived { post_id, .. } => {
                    self.blog_projector
                        .upsert_post(envelope.tenant_id, *post_id)
                        .await
                }
                DomainEvent::BlogPostDeleted { post_id } => {
                    self.blog_projector
                        .delete_post(envelope.tenant_id, *post_id)
                        .await
                }
                DomainEvent::LocaleEnabled { .. }
                | DomainEvent::LocaleDisabled { .. }
                | DomainEvent::TenantCreated { .. }
                | DomainEvent::TenantUpdated { .. } => {
                    self.rebuild_tenant(envelope.tenant_id).await
                }
                DomainEvent::ReindexRequested {
                    target_type,
                    target_id,
                } => {
                    self.handle_reindex_request(
                        envelope.tenant_id,
                        target_type.as_str(),
                        *target_id,
                    )
                    .await
                }
                _ => Ok(()),
            }
        }
        .instrument(span)
        .await
    }

    async fn on_error(&self, envelope: &EventEnvelope, error: &Error) {
        let operation = match &envelope.event {
            DomainEvent::ReindexRequested { target_type, .. } => match target_type.as_str() {
                "content" => "rebuild_content_scope",
                "product" => "rebuild_product_scope",
                "blog" => "rebuild_blog_scope",
                _ => "rebuild_tenant",
            },
            DomainEvent::ProductCreated { .. }
            | DomainEvent::ProductUpdated { .. }
            | DomainEvent::ProductPublished { .. }
            | DomainEvent::ProductDeleted { .. }
            | DomainEvent::VariantCreated { .. }
            | DomainEvent::VariantUpdated { .. }
            | DomainEvent::VariantDeleted { .. }
            | DomainEvent::InventoryUpdated { .. }
            | DomainEvent::PriceUpdated { .. } => "upsert_product",
            DomainEvent::BlogPostDeleted { .. } => "delete_blog_post",
            DomainEvent::BlogPostCreated { .. }
            | DomainEvent::BlogPostPublished { .. }
            | DomainEvent::BlogPostUnpublished { .. }
            | DomainEvent::BlogPostUpdated { .. }
            | DomainEvent::BlogPostArchived { .. } => "upsert_blog_post",
            _ => "upsert_node",
        };

        metrics::record_search_indexing_operation(operation, "event_handler", "error", 0.0);
        metrics::record_module_error("search", classify_error(error), "error");
        tracing::error!(
            handler = self.name(),
            event_id = %envelope.id,
            event_type = envelope.event.event_type(),
            tenant_id = %envelope.tenant_id,
            correlation_id = %envelope.correlation_id,
            causation_id = ?envelope.causation_id,
            trace_id = envelope.trace_id.as_deref().unwrap_or(""),
            error = %error,
            "Search ingestion handler error"
        );
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::events::EventHandler;
    use sea_orm::Database;

    use super::*;

    #[tokio::test]
    async fn handler_matches_search_relevant_events() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let handler = SearchIngestionHandler::new(db);

        assert!(handler.handles(&DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "page".to_string(),
            author_id: None,
        }));
        assert!(handler.handles(&DomainEvent::ProductUpdated {
            product_id: Uuid::new_v4(),
        }));
        assert!(handler.handles(&DomainEvent::ReindexRequested {
            target_type: "search".to_string(),
            target_id: None,
        }));
    }

    #[tokio::test]
    async fn handler_ignores_non_search_events() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let handler = SearchIngestionHandler::new(db);

        assert!(!handler.handles(&DomainEvent::OrderPlaced {
            order_id: Uuid::new_v4(),
            customer_id: None,
            total: 1000,
            currency: "USD".to_string(),
        }));
    }
}

fn classify_error(error: &Error) -> &'static str {
    match error {
        Error::Database(_) => "database",
        Error::Validation(_) => "validation",
        Error::External(_) => "external",
        Error::NotFound(_) => "not_found",
        Error::Forbidden(_) => "forbidden",
        Error::Auth(_) => "auth",
        Error::Cache(_) => "cache",
        Error::Serialization(_) => "serialization",
        Error::Scripting(_) => "scripting",
        Error::InvalidIdFormat(_) => "invalid_id",
    }
}

fn projector_operation_for_event(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::ReindexRequested { target_type, .. } => match target_type.as_str() {
            "content" => "rebuild_content_scope",
            "product" => "rebuild_product_scope",
            "blog" => "rebuild_blog_scope",
            _ => "rebuild_tenant",
        },
        DomainEvent::NodeTranslationUpdated { .. } | DomainEvent::BodyUpdated { .. } => {
            "upsert_node_locale"
        }
        DomainEvent::NodeDeleted { .. } => "delete_node",
        DomainEvent::TagAttached { .. }
        | DomainEvent::TagDetached { .. }
        | DomainEvent::NodeCreated { .. }
        | DomainEvent::NodeUpdated { .. }
        | DomainEvent::NodePublished { .. }
        | DomainEvent::NodeUnpublished { .. } => "upsert_node",
        DomainEvent::CategoryUpdated { .. } => "reindex_category",
        DomainEvent::ProductDeleted { .. } => "delete_product",
        DomainEvent::ProductCreated { .. }
        | DomainEvent::ProductUpdated { .. }
        | DomainEvent::ProductPublished { .. }
        | DomainEvent::VariantCreated { .. }
        | DomainEvent::VariantUpdated { .. }
        | DomainEvent::VariantDeleted { .. }
        | DomainEvent::InventoryUpdated { .. }
        | DomainEvent::PriceUpdated { .. } => "upsert_product",
        DomainEvent::BlogPostDeleted { .. } => "delete_blog_post",
        DomainEvent::BlogPostCreated { .. }
        | DomainEvent::BlogPostPublished { .. }
        | DomainEvent::BlogPostUnpublished { .. }
        | DomainEvent::BlogPostUpdated { .. }
        | DomainEvent::BlogPostArchived { .. } => "upsert_blog_post",
        DomainEvent::LocaleEnabled { .. }
        | DomainEvent::LocaleDisabled { .. }
        | DomainEvent::TenantCreated { .. }
        | DomainEvent::TenantUpdated { .. } => "rebuild_tenant",
        _ => "noop",
    }
}

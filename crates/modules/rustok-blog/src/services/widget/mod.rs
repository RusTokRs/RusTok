mod contract;
mod queries;

#[cfg(test)]
mod tests;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{BlogWidgetCatalog, BlogWidgetInput, BlogWidgetPreview, BlogWidgetValidation};
use crate::error::BlogResult;

/// Owner of widget configuration and read-only preview. Fly owns the draft document.
pub struct BlogWidgetService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl BlogWidgetService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub fn catalog() -> BlogWidgetCatalog {
        contract::catalog()
    }

    pub fn validate(input: BlogWidgetInput) -> BlogWidgetValidation {
        contract::validate(input)
    }

    /// Preview never authorizes from props or copies records into the Fly document.
    /// Each read retains the owner service's resource scope, locale and channel policy.
    pub async fn preview(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        input: BlogWidgetInput,
    ) -> BlogResult<BlogWidgetPreview> {
        self.preview_owner(
            tenant_id,
            security,
            locale,
            fallback_locale,
            channel_slug,
            input,
        )
        .await
    }
}

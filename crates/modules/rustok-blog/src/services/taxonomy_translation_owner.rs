use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyModuleTermTranslationOwner, TaxonomyTermKind};

pub struct BlogTaxonomyTranslationOwner;

#[async_trait]
impl TaxonomyModuleTermTranslationOwner for BlogTaxonomyTranslationOwner {
    fn module_slug(&self) -> &str {
        "blog"
    }

    fn authorize(
        &self,
        context: &PortContext,
        _tenant_id: Uuid,
        kind: TaxonomyTermKind,
        _term_id: Uuid,
        action: Action,
    ) -> Result<(), PortError> {
        let security = SecurityContext::try_from_port_context(context)?;
        let resource = match kind {
            TaxonomyTermKind::Tag => Resource::Tags,
            TaxonomyTermKind::Category => Resource::BlogCategories,
        };
        if security.get_scope(resource, action) == PermissionScope::None {
            return Err(PortError::forbidden(
                "blog.taxonomy_translation_permission_denied",
                format!("blog owner permission is required for {resource}:{action}"),
            ));
        }
        Ok(())
    }

    async fn on_translation_applied_in_tx(
        &self,
        transaction: &DatabaseTransaction,
        context: &PortContext,
        tenant_id: Uuid,
        _kind: TaxonomyTermKind,
        _term_id: Uuid,
    ) -> Result<(), PortError> {
        let security = SecurityContext::try_from_port_context(context)?;
        TransactionalEventBus::publish_root_in_tx(
            transaction,
            tenant_id,
            security.user_id,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )
        .await
        .map_err(|_| {
            PortError::unavailable(
                "blog.taxonomy_translation_reindex_unavailable",
                "Blog reindex side effect could not be queued",
            )
        })?;
        Ok(())
    }
}

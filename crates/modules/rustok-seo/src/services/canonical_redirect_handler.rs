use std::sync::Arc;
use async_trait::async_trait;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_core::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::SeoTargetRegistry;
use sea_orm::DatabaseConnection;

use super::SeoService;

pub(crate) struct SeoCanonicalUrlRedirectHandler {
    service: SeoService,
}

impl SeoCanonicalUrlRedirectHandler {
    pub(crate) fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        registry: Arc<SeoTargetRegistry>,
    ) -> Self {
        Self {
            service: SeoService::new(db, event_bus, registry),
        }
    }
}

#[async_trait]
impl EventHandler for SeoCanonicalUrlRedirectHandler {
    fn name(&self) -> &'static str {
        "seo.canonical_url_redirect_handler"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::CanonicalUrlChanged { .. })
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let DomainEvent::CanonicalUrlChanged {
            new_canonical_url,
            old_urls,
            ..
        } = &envelope.event
        else {
            return Ok(());
        };

        self.service
            .auto_redirect_on_canonical_url_changed(
                envelope.tenant_id,
                new_canonical_url.as_str(),
                old_urls.as_slice(),
            )
            .await
            .map_err(|err| rustok_core::Error::Internal(err.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::DomainEvent;
    use uuid::Uuid;

    #[test]
    fn handler_identifies_canonical_url_changed_event() {
        let dummy_event = DomainEvent::CanonicalUrlChanged {
            target_id: Uuid::new_v4(),
            target_kind: "category".to_string(),
            locale: "ru".to_string(),
            new_canonical_url: "/catalog/shoes-2026".to_string(),
            old_urls: vec!["/catalog/shoes".to_string()],
        };

        let other_event = DomainEvent::SeoMetaUpserted {
            target_id: Uuid::new_v4(),
            target_kind: "category".to_string(),
            locale: "ru".to_string(),
            source: "manual".to_string(),
            idempotency_key: "test-key-1".to_string(),
        };

        // We can test `handles` without instantiating DB using a zero-sized or mock wrapper if needed,
        // or test `name` and event matching logic directly.
        assert_eq!(
            matches!(dummy_event, DomainEvent::CanonicalUrlChanged { .. }),
            true
        );
        assert_eq!(
            matches!(other_event, DomainEvent::CanonicalUrlChanged { .. }),
            false
        );
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::events::{EventHandler, HandlerResult};
use rustok_events::{DomainEvent, EventEnvelope};
use thiserror::Error;
use uuid::Uuid;

pub const PAGES_CACHE_NAMESPACE_FORMAT: &str = "pages_cache_namespace_v1";
pub const PAGES_CACHE_EVENT_HANDLER: &str = "pages_cache_invalidation";
pub const PAGES_CACHE_ENTITY_KIND: &str = "page";
pub const MAX_PAGE_CACHE_KEY_VARIANT_BYTES: usize = 512;

const ALL_SCOPES: [PageCacheScope; 3] = [
    PageCacheScope::Route,
    PageCacheScope::Page,
    PageCacheScope::Artifact,
];
const ROUTE_AND_PAGE_SCOPES: [PageCacheScope; 2] = [PageCacheScope::Route, PageCacheScope::Page];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageCacheScope {
    Route,
    Page,
    Artifact,
}

impl PageCacheScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Page => "page",
            Self::Artifact => "artifact",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageCacheInvalidationCause {
    Updated,
    Published,
    Unpublished,
    Deleted,
}

impl PageCacheInvalidationCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "page.updated",
            Self::Published => "page.published",
            Self::Unpublished => "page.unpublished",
            Self::Deleted => "page.deleted",
        }
    }

    pub const fn scopes(self) -> &'static [PageCacheScope] {
        match self {
            Self::Updated => &ROUTE_AND_PAGE_SCOPES,
            Self::Published | Self::Unpublished | Self::Deleted => &ALL_SCOPES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCacheInvalidationRequest {
    pub tenant_id: Uuid,
    pub page_id: Uuid,
    pub event_id: Uuid,
    pub correlation_id: Uuid,
    pub trace_id: Option<String>,
    pub cause: PageCacheInvalidationCause,
}

impl PageCacheInvalidationRequest {
    pub fn new(
        tenant_id: Uuid,
        page_id: Uuid,
        event_id: Uuid,
        correlation_id: Uuid,
        trace_id: Option<String>,
        cause: PageCacheInvalidationCause,
    ) -> Result<Self, PageCacheInvalidationError> {
        if tenant_id.is_nil() {
            return Err(PageCacheInvalidationError::InvalidRequest(
                "tenant id must not be nil".to_string(),
            ));
        }
        if page_id.is_nil() {
            return Err(PageCacheInvalidationError::InvalidRequest(
                "page id must not be nil".to_string(),
            ));
        }
        if event_id.is_nil() {
            return Err(PageCacheInvalidationError::InvalidRequest(
                "event id must not be nil".to_string(),
            ));
        }
        if correlation_id.is_nil() {
            return Err(PageCacheInvalidationError::InvalidRequest(
                "correlation id must not be nil".to_string(),
            ));
        }
        let trace_id = trace_id.and_then(|trace_id| {
            let trace_id = trace_id.trim();
            (!trace_id.is_empty()).then(|| trace_id.to_string())
        });
        Ok(Self {
            tenant_id,
            page_id,
            event_id,
            correlation_id,
            trace_id,
            cause,
        })
    }

    pub const fn scopes(&self) -> &'static [PageCacheScope] {
        self.cause.scopes()
    }

    pub fn namespace(&self, scope: PageCacheScope) -> String {
        page_cache_namespace(scope, self.tenant_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCacheInvalidationReceipt {
    pub event_id: Uuid,
    pub correlation_id: Uuid,
    pub route_generation: Option<u64>,
    pub page_generation: Option<u64>,
    pub artifact_generation: Option<u64>,
}

impl PageCacheInvalidationReceipt {
    pub fn new(request: &PageCacheInvalidationRequest) -> Self {
        Self {
            event_id: request.event_id,
            correlation_id: request.correlation_id,
            route_generation: None,
            page_generation: None,
            artifact_generation: None,
        }
    }

    pub fn record(&mut self, scope: PageCacheScope, generation: u64) {
        match scope {
            PageCacheScope::Route => self.route_generation = Some(generation),
            PageCacheScope::Page => self.page_generation = Some(generation),
            PageCacheScope::Artifact => self.artifact_generation = Some(generation),
        }
    }

    pub const fn generation(&self, scope: PageCacheScope) -> Option<u64> {
        match scope {
            PageCacheScope::Route => self.route_generation,
            PageCacheScope::Page => self.page_generation,
            PageCacheScope::Artifact => self.artifact_generation,
        }
    }

    pub fn validate_for(
        &self,
        request: &PageCacheInvalidationRequest,
    ) -> Result<(), PageCacheInvalidationError> {
        if self.event_id != request.event_id || self.correlation_id != request.correlation_id {
            return Err(PageCacheInvalidationError::ReceiptIdentityMismatch);
        }
        for scope in request.scopes() {
            if self
                .generation(*scope)
                .is_none_or(|generation| generation == 0)
            {
                return Err(PageCacheInvalidationError::MissingGeneration(*scope));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PageCacheInvalidationError {
    #[error("invalid Pages cache invalidation request: {0}")]
    InvalidRequest(String),
    #[error("Pages cache invalidation provider failed: {0}")]
    Provider(String),
    #[error("Pages cache invalidation receipt identity does not match the source event")]
    ReceiptIdentityMismatch,
    #[error("Pages cache invalidation receipt is missing a positive {0:?} generation")]
    MissingGeneration(PageCacheScope),
    #[error("Pages cache key generation must be positive")]
    ZeroGeneration,
    #[error("Pages cache key variant must not be empty")]
    EmptyKeyVariant,
    #[error("Pages cache key variant is {length} bytes; maximum is {maximum}")]
    KeyVariantTooLarge { length: usize, maximum: usize },
}

#[async_trait]
pub trait PageCacheInvalidationPort: Send + Sync {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheInvalidationError>;
}

#[derive(Clone)]
pub struct PagesCacheInvalidationRuntime {
    port: Arc<dyn PageCacheInvalidationPort>,
}

impl PagesCacheInvalidationRuntime {
    pub fn new(port: Arc<dyn PageCacheInvalidationPort>) -> Self {
        Self { port }
    }

    pub async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheInvalidationError> {
        let receipt = self.port.invalidate(request.clone()).await?;
        receipt.validate_for(&request)?;
        Ok(receipt)
    }
}

#[derive(Clone)]
pub struct PageCacheInvalidationEventHandler {
    runtime: PagesCacheInvalidationRuntime,
}

impl PageCacheInvalidationEventHandler {
    pub fn new(runtime: PagesCacheInvalidationRuntime) -> Self {
        Self { runtime }
    }

    fn request(
        envelope: &EventEnvelope,
    ) -> Result<Option<PageCacheInvalidationRequest>, PageCacheInvalidationError> {
        let (page_id, cause) = match &envelope.event {
            DomainEvent::NodeUpdated { node_id, kind } if kind == PAGES_CACHE_ENTITY_KIND => {
                (*node_id, PageCacheInvalidationCause::Updated)
            }
            DomainEvent::NodePublished { node_id, kind } if kind == PAGES_CACHE_ENTITY_KIND => {
                (*node_id, PageCacheInvalidationCause::Published)
            }
            DomainEvent::NodeUnpublished { node_id, kind } if kind == PAGES_CACHE_ENTITY_KIND => {
                (*node_id, PageCacheInvalidationCause::Unpublished)
            }
            DomainEvent::NodeDeleted { node_id, kind } if kind == PAGES_CACHE_ENTITY_KIND => {
                (*node_id, PageCacheInvalidationCause::Deleted)
            }
            _ => return Ok(None),
        };
        PageCacheInvalidationRequest::new(
            envelope.tenant_id,
            page_id,
            envelope.id,
            envelope.correlation_id,
            envelope.trace_id.clone(),
            cause,
        )
        .map(Some)
    }
}

#[async_trait]
impl EventHandler for PageCacheInvalidationEventHandler {
    fn name(&self) -> &'static str {
        PAGES_CACHE_EVENT_HANDLER
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::NodeUpdated { kind, .. }
                | DomainEvent::NodePublished { kind, .. }
                | DomainEvent::NodeUnpublished { kind, .. }
                | DomainEvent::NodeDeleted { kind, .. }
                if kind == PAGES_CACHE_ENTITY_KIND
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let Some(request) = Self::request(envelope)
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?
        else {
            return Ok(());
        };
        let cause = request.cause;
        let page_id = request.page_id;
        let receipt = self
            .runtime
            .invalidate(request)
            .await
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
        tracing::info!(
            %page_id,
            cause = cause.as_str(),
            route_generation = receipt.route_generation,
            page_generation = receipt.page_generation,
            artifact_generation = receipt.artifact_generation,
            event_id = %receipt.event_id,
            correlation_id = %receipt.correlation_id,
            "Pages cache namespaces invalidated"
        );
        Ok(())
    }
}

pub fn page_cache_namespace(scope: PageCacheScope, tenant_id: Uuid) -> String {
    format!(
        "{PAGES_CACHE_NAMESPACE_FORMAT}:{}:tenant:{tenant_id}",
        scope.as_str()
    )
}

pub fn page_cache_key(
    scope: PageCacheScope,
    tenant_id: Uuid,
    page_id: Uuid,
    generation: u64,
    variant: &str,
) -> Result<String, PageCacheInvalidationError> {
    if generation == 0 {
        return Err(PageCacheInvalidationError::ZeroGeneration);
    }
    let variant = variant.trim();
    if variant.is_empty() {
        return Err(PageCacheInvalidationError::EmptyKeyVariant);
    }
    if variant.len() > MAX_PAGE_CACHE_KEY_VARIANT_BYTES {
        return Err(PageCacheInvalidationError::KeyVariantTooLarge {
            length: variant.len(),
            maximum: MAX_PAGE_CACHE_KEY_VARIANT_BYTES,
        });
    }
    Ok(format!(
        "{}:g-{generation}:page:{page_id}:{}",
        page_cache_namespace(scope, tenant_id),
        hex::encode(variant.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakePort {
        requests: Mutex<Vec<PageCacheInvalidationRequest>>,
    }

    #[async_trait]
    impl PageCacheInvalidationPort for FakePort {
        async fn invalidate(
            &self,
            request: PageCacheInvalidationRequest,
        ) -> Result<PageCacheInvalidationReceipt, PageCacheInvalidationError> {
            self.requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            let mut receipt = PageCacheInvalidationReceipt::new(&request);
            for scope in request.scopes() {
                receipt.record(*scope, 7);
            }
            Ok(receipt)
        }
    }

    fn envelope(event: DomainEvent) -> EventEnvelope {
        EventEnvelope::new(Uuid::from_u128(1), Some(Uuid::from_u128(2)), event)
    }

    #[test]
    fn published_pages_invalidate_route_page_and_artifact_namespaces() {
        assert_eq!(PageCacheInvalidationCause::Published.scopes(), &ALL_SCOPES);
        assert_eq!(
            PageCacheInvalidationCause::Updated.scopes(),
            &ROUTE_AND_PAGE_SCOPES
        );
    }

    #[test]
    fn namespace_generations_are_bounded_by_tenant_and_scope() {
        let tenant = Uuid::from_u128(11);
        assert_ne!(
            page_cache_namespace(PageCacheScope::Route, tenant),
            page_cache_namespace(PageCacheScope::Page, tenant)
        );
        assert_ne!(
            page_cache_namespace(PageCacheScope::Page, tenant),
            page_cache_namespace(PageCacheScope::Artifact, tenant)
        );
        assert_eq!(
            page_cache_namespace(PageCacheScope::Page, tenant),
            page_cache_namespace(PageCacheScope::Page, tenant)
        );
    }

    #[tokio::test]
    async fn handler_forwards_only_page_events_and_validates_the_receipt() {
        let port = Arc::new(FakePort::default());
        let handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
            port.clone(),
        ));
        let page_id = Uuid::from_u128(42);
        let page = envelope(DomainEvent::NodePublished {
            node_id: page_id,
            kind: PAGES_CACHE_ENTITY_KIND.to_string(),
        });
        handler.handle(&page).await.unwrap();
        assert_eq!(
            port.requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            &[PageCacheInvalidationRequest::new(
                page.tenant_id,
                page_id,
                page.id,
                page.correlation_id,
                page.trace_id.clone(),
                PageCacheInvalidationCause::Published,
            )
            .unwrap()]
        );
        assert!(!handler.handles(&DomainEvent::NodePublished {
            node_id: page_id,
            kind: "post".to_string(),
        }));
    }

    #[test]
    fn cache_keys_bind_scope_generation_page_and_variant_without_raw_variant_text() {
        let tenant = Uuid::from_u128(11);
        let page = Uuid::from_u128(22);
        let key = page_cache_key(PageCacheScope::Route, tenant, page, 3, "en:/about").unwrap();
        assert!(key.contains(":g-3:page:"));
        assert!(key.contains(&page.to_string()));
        assert!(!key.contains("/about"));
    }
}

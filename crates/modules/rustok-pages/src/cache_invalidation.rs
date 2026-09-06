use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::events::{EventHandler, HandlerResult};
use rustok_events::{DomainEvent, EventEnvelope};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const PAGES_CACHE_NAMESPACE_FORMAT: &str = "pages_cache_namespace_v1";
pub const PAGES_CACHE_EVENT_HANDLER: &str = "pages_cache_invalidation";
pub const PAGES_CACHE_ENTITY_KIND: &str = "page";
pub const PAGES_STOREFRONT_CACHE_TTL_SECS: u64 = 60;
pub const PAGES_STOREFRONT_CACHE_MAX_CAPACITY: u64 = 10_000;
pub const MAX_PAGE_CACHE_KEY_VARIANT_BYTES: usize = 512;
pub const MAX_PAGE_CACHE_VALUE_BYTES: usize = 2 * 1024 * 1024;

pub const PAGE_CACHE_SCOPES: [PageCacheScope; 3] = [
    PageCacheScope::Route,
    PageCacheScope::Page,
    PageCacheScope::Artifact,
];
pub const PAGE_CACHE_MUTABLE_SCOPES: [PageCacheScope; 2] =
    [PageCacheScope::Route, PageCacheScope::Page];

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
            Self::Updated => &PAGE_CACHE_MUTABLE_SCOPES,
            Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PageCacheGenerationSnapshot {
    pub route: u64,
    pub page: u64,
    pub artifact: u64,
}

impl PageCacheGenerationSnapshot {
    pub const fn new(route: u64, page: u64, artifact: u64) -> Self {
        Self {
            route,
            page,
            artifact,
        }
    }

    pub fn record(&mut self, scope: PageCacheScope, generation: u64) {
        match scope {
            PageCacheScope::Route => self.route = generation,
            PageCacheScope::Page => self.page = generation,
            PageCacheScope::Artifact => self.artifact = generation,
        }
    }

    pub const fn generation(self, scope: PageCacheScope) -> u64 {
        match scope {
            PageCacheScope::Route => self.route,
            PageCacheScope::Page => self.page,
            PageCacheScope::Artifact => self.artifact,
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
    ) -> Result<Self, PageCacheError> {
        if tenant_id.is_nil() {
            return Err(PageCacheError::InvalidRequest(
                "tenant id must not be nil".to_string(),
            ));
        }
        if page_id.is_nil() {
            return Err(PageCacheError::InvalidRequest(
                "page id must not be nil".to_string(),
            ));
        }
        if event_id.is_nil() {
            return Err(PageCacheError::InvalidRequest(
                "event id must not be nil".to_string(),
            ));
        }
        if correlation_id.is_nil() {
            return Err(PageCacheError::InvalidRequest(
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
    ) -> Result<(), PageCacheError> {
        if self.event_id != request.event_id || self.correlation_id != request.correlation_id {
            return Err(PageCacheError::ReceiptIdentityMismatch);
        }
        for scope in request.scopes() {
            if self
                .generation(*scope)
                .is_none_or(|generation| generation == 0)
            {
                return Err(PageCacheError::MissingGeneration(*scope));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PageCacheError {
    #[error("invalid Pages cache request: {0}")]
    InvalidRequest(String),
    #[error("Pages cache provider failed: {0}")]
    Provider(String),
    #[error("Pages cache invalidation receipt identity does not match the source event")]
    ReceiptIdentityMismatch,
    #[error("Pages cache invalidation receipt is missing a positive {0:?} generation")]
    MissingGeneration(PageCacheScope),
    #[error("Pages cache key variant must not be empty")]
    EmptyKeyVariant,
    #[error("Pages cache key variant is {length} bytes; maximum is {maximum}")]
    KeyVariantTooLarge { length: usize, maximum: usize },
    #[error("Pages cache value is {length} bytes; maximum is {maximum}")]
    ValueTooLarge { length: usize, maximum: usize },
    #[error("Pages cache serialization failed: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait PageCacheInvalidationPort: Send + Sync {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError>;
}

#[async_trait]
pub trait PagesCacheReadPort: Send + Sync {
    async fn generation_snapshot(
        &self,
        tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError>;

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError>;

    async fn put(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<(), PageCacheError>;
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
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let receipt = self.port.invalidate(request.clone()).await?;
        receipt.validate_for(&request)?;
        Ok(receipt)
    }
}

#[derive(Clone)]
pub struct PagesCacheReadRuntime {
    port: Arc<dyn PagesCacheReadPort>,
}

impl PagesCacheReadRuntime {
    pub fn new(port: Arc<dyn PagesCacheReadPort>) -> Self {
        Self { port }
    }

    pub async fn generation_snapshot(
        &self,
        tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        if tenant_id.is_nil() {
            return Err(PageCacheError::InvalidRequest(
                "tenant id must not be nil".to_string(),
            ));
        }
        self.port.generation_snapshot(tenant_id).await
    }

    pub async fn get_json<T>(&self, key: &str) -> Result<Option<T>, PageCacheError>
    where
        T: DeserializeOwned,
    {
        let Some(bytes) = self.port.get(key).await? else {
            return Ok(None);
        };
        validate_cache_value_size(bytes.len())?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| PageCacheError::Serialization(error.to_string()))
    }

    pub async fn put_json<T>(&self, key: String, value: &T) -> Result<(), PageCacheError>
    where
        T: Serialize + Sync,
    {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| PageCacheError::Serialization(error.to_string()))?;
        validate_cache_value_size(bytes.len())?;
        self.port
            .put(
                key,
                bytes,
                Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS),
            )
            .await
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
    ) -> Result<Option<PageCacheInvalidationRequest>, PageCacheError> {
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
) -> Result<String, PageCacheError> {
    let variant_hash = cache_variant_hash(variant)?;
    Ok(format!(
        "{}:g-{generation}:page:{page_id}:{variant_hash}",
        page_cache_namespace(scope, tenant_id),
    ))
}

pub fn storefront_pages_cache_key(
    tenant_id: Uuid,
    generations: PageCacheGenerationSnapshot,
    variant: &str,
) -> Result<String, PageCacheError> {
    let variant_hash = cache_variant_hash(variant)?;
    Ok(format!(
        "{PAGES_CACHE_NAMESPACE_FORMAT}:storefront:tenant:{tenant_id}:rg-{}:pg-{}:ag-{}:{variant_hash}",
        generations.route, generations.page, generations.artifact,
    ))
}

fn cache_variant_hash(variant: &str) -> Result<String, PageCacheError> {
    let variant = variant.trim();
    if variant.is_empty() {
        return Err(PageCacheError::EmptyKeyVariant);
    }
    if variant.len() > MAX_PAGE_CACHE_KEY_VARIANT_BYTES {
        return Err(PageCacheError::KeyVariantTooLarge {
            length: variant.len(),
            maximum: MAX_PAGE_CACHE_KEY_VARIANT_BYTES,
        });
    }
    Ok(Sha256::digest(variant.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn validate_cache_value_size(length: usize) -> Result<(), PageCacheError> {
    if length > MAX_PAGE_CACHE_VALUE_BYTES {
        return Err(PageCacheError::ValueTooLarge {
            length,
            maximum: MAX_PAGE_CACHE_VALUE_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use serde_json::{Value, json};

    use super::*;

    #[derive(Default)]
    struct FakeInvalidationPort {
        requests: Mutex<Vec<PageCacheInvalidationRequest>>,
    }

    #[async_trait]
    impl PageCacheInvalidationPort for FakeInvalidationPort {
        async fn invalidate(
            &self,
            request: PageCacheInvalidationRequest,
        ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
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

    struct FakeReadPort {
        generations: PageCacheGenerationSnapshot,
        values: Mutex<HashMap<String, Vec<u8>>>,
    }

    #[async_trait]
    impl PagesCacheReadPort for FakeReadPort {
        async fn generation_snapshot(
            &self,
            _tenant_id: Uuid,
        ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
            Ok(self.generations)
        }

        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
            Ok(self
                .values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(key)
                .cloned())
        }

        async fn put(
            &self,
            key: String,
            value: Vec<u8>,
            _ttl: Duration,
        ) -> Result<(), PageCacheError> {
            self.values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key, value);
            Ok(())
        }
    }

    fn envelope(event: DomainEvent) -> EventEnvelope {
        EventEnvelope::new(Uuid::from_u128(1), Some(Uuid::from_u128(2)), event)
    }

    #[test]
    fn published_pages_invalidate_route_page_and_artifact_namespaces() {
        assert_eq!(
            PageCacheInvalidationCause::Published.scopes(),
            &PAGE_CACHE_SCOPES
        );
        assert_eq!(
            PageCacheInvalidationCause::Updated.scopes(),
            &PAGE_CACHE_MUTABLE_SCOPES
        );
    }

    #[test]
    fn namespace_generations_are_bounded_by_tenant_and_scope() {
        let tenant = Uuid::from_u128(11);
        assert_eq!(
            page_cache_namespace(PageCacheScope::Route, tenant),
            format!("{PAGES_CACHE_NAMESPACE_FORMAT}:route:tenant:{tenant}")
        );
        assert_ne!(
            page_cache_namespace(PageCacheScope::Page, tenant),
            page_cache_namespace(PageCacheScope::Artifact, tenant)
        );
    }

    #[tokio::test]
    async fn handler_forwards_only_page_events_and_validates_the_receipt() {
        let port = Arc::new(FakeInvalidationPort::default());
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
    fn cache_keys_allow_initial_generation_and_hash_raw_variants() {
        let tenant = Uuid::from_u128(11);
        let page = Uuid::from_u128(22);
        let key = page_cache_key(PageCacheScope::Route, tenant, page, 0, "en:/about").unwrap();
        assert!(key.contains(":g-0:page:"));
        assert!(key.contains(&page.to_string()));
        assert!(!key.contains("/about"));
    }

    #[test]
    fn storefront_key_changes_when_any_dependency_generation_changes() {
        let tenant = Uuid::from_u128(11);
        let variant = "en|en|web|about";
        let base =
            storefront_pages_cache_key(tenant, PageCacheGenerationSnapshot::new(1, 2, 3), variant)
                .unwrap();
        for changed in [
            PageCacheGenerationSnapshot::new(2, 2, 3),
            PageCacheGenerationSnapshot::new(1, 3, 3),
            PageCacheGenerationSnapshot::new(1, 2, 4),
        ] {
            assert_ne!(
                base,
                storefront_pages_cache_key(tenant, changed, variant).unwrap()
            );
        }
        assert!(!base.contains("about"));
    }

    #[tokio::test]
    async fn read_runtime_round_trips_bounded_json() {
        let runtime = PagesCacheReadRuntime::new(Arc::new(FakeReadPort {
            generations: PageCacheGenerationSnapshot::new(1, 2, 3),
            values: Mutex::new(HashMap::new()),
        }));
        let key = "pages:test".to_string();
        let value = json!({"page": "home"});
        runtime.put_json(key.clone(), &value).await.unwrap();
        let cached: Option<Value> = runtime.get_json(&key).await.unwrap();
        assert_eq!(cached, Some(value));
    }
}

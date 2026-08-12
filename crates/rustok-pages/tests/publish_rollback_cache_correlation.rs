use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::events::EventHandler;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheError, PageCacheGenerationSnapshot,
    PageCacheInvalidationEventHandler, PageCacheInvalidationPort, PageCacheInvalidationReceipt,
    PageCacheInvalidationRequest, PageCacheScope, PagesCacheInvalidationRuntime,
    PagesCacheReadPort, PagesCacheReadRuntime, page_cache_key, storefront_pages_cache_key,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default)]
struct CorrelationState {
    generations: PageCacheGenerationSnapshot,
    values: HashMap<String, Vec<u8>>,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

struct CorrelatingCachePort {
    state: Mutex<CorrelationState>,
}

impl CorrelatingCachePort {
    fn new(generations: PageCacheGenerationSnapshot) -> Self {
        Self {
            state: Mutex::new(CorrelationState {
                generations,
                ..CorrelationState::default()
            }),
        }
    }

    fn recorded(
        &self,
    ) -> (
        PageCacheGenerationSnapshot,
        Vec<PageCacheInvalidationRequest>,
        Vec<PageCacheInvalidationReceipt>,
    ) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            state.generations,
            state.requests.clone(),
            state.receipts.clone(),
        )
    }
}

#[async_trait]
impl PageCacheInvalidationPort for CorrelatingCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.requests.push(request.clone());

        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let next = state.generations.generation(*scope) + 1;
            state.generations.record(*scope, next);
            receipt.record(*scope, next);
        }
        state.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[async_trait]
impl PagesCacheReadPort for CorrelatingCachePort {
    async fn generation_snapshot(
        &self,
        _tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values
            .get(key)
            .cloned())
    }

    async fn put(&self, key: String, value: Vec<u8>, _ttl: Duration) -> Result<(), PageCacheError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values
            .insert(key, value);
        Ok(())
    }
}

#[tokio::test]
async fn published_event_rotates_generations_and_forces_storefront_and_artifact_miss_refill() {
    let tenant_id = Uuid::from_u128(11);
    let page_id = Uuid::from_u128(22);
    let before = PageCacheGenerationSnapshot::new(3, 5, 7);
    let port = Arc::new(CorrelatingCachePort::new(before));
    let invalidation_port: Arc<dyn PageCacheInvalidationPort> = port.clone();
    let read_port: Arc<dyn PagesCacheReadPort> = port.clone();
    let handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
        invalidation_port,
    ));
    let reads = PagesCacheReadRuntime::new(read_port);

    let storefront_variant = "home|en|en|web";
    let artifact_variant = "en|en|web";
    let old_storefront_key =
        storefront_pages_cache_key(tenant_id, before, storefront_variant).unwrap();
    let old_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        tenant_id,
        page_id,
        before.artifact,
        artifact_variant,
    )
    .unwrap();
    let old_storefront = json!({"generation": "before", "reader": "storefront"});
    let old_artifact = json!({"generation": "before", "reader": "artifact"});
    reads
        .put_json(old_storefront_key.clone(), &old_storefront)
        .await
        .unwrap();
    reads
        .put_json(old_artifact_key.clone(), &old_artifact)
        .await
        .unwrap();
    assert_eq!(
        reads.get_json::<Value>(&old_storefront_key).await.unwrap(),
        Some(old_storefront.clone())
    );
    assert_eq!(
        reads.get_json::<Value>(&old_artifact_key).await.unwrap(),
        Some(old_artifact.clone())
    );

    let envelope = EventEnvelope::new(
        tenant_id,
        Some(Uuid::from_u128(33)),
        DomainEvent::NodePublished {
            node_id: page_id,
            kind: PAGES_CACHE_ENTITY_KIND.to_string(),
        },
    );
    handler.handle(&envelope).await.unwrap();

    let after = reads.generation_snapshot(tenant_id).await.unwrap();
    assert_eq!(after.route, before.route + 1);
    assert_eq!(after.page, before.page + 1);
    assert_eq!(after.artifact, before.artifact + 1);

    let (recorded_generations, requests, receipts) = port.recorded();
    assert_eq!(recorded_generations, after);
    assert_eq!(requests.len(), 1);
    assert_eq!(receipts.len(), 1);
    assert_eq!(requests[0].tenant_id, tenant_id);
    assert_eq!(requests[0].page_id, page_id);
    assert_eq!(requests[0].event_id, envelope.id);
    assert_eq!(requests[0].correlation_id, envelope.correlation_id);
    assert_eq!(
        requests[0].scopes(),
        &[
            PageCacheScope::Route,
            PageCacheScope::Page,
            PageCacheScope::Artifact,
        ]
    );
    assert_eq!(receipts[0].event_id, envelope.id);
    assert_eq!(receipts[0].correlation_id, envelope.correlation_id);
    assert_eq!(receipts[0].route_generation, Some(after.route));
    assert_eq!(receipts[0].page_generation, Some(after.page));
    assert_eq!(receipts[0].artifact_generation, Some(after.artifact));

    let new_storefront_key =
        storefront_pages_cache_key(tenant_id, after, storefront_variant).unwrap();
    let new_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        tenant_id,
        page_id,
        after.artifact,
        artifact_variant,
    )
    .unwrap();
    assert_ne!(new_storefront_key, old_storefront_key);
    assert_ne!(new_artifact_key, old_artifact_key);
    assert_eq!(
        reads.get_json::<Value>(&new_storefront_key).await.unwrap(),
        None
    );
    assert_eq!(
        reads.get_json::<Value>(&new_artifact_key).await.unwrap(),
        None
    );

    let refilled_storefront = json!({"generation": "after", "reader": "storefront"});
    let refilled_artifact = json!({"generation": "after", "reader": "artifact"});
    reads
        .put_json(new_storefront_key.clone(), &refilled_storefront)
        .await
        .unwrap();
    reads
        .put_json(new_artifact_key.clone(), &refilled_artifact)
        .await
        .unwrap();
    assert_eq!(
        reads.get_json::<Value>(&new_storefront_key).await.unwrap(),
        Some(refilled_storefront)
    );
    assert_eq!(
        reads.get_json::<Value>(&new_artifact_key).await.unwrap(),
        Some(refilled_artifact)
    );

    assert_eq!(
        reads.get_json::<Value>(&old_storefront_key).await.unwrap(),
        Some(old_storefront)
    );
    assert_eq!(
        reads.get_json::<Value>(&old_artifact_key).await.unwrap(),
        Some(old_artifact)
    );
}

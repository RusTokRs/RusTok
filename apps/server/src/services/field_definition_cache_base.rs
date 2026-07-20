//! Cache for Flex field definitions schema/list queries.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::FutureExt;
use moka::future::Cache;
use rustok_core::{EventBus, EventConsumerRuntime};
use tokio::task::JoinHandle;
use uuid::Uuid;

use flex::FieldDefinitionView;

use crate::services::server_runtime_context::ServerRuntimeContext;

const FIELD_DEFINITION_CACHE_TTL: Duration = Duration::from_secs(30);
const FIELD_DEFINITION_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const FIELD_DEFINITION_INVALIDATION_RESTART_DELAY: Duration = Duration::from_secs(1);
const FIELD_DEFINITION_INVALIDATION_CHANNEL: &str = "field_definition_cache_invalidator";

#[derive(Clone)]
pub struct FieldDefinitionCache {
    inner: Cache<(Uuid, String), Vec<FieldDefinitionView>>,
}

#[derive(Clone)]
pub struct SharedFieldDefinitionCache(pub Arc<FieldDefinitionCache>);

struct FieldDefinitionCacheInvalidationRuntime {
    cache: Arc<FieldDefinitionCache>,
    task: JoinHandle<()>,
}

impl Drop for FieldDefinitionCacheInvalidationRuntime {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct FieldDefinitionCacheInvalidationHandle(Arc<FieldDefinitionCacheInvalidationRuntime>);

impl FieldDefinitionCacheInvalidationHandle {
    fn new(cache: Arc<FieldDefinitionCache>, task: JoinHandle<()>) -> Self {
        Self(Arc::new(FieldDefinitionCacheInvalidationRuntime {
            cache,
            task,
        }))
    }

    fn cache(&self) -> Arc<FieldDefinitionCache> {
        Arc::clone(&self.0.cache)
    }

    pub fn is_running(&self) -> bool {
        !self.0.task.is_finished()
    }

    fn abort(&self) {
        self.0.task.abort();
    }

    #[cfg(test)]
    fn abort_for_test(&self) {
        self.abort();
    }
}

#[derive(Clone, Default)]
struct FieldDefinitionCacheStartLock(Arc<Mutex<()>>);

impl Default for FieldDefinitionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldDefinitionCache {
    pub fn new() -> Self {
        Self::with_max_weight(FIELD_DEFINITION_CACHE_MAX_WEIGHT_BYTES)
    }

    fn with_max_weight(max_weight_bytes: u64) -> Self {
        let inner = Cache::builder()
            .time_to_live(FIELD_DEFINITION_CACHE_TTL)
            .weigher(field_definition_entry_weight)
            .max_capacity(max_weight_bytes)
            .build();

        Self { inner }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
    ) -> Option<Vec<FieldDefinitionView>> {
        self.inner.get(&(tenant_id, entity_type.to_string())).await
    }

    pub async fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>) {
        self.inner
            .insert((tenant_id, entity_type.to_string()), rows)
            .await;
    }

    pub async fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        self.inner
            .invalidate(&(tenant_id, entity_type.to_string()))
            .await;
    }

    /// Invalidate every cached schema after the event consumer reports lag.
    ///
    /// A lagged broadcast receiver has permanently skipped an unknown subset of
    /// field-definition changes. Keeping any entry at that point can serve an
    /// obsolete schema until TTL expiry, so the only safe recovery is a bounded
    /// full-cache invalidation.
    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }
}

fn field_definition_entry_weight(key: &(Uuid, String), rows: &Vec<FieldDefinitionView>) -> u32 {
    let mut weight = std::mem::size_of::<Uuid>()
        .saturating_add(key.1.len())
        .saturating_add(std::mem::size_of::<Vec<FieldDefinitionView>>());

    for row in rows {
        weight = weight
            .saturating_add(std::mem::size_of::<FieldDefinitionView>())
            .saturating_add(row.field_key.len())
            .saturating_add(row.field_type.len())
            .saturating_add(json_value_weight(&row.label))
            .saturating_add(row.description.as_ref().map_or(0, json_value_weight))
            .saturating_add(row.default_value.as_ref().map_or(0, json_value_weight))
            .saturating_add(row.validation.as_ref().map_or(0, json_value_weight))
            .saturating_add(row.created_at.len())
            .saturating_add(row.updated_at.len());
    }

    weight.clamp(1, u32::MAX as usize) as u32
}

fn json_value_weight(value: &serde_json::Value) -> usize {
    serde_json::to_vec(value)
        .map(|encoded| encoded.len())
        .unwrap_or(std::mem::size_of::<serde_json::Value>())
}

pub fn field_definition_cache_from_context(
    ctx: &ServerRuntimeContext,
    bus: EventBus,
) -> FieldDefinitionCache {
    let _ = ctx.shared_insert_if_absent(FieldDefinitionCacheStartLock::default());
    let start_lock = ctx
        .shared_get::<FieldDefinitionCacheStartLock>()
        .expect("field definition cache start lock must be available");
    let _start_guard = start_lock
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let cache = match ctx.shared_get::<FieldDefinitionCacheInvalidationHandle>() {
        Some(existing) if existing.is_running() => return (*existing.cache()).clone(),
        Some(existing) => {
            tracing::warn!(
                "Field definition cache invalidation consumer stopped; replacing runtime"
            );
            let cache = existing.cache();
            existing.abort();
            drop(existing);
            let _ = ctx.shared_take::<FieldDefinitionCacheInvalidationHandle>();
            cache
        }
        None => Arc::new(FieldDefinitionCache::new()),
    };

    let task = spawn_field_definition_cache_invalidation_consumer(bus, Arc::clone(&cache));
    let handle = FieldDefinitionCacheInvalidationHandle::new(Arc::clone(&cache), task);
    ctx.shared_insert(handle);
    ctx.shared_insert(SharedFieldDefinitionCache(Arc::clone(&cache)));

    (*cache).clone()
}

fn spawn_field_definition_cache_invalidation_consumer(
    bus: EventBus,
    cache: Arc<FieldDefinitionCache>,
) -> JoinHandle<()> {
    // Subscribe before committing the runtime handle so no event can fall into
    // the startup gap. Later supervisor attempts create a fresh subscription.
    let first_receiver = Arc::new(Mutex::new(Some(bus.subscribe())));
    tokio::spawn(supervise_field_definition_cache_invalidation(
        move || {
            let mut receiver = first_receiver
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| bus.subscribe());
            let cache = Arc::clone(&cache);
            async move {
                let consumer_runtime =
                    EventConsumerRuntime::new(FIELD_DEFINITION_INVALIDATION_CHANNEL);
                consumer_runtime.restarted("startup");
                loop {
                    match receiver.recv().await {
                        Ok(envelope) => {
                            if let Some((tenant_id, entity_type)) =
                                flex::field_definition_cache_invalidation_target(&envelope.event)
                            {
                                cache.invalidate(tenant_id, entity_type).await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            consumer_runtime.lagged(skipped);
                            cache.invalidate_all();
                            tracing::warn!(
                                skipped,
                                "Field definition cache invalidation consumer lagged; cleared all cached schemas"
                            );
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            consumer_runtime.closed();
                            return;
                        }
                    }
                }
            }
        },
        FIELD_DEFINITION_INVALIDATION_RESTART_DELAY,
    ))
}

async fn supervise_field_definition_cache_invalidation<F, Fut>(
    mut worker_factory: F,
    restart_delay: Duration,
) where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    loop {
        let worker = match std::panic::catch_unwind(AssertUnwindSafe(&mut worker_factory)) {
            Ok(worker) => worker,
            Err(_) => {
                record_field_definition_consumer_restart("factory_panicked");
                tokio::time::sleep(restart_delay).await;
                continue;
            }
        };
        let outcome = AssertUnwindSafe(worker).catch_unwind().await;
        if outcome.is_err() {
            record_field_definition_consumer_restart("worker_panicked");
        } else {
            record_field_definition_consumer_restart("worker_exited");
        }
        tokio::time::sleep(restart_delay).await;
    }
}

fn record_field_definition_consumer_restart(reason: &'static str) {
    tracing::error!(
        reason,
        "Field definition cache invalidation consumer stopped; restarting"
    );
    rustok_telemetry::metrics::record_event_error(FIELD_DEFINITION_INVALIDATION_CHANNEL, reason);
}

#[async_trait]
impl flex::FieldDefinitionCachePort for FieldDefinitionCache {
    async fn get(&self, tenant_id: Uuid, entity_type: &str) -> Option<Vec<FieldDefinitionView>> {
        FieldDefinitionCache::get(self, tenant_id, entity_type).await
    }

    async fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>) {
        FieldDefinitionCache::set(self, tenant_id, entity_type, rows).await;
    }

    async fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        FieldDefinitionCache::invalidate(self, tenant_id, entity_type).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FieldDefinitionCache, FieldDefinitionCacheInvalidationHandle,
        field_definition_cache_from_context, field_definition_entry_weight,
        supervise_field_definition_cache_invalidation,
    };
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use flex::FieldDefinitionView;
    use rustok_core::EventBus;
    use rustok_events::{DomainEvent, EventEnvelope};
    use sea_orm::Database;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::sleep;
    use uuid::Uuid;

    fn mock_view(field_key: &str) -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: field_key.to_string(),
            field_type: "text".to_string(),
            label: json!({"en": field_key}),
            description: None,
            is_localized: false,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn entry_weight_accounts_for_dynamic_schema_payloads() {
        let tenant_id = Uuid::new_v4();
        let rows = vec![mock_view("nickname")];
        let weight = field_definition_entry_weight(&(tenant_id, "user".to_string()), &rows);

        assert!(weight as usize >= "user".len() + "nickname".len());
    }

    #[tokio::test]
    async fn oversized_schema_is_not_retained_beyond_weight_budget() {
        let cache = FieldDefinitionCache::with_max_weight(128);
        let tenant_id = Uuid::new_v4();
        let mut row = mock_view("large");
        row.description = Some(json!({"en": "x".repeat(2_048)}));

        cache.set(tenant_id, "user", vec![row]).await;
        cache.inner.run_pending_tasks().await;

        assert!(cache.get(tenant_id, "user").await.is_none());
    }

    #[tokio::test]
    async fn cache_set_get_and_invalidate() {
        let cache = FieldDefinitionCache::new();
        let tenant_id = Uuid::new_v4();
        let entity_type = "user";

        cache
            .set(tenant_id, entity_type, vec![mock_view("nickname")])
            .await;

        let cached = cache.get(tenant_id, entity_type).await;
        assert!(cached.is_some());
        assert_eq!(cached.expect("cache entry")[0].field_key, "nickname");

        cache.invalidate(tenant_id, entity_type).await;
        assert!(cache.get(tenant_id, entity_type).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_all_clears_every_tenant_schema() {
        let cache = FieldDefinitionCache::new();
        let first_tenant = Uuid::new_v4();
        let second_tenant = Uuid::new_v4();

        cache
            .set(first_tenant, "user", vec![mock_view("nickname")])
            .await;
        cache
            .set(second_tenant, "order", vec![mock_view("reference")])
            .await;

        cache.invalidate_all();

        assert!(cache.get(first_tenant, "user").await.is_none());
        assert!(cache.get(second_tenant, "order").await.is_none());
    }

    async fn assert_event_bus_invalidation_drops_cached_field_definitions(event: DomainEvent) {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite db should connect");
        let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let bus = EventBus::default();
        let cache = field_definition_cache_from_context(&runtime_ctx, bus.clone());
        let (tenant_id, entity_type) = match &event {
            DomainEvent::FieldDefinitionCreated {
                tenant_id,
                entity_type,
                ..
            }
            | DomainEvent::FieldDefinitionUpdated {
                tenant_id,
                entity_type,
                ..
            }
            | DomainEvent::FieldDefinitionDeleted {
                tenant_id,
                entity_type,
                ..
            } => (*tenant_id, entity_type.clone()),
            _ => panic!("test helper expects a FieldDefinition event"),
        };

        cache
            .set(tenant_id, &entity_type, vec![mock_view("nickname")])
            .await;
        assert!(cache.get(tenant_id, &entity_type).await.is_some());

        bus.publish_envelope(EventEnvelope::new(tenant_id, None, event))
            .expect("field definition event should publish");

        for _ in 0..20 {
            if cache.get(tenant_id, &entity_type).await.is_none() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }

        panic!("cache entry should be invalidated after field definition event");
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_create() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionCreated {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
                field_type: "text".to_string(),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_update() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionUpdated {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_delete() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionDeleted {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn invalidation_handle_reports_terminal_worker() {
        let cache = Arc::new(FieldDefinitionCache::new());
        let handle = FieldDefinitionCacheInvalidationHandle::new(
            cache,
            tokio::spawn(async { std::future::pending::<()>().await }),
        );
        assert!(handle.is_running());
        handle.abort_for_test();
        tokio::task::yield_now().await;
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn invalidation_supervisor_restarts_after_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = Arc::clone(&attempts);
        let supervisor = tokio::spawn(supervise_field_definition_cache_invalidation(
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        panic!("field definition invalidation regression fixture");
                    }
                    std::future::pending::<()>().await;
                }
            },
            Duration::from_millis(1),
        ));

        tokio::time::timeout(Duration::from_secs(1), async {
            while attempts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("field definition invalidation supervisor should restart worker");
        supervisor.abort();
    }
}

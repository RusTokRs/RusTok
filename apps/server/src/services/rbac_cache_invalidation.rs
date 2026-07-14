use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use rustok_cache::{
    cache_backend_generation_snapshot, observe_cache_backend_generation,
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    CacheBackendGenerationError, CacheInvalidationMessage, CacheInvalidationObservation,
    CacheInvalidationPayloadError, CacheService, DurableCacheInvalidationRecord,
    VersionedCacheInvalidation,
};
use sea_orm::{EntityTrait, QuerySelect};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::_entities::users;
use crate::services::rbac_runtime::invalidate_user_permissions_cache;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const RBAC_PERMISSION_GENERATION_PREFIX: &str = "rustok:rbac:permissions:v1";
pub const RBAC_PERMISSION_INVALIDATION_CHANNEL: &str = "rbac.permissions.generation.v1";
const RBAC_PERMISSION_INVALIDATION_CAUSE: &str = "rbac.user.permissions.changed";
const RBAC_PERMISSION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

static RBAC_INVALIDATION_CACHE_SERVICE: Lazy<RwLock<Option<CacheService>>> =
    Lazy::new(|| RwLock::new(None));

#[derive(Clone)]
pub struct RbacCacheInvalidationListenerHandle;

#[derive(Clone, Default)]
struct RbacCacheInvalidationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

fn observe_rbac_backend_generation(generation: u64) -> Result<u64> {
    match observe_cache_backend_generation(RBAC_PERMISSION_GENERATION_PREFIX, generation) {
        Ok(snapshot) => Ok(snapshot.generation),
        Err(CacheBackendGenerationError::GenerationRegressed { current, proposed })
            if current >= proposed =>
        {
            Ok(current)
        }
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

fn acknowledge_rbac_applied_generation(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_applied(RBAC_PERMISSION_INVALIDATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

fn acknowledge_rbac_recovery(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_recovery(RBAC_PERMISSION_INVALIDATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

#[derive(Clone)]
struct RbacCacheInvalidationListener {
    ctx: ServerRuntimeContext,
    cache: CacheService,
    tracker: BoundedCacheInvalidationGapTracker,
}

impl RbacCacheInvalidationListener {
    fn new(ctx: ServerRuntimeContext, cache: CacheService) -> Self {
        Self {
            ctx,
            cache,
            tracker: BoundedCacheInvalidationGapTracker::default(),
        }
    }

    async fn read_generation(&self) -> Result<u64> {
        if self.cache.redis_configuration_present() {
            if !self.cache.redis_client_initialized() {
                return Err(Error::Cache(
                    "Redis is configured but the RBAC invalidation client is unavailable"
                        .to_string(),
                ));
            }
            return self
                .cache
                .namespace_generations()
                .read(RBAC_PERMISSION_GENERATION_PREFIX)
                .await
                .map(|generation| generation.value())
                .map_err(|error| Error::Cache(error.to_string()));
        }

        let snapshot = cache_backend_generation_snapshot(RBAC_PERMISSION_GENERATION_PREFIX)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if snapshot.trusted {
            Ok(snapshot.generation)
        } else {
            observe_cache_backend_generation(RBAC_PERMISSION_GENERATION_PREFIX, 0)
                .map(|snapshot| snapshot.generation)
                .map_err(|error| Error::Cache(error.to_string()))
        }
    }

    async fn recover_generation_and_clear(&self) -> Result<u64> {
        let generation = self.read_generation().await?;
        let recovered_through = observe_rbac_backend_generation(generation)?;
        invalidate_all_user_permission_snapshots(&self.ctx).await?;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        Ok(recovered_through)
    }

    async fn reconcile_generation_if_advanced(&self) -> Result<Option<u64>> {
        let generation = self.read_generation().await?;
        let recovered_through = observe_rbac_backend_generation(generation)?;
        if self
            .tracker
            .last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL)
            .is_some_and(|current| current >= recovered_through)
        {
            return Ok(None);
        }

        invalidate_all_user_permission_snapshots(&self.ctx).await?;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        Ok(Some(recovered_through))
    }

    async fn handle_message(&self, message: CacheInvalidationMessage) -> Result<()> {
        let event = VersionedCacheInvalidation::from_message(&message)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if event.channel != RBAC_PERMISSION_INVALIDATION_CHANNEL {
            return Err(Error::Validation(format!(
                "unexpected RBAC cache invalidation channel {}",
                event.channel
            )));
        }

        match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                let (tenant_id, user_id) = parse_rbac_invalidation_key(&event.key)?;
                invalidate_user_permissions_cache(&tenant_id, &user_id).await;
                acknowledge_rbac_applied_generation(&self.tracker, generation)?;
            }
            CacheInvalidationObservation::Duplicate { .. }
            | CacheInvalidationObservation::Stale { .. } => {}
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                let recovered = self.recover_generation_and_clear().await?;
                if recovered < event.generation {
                    return Err(Error::Cache(format!(
                        "shared RBAC invalidation generation {recovered} trails received {}",
                        event.generation
                    )));
                }
            }
        }

        Ok(())
    }
}

pub async fn publish_user_rbac_invalidation(
    tenant_id: &Uuid,
    user_id: &Uuid,
) -> Result<()> {
    let Some(cache) = RBAC_INVALIDATION_CACHE_SERVICE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    else {
        tracing::warn!(
            %tenant_id,
            %user_id,
            "RBAC distributed invalidation is not initialized; local cache invalidation only"
        );
        return Ok(());
    };
    let generation = cache
        .bump_cache_backend_generation(RBAC_PERMISSION_GENERATION_PREFIX)
        .await
        .map_err(|error| Error::Cache(error.to_string()))?;
    let emitted_at_unix_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Error::Cache(error.to_string()))?
            .as_millis(),
    )
    .map_err(|_| Error::Cache("RBAC invalidation timestamp overflow".to_string()))?;
    let record = DurableCacheInvalidationRecord::new(
        Uuid::new_v4(),
        Some(*tenant_id),
        RBAC_PERMISSION_INVALIDATION_CHANNEL,
        rbac_invalidation_key(*tenant_id, *user_id),
        generation.generation,
        emitted_at_unix_ms,
        RBAC_PERMISSION_INVALIDATION_CAUSE,
        None,
    )
    .map_err(|error| Error::Cache(error.to_string()))?;
    let outcome = cache
        .invalidations()
        .publish_durable(&record)
        .await
        .map_err(|error| Error::Cache(error.to_string()))?;

    if cache.redis_configuration_present() {
        if !outcome.redis_published {
            return Err(Error::Cache(
                "RBAC permission cache generation advanced but Redis publish failed".to_string(),
            ));
        }
    } else if outcome.local_subscribers == 0 {
        return Err(Error::Cache(
            "RBAC permission cache generation advanced without a local subscriber".to_string(),
        ));
    }

    Ok(())
}

pub async fn start_rbac_cache_invalidation_listener(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) -> Result<()> {
    let _ = ctx.shared_insert_if_absent(RbacCacheInvalidationListenerStartLock::default());
    let start_lock = ctx
        .shared_get::<RbacCacheInvalidationListenerStartLock>()
        .ok_or_else(|| Error::Cache("RBAC invalidation start lock is unavailable".to_string()))?;
    let _start_guard = start_lock.0.lock().await;

    if ctx
        .shared_get::<RbacCacheInvalidationListenerHandle>()
        .is_some()
    {
        return Ok(());
    }

    let listener = RbacCacheInvalidationListener::new(ctx.clone(), cache.clone());
    let mut local = cache
        .invalidations()
        .subscribe_local_channel(RBAC_PERMISSION_INVALIDATION_CHANNEL);
    listener.recover_generation_and_clear().await?;

    let local_listener = listener.clone();
    tokio::spawn(async move {
        loop {
            match local.recv().await {
                Ok(message) => {
                    if let Err(error) = local_listener.handle_message(message).await {
                        tracing::error!(%error, "Local RBAC cache invalidation apply failed");
                        rustok_telemetry::metrics::record_event_error(
                            RBAC_PERMISSION_INVALIDATION_CHANNEL,
                            "local_apply",
                        );
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "RBAC cache invalidation listener lagged; clearing all permission snapshots");
                    if let Err(error) = local_listener.recover_generation_and_clear().await {
                        tracing::error!(%error, "RBAC cache invalidation recovery after local lag failed");
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::error!("Local RBAC cache invalidation subscription closed");
                    break;
                }
            }
        }
    });

    if cache.redis_client_initialized() {
        let redis_listener = listener.clone();
        let invalidations = cache.invalidations();
        tokio::spawn(async move {
            loop {
                let ready_listener = redis_listener.clone();
                let handler_listener = redis_listener.clone();
                let result = invalidations
                    .consume_subscription_with_ready(
                        RBAC_PERMISSION_INVALIDATION_CHANNEL,
                        move || {
                            let ready_listener = ready_listener.clone();
                            async move {
                                if let Err(error) = ready_listener.recover_generation_and_clear().await
                                {
                                    tracing::error!(%error, "RBAC cache recovery after Redis subscribe failed");
                                }
                            }
                        },
                        move |message| {
                            let handler_listener = handler_listener.clone();
                            async move {
                                if let Err(error) = handler_listener.handle_message(message).await {
                                    tracing::error!(%error, "Redis RBAC cache invalidation apply failed");
                                    rustok_telemetry::metrics::record_event_error(
                                        RBAC_PERMISSION_INVALIDATION_CHANNEL,
                                        "redis_apply",
                                    );
                                }
                            }
                        },
                    )
                    .await;
                tracing::warn!(?result, "RBAC Redis invalidation subscription stopped; restarting");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        let reconcile_listener = listener.clone();
        tokio::spawn(async move {
            let start = tokio::time::Instant::now() + RBAC_PERMISSION_RECONCILE_INTERVAL;
            let mut interval =
                tokio::time::interval_at(start, RBAC_PERMISSION_RECONCILE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                match reconcile_listener.reconcile_generation_if_advanced().await {
                    Ok(Some(generation)) => {
                        tracing::warn!(generation, "Reconciled missed RBAC cache invalidations");
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::error!(%error, "Periodic RBAC cache invalidation reconciliation failed");
                        rustok_telemetry::metrics::record_event_error(
                            RBAC_PERMISSION_INVALIDATION_CHANNEL,
                            "periodic_reconciliation",
                        );
                    }
                }
            }
        });
    }

    *RBAC_INVALIDATION_CACHE_SERVICE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cache);
    ctx.shared_insert(RbacCacheInvalidationListenerHandle);
    Ok(())
}

async fn invalidate_all_user_permission_snapshots(ctx: &ServerRuntimeContext) -> Result<()> {
    let identities = users::Entity::find()
        .select_only()
        .column(users::Column::TenantId)
        .column(users::Column::Id)
        .into_tuple::<(Uuid, Uuid)>()
        .all(ctx.db())
        .await?;
    for (tenant_id, user_id) in identities {
        invalidate_user_permissions_cache(&tenant_id, &user_id).await;
    }
    Ok(())
}

fn rbac_invalidation_key(tenant_id: Uuid, user_id: Uuid) -> String {
    format!("{tenant_id}:{user_id}")
}

fn parse_rbac_invalidation_key(value: &str) -> Result<(Uuid, Uuid)> {
    let (tenant_id, user_id) = value.split_once(':').ok_or_else(|| {
        Error::Validation("malformed RBAC cache invalidation key".to_string())
    })?;
    Ok((
        Uuid::parse_str(tenant_id)
            .map_err(|_| Error::Validation("invalid RBAC invalidation tenant id".to_string()))?,
        Uuid::parse_str(user_id)
            .map_err(|_| Error::Validation("invalid RBAC invalidation user id".to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        acknowledge_rbac_applied_generation, acknowledge_rbac_recovery,
        parse_rbac_invalidation_key, rbac_invalidation_key,
        RBAC_PERMISSION_INVALIDATION_CHANNEL,
    };
    use rustok_cache::BoundedCacheInvalidationGapTracker;
    use uuid::Uuid;

    #[test]
    fn rbac_invalidation_key_round_trips() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        assert_eq!(
            parse_rbac_invalidation_key(&rbac_invalidation_key(tenant_id, user_id)).unwrap(),
            (tenant_id, user_id)
        );
    }

    #[test]
    fn malformed_rbac_invalidation_key_is_rejected() {
        assert!(parse_rbac_invalidation_key("not-a-pair").is_err());
        assert!(parse_rbac_invalidation_key("invalid:also-invalid").is_err());
    }

    #[test]
    fn superseded_rbac_acknowledgements_are_safe_noops() {
        let tracker = BoundedCacheInvalidationGapTracker::default();
        tracker
            .seed(RBAC_PERMISSION_INVALIDATION_CHANNEL, 7)
            .unwrap();

        acknowledge_rbac_applied_generation(&tracker, 6).unwrap();
        acknowledge_rbac_recovery(&tracker, 6).unwrap();

        assert_eq!(
            tracker.last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL),
            Some(7)
        );
    }
}

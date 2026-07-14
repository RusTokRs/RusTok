use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use rustok_cache::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    CacheInvalidationMessage, CacheInvalidationObservation, CacheInvalidationOutcome,
    CacheInvalidationPayloadError, CacheService, DurableCacheInvalidationRecord,
    VersionedCacheInvalidation,
};
use sea_orm::DatabaseConnection;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::rbac_invalidation_generation::{
    ensure_rbac_invalidation_generation_state, read_rbac_invalidation_generation,
    RbacInvalidationGenerationState,
};
use crate::services::rbac_runtime::{
    invalidate_all_user_permissions_cache, invalidate_user_permissions_cache,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const RBAC_PERMISSION_INVALIDATION_CHANNEL: &str = "rbac.permissions.generation.v1";
const RBAC_PERMISSION_INVALIDATION_CAUSE: &str = "rbac.user.permissions.changed";
const RBAC_PERMISSION_INVALIDATE_ALL_KEY: &str = "*";
const RBAC_PERMISSION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

static RBAC_INVALIDATION_CACHE_SERVICE: Lazy<RwLock<Option<CacheService>>> =
    Lazy::new(|| RwLock::new(None));

#[derive(Clone)]
pub struct RbacCacheInvalidationListenerHandle;

#[derive(Clone, Default)]
struct RbacCacheInvalidationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum RbacInvalidationTarget {
    All,
    User { tenant_id: Uuid, user_id: Uuid },
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
    db: DatabaseConnection,
    durable_state: RbacInvalidationGenerationState,
    tracker: BoundedCacheInvalidationGapTracker,
}

impl RbacCacheInvalidationListener {
    fn new(
        db: DatabaseConnection,
        durable_state: RbacInvalidationGenerationState,
    ) -> Self {
        Self {
            db,
            durable_state,
            tracker: BoundedCacheInvalidationGapTracker::default(),
        }
    }

    async fn read_generation(&self) -> Result<u64> {
        read_rbac_invalidation_generation(&self.db).await
    }

    async fn recover_generation_and_clear(&self) -> Result<u64> {
        let recovered_through = self.read_generation().await?;
        invalidate_all_user_permissions_cache().await;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        self.durable_state.observe_applied(recovered_through);
        Ok(recovered_through)
    }

    async fn reconcile_generation_if_advanced(&self) -> Result<Option<u64>> {
        let recovered_through = self.read_generation().await?;
        let tracker_current = self
            .tracker
            .last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL);
        let process_current = self.durable_state.current();

        if process_current.is_some_and(|current| current >= recovered_through) {
            if !tracker_current.is_some_and(|current| current >= recovered_through) {
                acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
            }
            return Ok(None);
        }

        invalidate_all_user_permissions_cache().await;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        self.durable_state.observe_applied(recovered_through);
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
                match parse_rbac_invalidation_target(&event.key)? {
                    RbacInvalidationTarget::All => {
                        invalidate_all_user_permissions_cache().await;
                    }
                    RbacInvalidationTarget::User { tenant_id, user_id } => {
                        invalidate_user_permissions_cache(&tenant_id, &user_id).await;
                    }
                }
                acknowledge_rbac_applied_generation(&self.tracker, generation)?;
                self.durable_state.observe_applied(generation);
            }
            CacheInvalidationObservation::Duplicate { generation }
            | CacheInvalidationObservation::Stale { generation, .. } => {
                self.durable_state.observe_applied(generation);
            }
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                let recovered = self.recover_generation_and_clear().await?;
                if recovered < event.generation {
                    return Err(Error::Cache(format!(
                        "durable RBAC invalidation generation {recovered} trails received {}",
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
    generation: u64,
) -> Result<()> {
    publish_rbac_invalidation(
        Some(*tenant_id),
        rbac_invalidation_key(*tenant_id, *user_id),
        generation,
    )
    .await
}

pub async fn publish_all_rbac_invalidation(generation: u64) -> Result<()> {
    publish_rbac_invalidation(
        None,
        RBAC_PERMISSION_INVALIDATE_ALL_KEY.to_string(),
        generation,
    )
    .await
}

async fn publish_rbac_invalidation(
    tenant_id: Option<Uuid>,
    key: String,
    generation: u64,
) -> Result<()> {
    let Some(cache) = RBAC_INVALIDATION_CACHE_SERVICE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    else {
        tracing::warn!(
            ?tenant_id,
            %key,
            generation,
            "RBAC distributed invalidation is not initialized; durable generation reconciliation will recover"
        );
        return Ok(());
    };

    let fanout: Result<CacheInvalidationOutcome> = async {
        let emitted_at_unix_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| Error::Cache(error.to_string()))?
                .as_millis(),
        )
        .map_err(|_| Error::Cache("RBAC invalidation timestamp overflow".to_string()))?;
        let record = DurableCacheInvalidationRecord::new(
            Uuid::new_v4(),
            tenant_id,
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            key.clone(),
            generation,
            emitted_at_unix_ms,
            RBAC_PERMISSION_INVALIDATION_CAUSE,
            None,
        )
        .map_err(|error| Error::Cache(error.to_string()))?;
        cache
            .invalidations()
            .publish_durable(&record)
            .await
            .map_err(|error| Error::Cache(error.to_string()))
    }
    .await;

    let outcome = match fanout {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(
                ?tenant_id,
                %key,
                generation,
                %error,
                "RBAC invalidation fan-out deferred to durable generation reconciliation"
            );
            rustok_telemetry::metrics::record_event_error(
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                "fanout_deferred",
            );
            return Ok(());
        }
    };

    if cache.redis_configuration_present() {
        if !outcome.redis_published {
            tracing::warn!(
                ?tenant_id,
                %key,
                generation,
                "RBAC invalidation publication deferred to durable generation reconciliation"
            );
            rustok_telemetry::metrics::record_event_error(
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                "redis_publish_deferred",
            );
        }
    } else if outcome.local_subscribers == 0 {
        tracing::warn!(
            ?tenant_id,
            %key,
            generation,
            "Local RBAC invalidation delivery deferred to durable generation reconciliation"
        );
        rustok_telemetry::metrics::record_event_error(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            "local_publish_deferred",
        );
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

    let durable_state = ensure_rbac_invalidation_generation_state(ctx);
    let listener = RbacCacheInvalidationListener::new(ctx.db_clone(), durable_state);
    let mut local = cache
        .invalidations()
        .subscribe_local_channel(RBAC_PERMISSION_INVALIDATION_CHANNEL);
    if let Err(error) = listener.recover_generation_and_clear().await {
        tracing::warn!(
            %error,
            "Initial RBAC invalidation recovery is unavailable; durable watchdog will retry"
        );
        invalidate_all_user_permissions_cache().await;
        rustok_telemetry::metrics::record_event_error(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            "startup_recovery_deferred",
        );
    }

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
    }

    let reconcile_listener = listener.clone();
    tokio::spawn(async move {
        let start = tokio::time::Instant::now() + RBAC_PERMISSION_RECONCILE_INTERVAL;
        let mut interval = tokio::time::interval_at(start, RBAC_PERMISSION_RECONCILE_INTERVAL);
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

    *RBAC_INVALIDATION_CACHE_SERVICE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cache);
    ctx.shared_insert(RbacCacheInvalidationListenerHandle);
    Ok(())
}

fn rbac_invalidation_key(tenant_id: Uuid, user_id: Uuid) -> String {
    format!("{tenant_id}:{user_id}")
}

fn parse_rbac_invalidation_target(value: &str) -> Result<RbacInvalidationTarget> {
    if value == RBAC_PERMISSION_INVALIDATE_ALL_KEY {
        return Ok(RbacInvalidationTarget::All);
    }
    let (tenant_id, user_id) = parse_rbac_invalidation_key(value)?;
    Ok(RbacInvalidationTarget::User { tenant_id, user_id })
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
        parse_rbac_invalidation_key, parse_rbac_invalidation_target, rbac_invalidation_key,
        RbacInvalidationTarget, RBAC_PERMISSION_INVALIDATE_ALL_KEY,
        RBAC_PERMISSION_INVALIDATION_CHANNEL,
    };
    use rustok_cache::BoundedCacheInvalidationGapTracker;
    use uuid::Uuid;

    #[test]
    fn rbac_invalidation_key_round_trips() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        assert_eq!(
            parse_rbac_invalidation_target(&rbac_invalidation_key(tenant_id, user_id)).unwrap(),
            RbacInvalidationTarget::User { tenant_id, user_id }
        );
    }

    #[test]
    fn namespace_wide_invalidation_key_is_explicit() {
        assert_eq!(
            parse_rbac_invalidation_target(RBAC_PERMISSION_INVALIDATE_ALL_KEY).unwrap(),
            RbacInvalidationTarget::All
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

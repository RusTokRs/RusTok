//! Heartbeat dispatch loop for module lifecycle hook execution.

use std::time::Duration;
use sea_orm::DatabaseConnection;

use crate::{
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, StaticTenantLifecycleStore,
};

/// Claim leases last 300 seconds; renewing every minute leaves several
/// missed-heartbeat opportunities before a claim becomes uncertain. Expiry
/// does not permit another command to reclaim the aggregate.
pub(crate) const LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(crate) enum ClaimHeartbeatDispatchError {
    Hook(String),
    ClaimLost { module_slug: String },
    Persistence(String),
}

async fn renew_lifecycle_claim(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<(), ClaimHeartbeatDispatchError> {
    match StaticTenantLifecycleStore::heartbeat(
        db,
        tenant_id,
        module_slug,
        expected_revision,
        idempotency_key,
    ).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ClaimHeartbeatDispatchError::ClaimLost {
            module_slug: module_slug.to_string(),
        }),
        Err(error) => Err(ClaimHeartbeatDispatchError::Persistence(
            crate::executor::map_static_lifecycle_store_error(error).to_string(),
        )),
    }
}

/// One claim-lease renewal tick. The named future can be re-armed by the
/// dispatch loop after each successful renewal.
async fn lifecycle_claim_heartbeat_tick(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<(), ClaimHeartbeatDispatchError> {
    tokio::time::sleep(LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL).await;
    renew_lifecycle_claim(db, tenant_id, module_slug, expected_revision, idempotency_key).await
}

/// Dispatches one lifecycle hook while renewing the exact aggregate claim.
/// An observed expired, replaced, or wrong-revision claim rejects dispatch or
/// its successful result. This does not cancel already-issued external effects
/// or fence the interval between the last check and a later state transaction.
pub(crate) async fn dispatch_lifecycle_with_claim_heartbeat(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    settings: &serde_json::Value,
    phase: ModuleLifecycleHookPhase,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<(), ClaimHeartbeatDispatchError> {
    // Do not start another external hook using a claim already lost while
    // preparing its inputs. This check is not a lock over external effects.
    renew_lifecycle_claim(db, tenant_id, module_slug, expected_revision, idempotency_key).await?;
    let dispatch = dispatcher.dispatch_lifecycle(db, tenant_id, module_slug, settings, phase);
    tokio::pin!(dispatch);
    // Poll the heartbeat as an independent future rather than awaiting it
    // inside the timer branch. A hook may temporarily hold the only pooled
    // SQLite connection; the dispatch future must keep making progress so
    // it can release that connection for the heartbeat query.
    let mut heartbeat = Box::pin(lifecycle_claim_heartbeat_tick(
        db,
        tenant_id,
        module_slug,
        expected_revision,
        idempotency_key,
    ));
    loop {
        tokio::select! {
            biased;
            result = &mut dispatch => {
                // The in-flight heartbeat can hold the only pooled connection.
                // Cancel it before doing the final ownership check.
                drop(heartbeat);
                result.map_err(|error| ClaimHeartbeatDispatchError::Hook(error.to_string()))?;
                // `biased` selects a completed hook even if a failed heartbeat
                // is ready simultaneously. Never report success without a
                // fresh, live claim check after the hook returns.
                renew_lifecycle_claim(db, tenant_id, module_slug, expected_revision, idempotency_key).await?;
                return Ok(());
            }
            heartbeat_result = &mut heartbeat => {
                heartbeat_result?;
                heartbeat = Box::pin(lifecycle_claim_heartbeat_tick(
                    db,
                    tenant_id,
                    module_slug,
                    expected_revision,
                    idempotency_key,
                ));
            }
        }
    }
}

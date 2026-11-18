//! Heartbeat dispatch loop for module lifecycle hook execution.

use std::time::Duration;
use sea_orm::DatabaseConnection;

use crate::{
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, StaticTenantLifecycleStore,
    StaticTenantLifecycleStoreError,
};

/// Claim leases last 300 seconds; renewing every minute leaves several
/// missed-heartbeat opportunities before another command may reclaim the
/// aggregate.
pub(crate) const LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(crate) enum ClaimHeartbeatDispatchError {
    Hook(String),
    ClaimLost { module_slug: String },
    Persistence(String),
}

/// One claim-lease renewal tick. Extracted as a named future so the
/// dispatch loop can re-arm the same pinned future type after each
/// successful renewal (distinct `async move` blocks are distinct types).
async fn lifecycle_claim_heartbeat_tick(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<bool, StaticTenantLifecycleStoreError> {
    tokio::time::sleep(LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL).await;
    StaticTenantLifecycleStore::heartbeat(
        db,
        tenant_id,
        module_slug,
        expected_revision,
        idempotency_key,
    )
    .await
}

/// Dispatches one lifecycle hook while renewing the exact aggregate claim.
/// If the claim lease has already been reclaimed or the revision advanced,
/// dispatch is abandoned before the caller performs any further durable
/// lifecycle write under the stale claim.
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
    let dispatch = dispatcher.dispatch_lifecycle(db, tenant_id, module_slug, settings, phase);
    tokio::pin!(dispatch);
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
                return result.map_err(|error| {
                    ClaimHeartbeatDispatchError::Hook(error.to_string())
                });
            }
            heartbeat_result = &mut heartbeat => {
                match heartbeat_result {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(ClaimHeartbeatDispatchError::ClaimLost {
                            module_slug: module_slug.to_string(),
                        });
                    }
                    Err(error) => {
                        return Err(ClaimHeartbeatDispatchError::Persistence(error.to_string()));
                    }
                }
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

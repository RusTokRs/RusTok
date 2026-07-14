use std::sync::{Arc, RwLock};
use std::time::Duration;

use sea_orm::{ConnectionTrait, Statement};

use crate::error::{Error, Result};
use crate::services::rbac_runtime::invalidate_all_user_permissions_cache;
use crate::services::server_runtime_context::ServerRuntimeContext;

const RBAC_PERMISSION_SCOPE: &str = "permissions";
pub(crate) const RBAC_DURABLE_GENERATION_CHANNEL: &str =
    "rbac.permissions.durable_generation.v1";
const RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const MAX_DURABLE_GENERATION: i64 = i64::MAX;

#[derive(Clone)]
pub struct RbacInvalidationGenerationWatchdogHandle;

#[derive(Clone, Default)]
pub(crate) struct RbacInvalidationGenerationState(
    Arc<RwLock<Option<u64>>>,
);

impl RbacInvalidationGenerationState {
    pub(crate) fn current(&self) -> Option<u64> {
        *self
            .0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Record a generation already applied to this process. Stale or duplicate
    /// observations are harmless and never lower the local checkpoint.
    pub(crate) fn observe_applied(&self, generation: u64) -> u64 {
        let mut current = self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *current {
            Some(existing) if existing >= generation => existing,
            _ => {
                *current = Some(generation);
                generation
            }
        }
    }
}

pub(crate) fn ensure_rbac_invalidation_generation_state(
    ctx: &ServerRuntimeContext,
) -> RbacInvalidationGenerationState {
    let candidate = RbacInvalidationGenerationState::default();
    let _ = ctx.shared_insert_if_absent(candidate.clone());
    ctx.shared_get::<RbacInvalidationGenerationState>()
        .unwrap_or(candidate)
}

/// Reserve the next durable RBAC invalidation generation on the caller-owned
/// connection. When `db` is a transaction, the generation advances atomically
/// with the authorization mutation and is rolled back with it.
pub(crate) async fn reserve_rbac_invalidation_generation<C>(db: &C) -> Result<u64>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let update = db
        .execute(Statement::from_string(
            backend,
            format!(
                "UPDATE rbac_invalidation_state \
                 SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                 WHERE scope = '{RBAC_PERMISSION_SCOPE}' \
                   AND generation < {MAX_DURABLE_GENERATION}"
            ),
        ))
        .await?;

    if update.rows_affected() != 1 {
        return Err(Error::Cache(
            "durable RBAC invalidation generation is missing or exhausted".to_string(),
        ));
    }

    read_rbac_invalidation_generation(db).await
}

pub(crate) async fn read_rbac_invalidation_generation<C>(db: &C) -> Result<u64>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let row = db
        .query_one(Statement::from_string(
            backend,
            format!(
                "SELECT generation FROM rbac_invalidation_state \
                 WHERE scope = '{RBAC_PERMISSION_SCOPE}'"
            ),
        ))
        .await?
        .ok_or_else(|| {
            Error::Cache("durable RBAC invalidation generation row is missing".to_string())
        })?;
    let generation: i64 = row.try_get("", "generation")?;
    u64::try_from(generation).map_err(|_| {
        Error::Cache(format!(
            "durable RBAC invalidation generation is negative: {generation}"
        ))
    })
}

/// Poll the database source of truth so missed Redis/PubSub delivery can never
/// keep a replica on stale authorization snapshots indefinitely.
///
/// The worker is allowed to start before installation migrations complete. It
/// remains dormant while the generation table is absent, then establishes a
/// fail-safe cache baseline as soon as the migration becomes visible.
pub async fn start_rbac_invalidation_generation_watchdog(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    let state = ensure_rbac_invalidation_generation_state(ctx);
    if !ctx.shared_insert_if_absent(RbacInvalidationGenerationWatchdogHandle) {
        return Ok(());
    }

    let db = ctx.db_clone();
    tokio::spawn(async move {
        let mut last_regressed_database_generation: Option<u64> = None;
        let mut interval = tokio::time::interval(RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match read_rbac_invalidation_generation(&db).await {
                Ok(generation) => match state.current() {
                    Some(current) if generation == current => {
                        last_regressed_database_generation = None;
                    }
                    Some(current) if generation < current => {
                        if last_regressed_database_generation != Some(generation) {
                            tracing::error!(
                                previous = current,
                                current = generation,
                                "Durable RBAC invalidation generation regressed; clearing all permission snapshots"
                            );
                            rustok_telemetry::metrics::record_event_error(
                                RBAC_DURABLE_GENERATION_CHANNEL,
                                "generation_regressed",
                            );
                            invalidate_all_user_permissions_cache().await;
                            last_regressed_database_generation = Some(generation);
                        }
                    }
                    previous => {
                        if let Some(previous) = previous {
                            tracing::warn!(
                                previous,
                                current = generation,
                                "Reconciled RBAC permission snapshots from durable database generation"
                            );
                        } else {
                            tracing::info!(
                                generation,
                                "Durable RBAC invalidation generation became available"
                            );
                        }
                        invalidate_all_user_permissions_cache().await;
                        state.observe_applied(generation);
                        last_regressed_database_generation = None;
                    }
                },
                Err(error) if state.current().is_none() && is_missing_generation_state(&error) => {
                    tracing::debug!(
                        "Durable RBAC invalidation state is not installed yet; watchdog will retry"
                    );
                }
                Err(error) => {
                    tracing::error!(
                        %error,
                        "Failed to read durable RBAC invalidation generation"
                    );
                    rustok_telemetry::metrics::record_event_error(
                        RBAC_DURABLE_GENERATION_CHANNEL,
                        "generation_read",
                    );
                }
            }
        }
    });

    Ok(())
}

fn is_missing_generation_state(error: &Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("no such table")
        || message.contains("undefinedtable")
        || message.contains("does not exist") && message.contains("rbac_invalidation_state")
}

#[cfg(test)]
mod tests {
    use super::{
        is_missing_generation_state, read_rbac_invalidation_generation,
        reserve_rbac_invalidation_generation, RbacInvalidationGenerationState,
    };
    use crate::error::Error;
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::TransactionTrait;

    #[test]
    fn applied_generation_state_is_monotonic() {
        let state = RbacInvalidationGenerationState::default();
        assert_eq!(state.current(), None);
        assert_eq!(state.observe_applied(4), 4);
        assert_eq!(state.observe_applied(3), 4);
        assert_eq!(state.observe_applied(5), 5);
        assert_eq!(state.current(), Some(5));
    }

    #[test]
    fn missing_generation_table_errors_are_recognized_for_pre_install_boot() {
        assert!(is_missing_generation_state(&Error::Cache(
            "no such table: rbac_invalidation_state".to_string()
        )));
        assert!(is_missing_generation_state(&Error::Cache(
            "relation rbac_invalidation_state does not exist".to_string()
        )));
        assert!(!is_missing_generation_state(&Error::Cache(
            "connection refused".to_string()
        )));
    }

    #[tokio::test]
    async fn durable_generation_commits_and_rolls_back_with_the_owner_transaction() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let rolled_back = db.begin().await.unwrap();
        assert_eq!(
            reserve_rbac_invalidation_generation(&rolled_back)
                .await
                .unwrap(),
            1
        );
        rolled_back.rollback().await.unwrap();
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let committed = db.begin().await.unwrap();
        assert_eq!(
            reserve_rbac_invalidation_generation(&committed)
                .await
                .unwrap(),
            1
        );
        committed.commit().await.unwrap();
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 1);
    }
}

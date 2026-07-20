use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::FutureExt;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::rbac_runtime::invalidate_all_user_permissions_cache;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub(crate) const RBAC_DURABLE_GENERATION_CHANNEL: &str = "rbac.permissions.durable_generation.v1";
const RBAC_DURABLE_GENERATION_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const RBAC_DURABLE_GENERATION_WATCHDOG_RESTART_DELAY: Duration = Duration::from_secs(1);

struct AbortOnDropWatchdogTask {
    task: JoinHandle<()>,
}

impl Drop for AbortOnDropWatchdogTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct RbacInvalidationGenerationWatchdogHandle(Arc<AbortOnDropWatchdogTask>);

impl RbacInvalidationGenerationWatchdogHandle {
    fn new(task: JoinHandle<()>) -> Self {
        Self(Arc::new(AbortOnDropWatchdogTask { task }))
    }

    pub fn is_running(&self) -> bool {
        !self.0.task.is_finished()
    }

    #[cfg(test)]
    fn abort(&self) {
        self.0.task.abort();
    }
}

#[derive(Clone, Default)]
struct RbacInvalidationGenerationWatchdogStartLock(Arc<tokio::sync::Mutex<()>>);

#[derive(Clone, Default)]
pub(crate) struct RbacInvalidationGenerationState(Arc<RwLock<Option<u64>>>);

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

pub(crate) async fn reserve_rbac_invalidation_generation(db: &DatabaseTransaction) -> Result<u64> {
    rustok_rbac::reserve_permission_invalidation_generation(db)
        .await
        .map_err(|error| Error::Cache(error.to_string()))
}

pub(crate) async fn read_rbac_invalidation_generation<C>(db: &C) -> Result<u64>
where
    C: ConnectionTrait,
{
    rustok_rbac::read_permission_invalidation_generation(db)
        .await
        .map_err(|error| Error::Cache(error.to_string()))
}

async fn run_rbac_invalidation_generation_watchdog(
    db: DatabaseConnection,
    state: RbacInvalidationGenerationState,
) {
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
}

async fn supervise_rbac_invalidation_generation_watchdog<F, Fut>(
    mut worker_factory: F,
    restart_delay: Duration,
) where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    loop {
        let outcome = AssertUnwindSafe(worker_factory()).catch_unwind().await;
        if outcome.is_err() {
            tracing::error!("Durable RBAC invalidation generation watchdog panicked; restarting");
        } else {
            tracing::error!(
                "Durable RBAC invalidation generation watchdog exited unexpectedly; restarting"
            );
        }
        rustok_telemetry::metrics::record_event_error(
            RBAC_DURABLE_GENERATION_CHANNEL,
            "watchdog_restart",
        );
        tokio::time::sleep(restart_delay).await;
    }
}

fn spawn_rbac_invalidation_generation_watchdog(
    db: DatabaseConnection,
    state: RbacInvalidationGenerationState,
) -> JoinHandle<()> {
    tokio::spawn(supervise_rbac_invalidation_generation_watchdog(
        move || run_rbac_invalidation_generation_watchdog(db.clone(), state.clone()),
        RBAC_DURABLE_GENERATION_WATCHDOG_RESTART_DELAY,
    ))
}

/// Poll the database source of truth so missed Redis/PubSub delivery can never
/// keep a replica on stale authorization snapshots indefinitely.
///
/// The worker is allowed to start before installation migrations complete. It
/// remains dormant while the generation table is absent, then establishes a
/// fail-safe cache baseline as soon as the migration becomes visible.
pub async fn start_rbac_invalidation_generation_watchdog(ctx: &ServerRuntimeContext) -> Result<()> {
    let _ = ctx.shared_insert_if_absent(RbacInvalidationGenerationWatchdogStartLock::default());
    let start_lock = ctx
        .shared_get::<RbacInvalidationGenerationWatchdogStartLock>()
        .ok_or_else(|| {
            Error::Cache("RBAC durable generation watchdog start lock is unavailable".to_string())
        })?;
    let _start_guard = start_lock.0.lock().await;

    if let Some(existing) = ctx.shared_get::<RbacInvalidationGenerationWatchdogHandle>() {
        if existing.is_running() {
            return Ok(());
        }
        tracing::warn!("Durable RBAC invalidation generation watchdog stopped; replacing runtime");
    }

    let state = ensure_rbac_invalidation_generation_state(ctx);
    let task = spawn_rbac_invalidation_generation_watchdog(ctx.db_clone(), state);
    ctx.shared_insert(RbacInvalidationGenerationWatchdogHandle::new(task));
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::{
        RbacInvalidationGenerationState, RbacInvalidationGenerationWatchdogHandle,
        is_missing_generation_state, read_rbac_invalidation_generation,
        reserve_rbac_invalidation_generation, supervise_rbac_invalidation_generation_watchdog,
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

    #[tokio::test]
    async fn watchdog_handle_reports_terminal_tasks() {
        let handle = RbacInvalidationGenerationWatchdogHandle::new(tokio::spawn(async {
            std::future::pending::<()>().await;
        }));
        assert!(handle.is_running());
        handle.abort();
        tokio::task::yield_now().await;
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn watchdog_supervisor_restarts_after_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = attempts.clone();
        let supervisor = tokio::spawn(supervise_rbac_invalidation_generation_watchdog(
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        panic!("watchdog regression fixture");
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
        .expect("watchdog supervisor should restart the worker");
        supervisor.abort();
    }
}

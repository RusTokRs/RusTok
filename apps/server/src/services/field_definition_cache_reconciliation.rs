use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flex::cache_generation::FIELD_DEFINITION_CACHE_GENERATION_TABLE;
use futures_util::FutureExt;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement, TryGetable};
use tokio::task::JoinHandle;

use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::server_runtime_context::ServerRuntimeContext;

const FLEX_FIELD_CACHE_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const FLEX_FIELD_CACHE_RESTART_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
struct FieldDefinitionCacheGenerationStartLock(Arc<Mutex<()>>);

#[derive(Default)]
struct FieldDefinitionCacheGenerationState {
    healthy: AtomicBool,
    applied_generation: AtomicU64,
}

struct AbortOnDropFieldDefinitionCacheGenerationTask {
    task: JoinHandle<()>,
}

impl AbortOnDropFieldDefinitionCacheGenerationTask {
    fn new(task: JoinHandle<()>) -> Self {
        Self { task }
    }

    fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    fn abort(&self) {
        self.task.abort();
    }
}

impl Drop for AbortOnDropFieldDefinitionCacheGenerationTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct FieldDefinitionCacheGenerationReconciliationHandle {
    state: Arc<FieldDefinitionCacheGenerationState>,
    task: Arc<AbortOnDropFieldDefinitionCacheGenerationTask>,
}

impl FieldDefinitionCacheGenerationReconciliationHandle {
    fn new(
        state: Arc<FieldDefinitionCacheGenerationState>,
        task: JoinHandle<()>,
    ) -> Self {
        Self {
            state,
            task: Arc::new(AbortOnDropFieldDefinitionCacheGenerationTask::new(task)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.task.is_running() && self.state.healthy.load(Ordering::Acquire)
    }

    pub fn applied_generation(&self) -> u64 {
        self.state.applied_generation.load(Ordering::Acquire)
    }

    fn abort(&self) {
        self.task.abort();
    }
}

pub fn start_field_definition_cache_generation_reconciliation(
    ctx: &ServerRuntimeContext,
    cache: FieldDefinitionCache,
) {
    let _ = ctx.shared_insert_if_absent(FieldDefinitionCacheGenerationStartLock::default());
    let start_lock = ctx
        .shared_get::<FieldDefinitionCacheGenerationStartLock>()
        .expect("field-definition generation start lock must exist after registration");
    let _start_guard = start_lock
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(existing) =
        ctx.shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()
    {
        if existing.is_running() {
            return;
        }
        tracing::warn!(
            applied_generation = existing.applied_generation(),
            "Field-definition cache generation reconciler stopped; replacing supervised runtime"
        );
        existing.abort();
    }

    let state = Arc::new(FieldDefinitionCacheGenerationState::default());
    let db = ctx.db_clone();
    let worker_state = Arc::clone(&state);
    let task = tokio::spawn(async move {
        supervise_field_definition_cache_generation(db, cache, worker_state).await;
    });
    ctx.shared_insert(FieldDefinitionCacheGenerationReconciliationHandle::new(
        state, task,
    ));
}

async fn supervise_field_definition_cache_generation(
    db: DatabaseConnection,
    cache: FieldDefinitionCache,
    state: Arc<FieldDefinitionCacheGenerationState>,
) {
    loop {
        state.healthy.store(false, Ordering::Release);
        let outcome = AssertUnwindSafe(run_field_definition_cache_generation_once(
            db.clone(),
            cache.clone(),
            Arc::clone(&state),
        ))
        .catch_unwind()
        .await;

        match outcome {
            Ok(Ok(())) => tracing::error!(
                "Field-definition cache generation reconciler exited unexpectedly; restarting"
            ),
            Ok(Err(error)) => tracing::error!(
                %error,
                "Field-definition cache generation reconciler failed; restarting"
            ),
            Err(_) => tracing::error!(
                "Field-definition cache generation reconciler panicked; restarting"
            ),
        }
        rustok_telemetry::metrics::record_event_error(
            "flex_field_definition_cache_generation",
            "worker_restart",
        );
        tokio::time::sleep(FLEX_FIELD_CACHE_RESTART_DELAY).await;
    }
}

async fn run_field_definition_cache_generation_once(
    db: DatabaseConnection,
    cache: FieldDefinitionCache,
    state: Arc<FieldDefinitionCacheGenerationState>,
) -> Result<(), String> {
    let mut applied = read_field_definition_cache_generation(&db)
        .await
        .map_err(|error| error.to_string())?;

    // Seed from durable state before trusting any process-local cache contents.
    cache.invalidate_all();
    state.applied_generation.store(applied, Ordering::Release);
    state.healthy.store(true, Ordering::Release);

    loop {
        tokio::time::sleep(FLEX_FIELD_CACHE_RECONCILE_INTERVAL).await;
        let current = match read_field_definition_cache_generation(&db).await {
            Ok(current) => current,
            Err(error) => {
                state.healthy.store(false, Ordering::Release);
                return Err(error.to_string());
            }
        };

        if current < applied {
            state.healthy.store(false, Ordering::Release);
            return Err(format!(
                "field-definition cache generation regressed from {applied} to {current}"
            ));
        }
        if current == applied {
            continue;
        }

        state.healthy.store(false, Ordering::Release);
        cache.invalidate_all();
        tracing::info!(
            previous_generation = applied,
            current_generation = current,
            "Reconciled field-definition cache after durable generation advance"
        );
        applied = current;
        state.applied_generation.store(applied, Ordering::Release);
        state.healthy.store(true, Ordering::Release);
    }
}

async fn read_field_definition_cache_generation(
    db: &DatabaseConnection,
) -> Result<u64, DbErr> {
    let statement = Statement::from_string(
        db.get_database_backend(),
        format!(
            "SELECT generation FROM {FIELD_DEFINITION_CACHE_GENERATION_TABLE} WHERE id = 1"
        ),
    );
    let row = db.query_one(statement).await?.ok_or_else(|| {
        DbErr::RecordNotFound(
            "field-definition cache generation singleton row is missing".to_string(),
        )
    })?;
    let generation: i64 = row.try_get("", "generation")?;
    u64::try_from(generation).map_err(|_| {
        DbErr::Custom(format!(
            "field-definition cache generation must be non-negative, got {generation}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::FieldDefinitionCacheGenerationState;
    use std::sync::atomic::Ordering;

    #[test]
    fn generation_state_starts_fail_closed() {
        let state = FieldDefinitionCacheGenerationState::default();
        assert!(!state.healthy.load(Ordering::Acquire));
        assert_eq!(state.applied_generation.load(Ordering::Acquire), 0);
    }
}

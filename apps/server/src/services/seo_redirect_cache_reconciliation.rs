use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::settings::RuntimeHostMode;
use crate::services::server_runtime_context::ServerRuntimeContext;

const SEO_REDIRECT_CACHE_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const SEO_REDIRECT_CACHE_RESTART_DELAY: Duration = Duration::from_secs(1);
const SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256;
const SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL: usize = 16;

#[derive(Default)]
struct SeoRedirectCacheReconciliationState {
    healthy: AtomicBool,
    observed_count: AtomicU64,
}

trait SeoRedirectCacheInvalidator: Send + Sync {
    fn invalidate_tenant(&self, tenant_id: Uuid) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    fn invalidate_all(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

#[derive(Default)]
struct GlobalSeoRedirectCacheInvalidator;

impl SeoRedirectCacheInvalidator for GlobalSeoRedirectCacheInvalidator {
    fn invalidate_tenant(&self, tenant_id: Uuid) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            rustok_seo::services::invalidate_redirect_cache(tenant_id).await;
        })
    }

    fn invalidate_all(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {
            rustok_seo::services::invalidate_all_redirect_cache().await;
        })
    }
}

struct AbortOnDropSeoRedirectCacheTask {
    task: JoinHandle<()>,
}

impl AbortOnDropSeoRedirectCacheTask {
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

impl Drop for AbortOnDropSeoRedirectCacheTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct SeoRedirectCacheReconciliationHandle {
    state: Arc<SeoRedirectCacheReconciliationState>,
    task: Arc<AbortOnDropSeoRedirectCacheTask>,
}

impl SeoRedirectCacheReconciliationHandle {
    fn new(state: Arc<SeoRedirectCacheReconciliationState>, task: JoinHandle<()>) -> Self {
        Self {
            state,
            task: Arc::new(AbortOnDropSeoRedirectCacheTask::new(task)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.task.is_running()
    }

    pub fn is_ready(&self) -> bool {
        self.is_running() && self.state.healthy.load(Ordering::Acquire)
    }

    pub fn observed_count(&self) -> u64 {
        self.state.observed_count.load(Ordering::Acquire)
    }

    fn abort(&self) {
        self.task.abort();
        self.state.healthy.store(false, Ordering::Release);
    }
}

#[derive(Clone, Default)]
struct SeoRedirectCacheReconciliationStartLock(Arc<Mutex<()>>);

pub fn seo_redirect_cache_reconciliation_required(ctx: &ServerRuntimeContext) -> bool {
    !matches!(
        ctx.settings().runtime.host_mode,
        RuntimeHostMode::RegistryOnly | RuntimeHostMode::Worker
    )
}

/// Ensure every serving runtime reconciles its process-local redirect cache from the
/// transactionally persisted SEO delivery log.
pub fn start_seo_redirect_cache_reconciliation(ctx: &ServerRuntimeContext) {
    start_seo_redirect_cache_reconciliation_with_options(
        ctx,
        Arc::new(GlobalSeoRedirectCacheInvalidator),
        SEO_REDIRECT_CACHE_RECONCILE_INTERVAL,
        SEO_REDIRECT_CACHE_RESTART_DELAY,
        SEO_REDIRECT_CACHE_BATCH_LIMIT,
        SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL,
    );
}

fn start_seo_redirect_cache_reconciliation_with_options(
    ctx: &ServerRuntimeContext,
    invalidator: Arc<dyn SeoRedirectCacheInvalidator>,
    reconcile_interval: Duration,
    restart_delay: Duration,
    batch_limit: u64,
    max_pages_per_poll: usize,
) {
    if !seo_redirect_cache_reconciliation_required(ctx) {
        return;
    }

    let _ = ctx.shared_insert_if_absent(SeoRedirectCacheReconciliationStartLock::default());
    let start_lock = ctx
        .shared_get::<SeoRedirectCacheReconciliationStartLock>()
        .expect("SEO redirect cache reconciliation start lock must exist after registration");
    let _start_guard = start_lock
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(existing) = ctx.shared_get::<SeoRedirectCacheReconciliationHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!(
            observed_count = existing.observed_count(),
            "SEO redirect cache reconciliation stopped; replacing runtime"
        );
        existing.abort();
    }

    let state = Arc::new(SeoRedirectCacheReconciliationState::default());
    let worker_state = Arc::clone(&state);
    let task = tokio::spawn(supervise_seo_redirect_cache_reconciliation(
        ctx.db_clone(),
        invalidator,
        worker_state,
        reconcile_interval,
        restart_delay,
        batch_limit,
        max_pages_per_poll,
    ));
    ctx.shared_insert(SeoRedirectCacheReconciliationHandle::new(state, task));
}

async fn supervise_seo_redirect_cache_reconciliation(
    db: DatabaseConnection,
    invalidator: Arc<dyn SeoRedirectCacheInvalidator>,
    state: Arc<SeoRedirectCacheReconciliationState>,
    reconcile_interval: Duration,
    restart_delay: Duration,
    batch_limit: u64,
    max_pages_per_poll: usize,
) {
    loop {
        state.healthy.store(false, Ordering::Release);
        let outcome = AssertUnwindSafe(run_seo_redirect_cache_reconciliation(
            db.clone(),
            Arc::clone(&invalidator),
            Arc::clone(&state),
            reconcile_interval,
            batch_limit,
            max_pages_per_poll,
        ))
        .catch_unwind()
        .await;
        state.healthy.store(false, Ordering::Release);
        match outcome {
            Ok(Ok(())) => {
                tracing::error!("SEO redirect cache reconciliation exited unexpectedly")
            }
            Ok(Err(error)) => tracing::error!(
                %error,
                "SEO redirect cache reconciliation failed; restarting"
            ),
            Err(_) => tracing::error!("SEO redirect cache reconciliation panicked; restarting"),
        }
        rustok_telemetry::metrics::record_event_error(
            "seo.redirect.cache",
            "reconciliation_restart",
        );
        tokio::time::sleep(restart_delay).await;
    }
}

async fn seed_redirect_cache_state(
    db: &DatabaseConnection,
    invalidator: &dyn SeoRedirectCacheInvalidator,
) -> rustok_seo::SeoResult<(Option<rustok_seo::services::SeoRedirectCacheCursor>, u64)> {
    // Read count first, then the high-water cursor, then clear. Any commit racing between these
    // reads either appears after the cursor and is consumed normally or changes the next count
    // delta and triggers another safe full-clear recovery.
    let observed_count = rustok_seo::services::redirect_cache_change_count(db).await?;
    let cursor = rustok_seo::services::latest_redirect_cache_cursor(db).await?;
    invalidator.invalidate_all().await;
    Ok((cursor, observed_count))
}

async fn run_seo_redirect_cache_reconciliation(
    db: DatabaseConnection,
    invalidator: Arc<dyn SeoRedirectCacheInvalidator>,
    state: Arc<SeoRedirectCacheReconciliationState>,
    reconcile_interval: Duration,
    batch_limit: u64,
    max_pages_per_poll: usize,
) -> rustok_seo::SeoResult<()> {
    let (mut cursor, mut observed_count) =
        match seed_redirect_cache_state(&db, invalidator.as_ref()).await {
            Ok(seed) => seed,
            Err(error) => {
                invalidator.invalidate_all().await;
                return Err(error);
            }
        };
    state
        .observed_count
        .store(observed_count, Ordering::Release);
    state.healthy.store(true, Ordering::Release);

    loop {
        tokio::time::sleep(reconcile_interval).await;
        let poll_result = poll_redirect_cache_changes(
            &db,
            invalidator.as_ref(),
            &mut cursor,
            &mut observed_count,
            &state,
            batch_limit,
            max_pages_per_poll,
        )
        .await;
        if let Err(error) = poll_result {
            state.healthy.store(false, Ordering::Release);
            invalidator.invalidate_all().await;
            return Err(error);
        }
    }
}

async fn poll_redirect_cache_changes(
    db: &DatabaseConnection,
    invalidator: &dyn SeoRedirectCacheInvalidator,
    cursor: &mut Option<rustok_seo::services::SeoRedirectCacheCursor>,
    observed_count: &mut u64,
    state: &SeoRedirectCacheReconciliationState,
    batch_limit: u64,
    max_pages_per_poll: usize,
) -> rustok_seo::SeoResult<()> {
    let current_count = rustok_seo::services::redirect_cache_change_count(db).await?;
    let mut processed = 0_u64;

    for _ in 0..max_pages_per_poll {
        let changes =
            rustok_seo::services::redirect_cache_changes_after(db, cursor.as_ref(), batch_limit)
                .await?;
        let page_len = changes.len() as u64;

        for change in changes {
            invalidator.invalidate_tenant(change.tenant_id).await;
            *cursor = Some(change.cursor);
        }
        processed = processed.saturating_add(page_len);

        if page_len < batch_limit {
            break;
        }
        tokio::task::yield_now().await;
    }

    let expected_count = observed_count.saturating_add(processed);
    if current_count != expected_count {
        tracing::warn!(
            observed_count = *observed_count,
            current_count,
            processed,
            "SEO redirect cursor/count gap detected; clearing and reseeding cache"
        );
        rustok_telemetry::metrics::record_event_error("seo.redirect.cache", "cursor_gap_recovery");
        state.healthy.store(false, Ordering::Release);
        (*cursor, *observed_count) = seed_redirect_cache_state(db, invalidator).await?;
        state
            .observed_count
            .store(*observed_count, Ordering::Release);
        state.healthy.store(true, Ordering::Release);
        return Ok(());
    }

    *observed_count = current_count;
    state.observed_count.store(current_count, Ordering::Release);
    Ok(())
}

#[cfg(test)]
#[path = "seo_redirect_cache_reconciliation_tests.rs"]
mod tests;

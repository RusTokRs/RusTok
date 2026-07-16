use std::panic::AssertUnwindSafe;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use futures_util::FutureExt;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::common::settings::RuntimeHostMode;
use crate::services::server_runtime_context::ServerRuntimeContext;

const SEO_REDIRECT_CACHE_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const SEO_REDIRECT_CACHE_RESTART_DELAY: Duration = Duration::from_secs(1);
const SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256;
const SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL: usize = 16;

struct AbortOnDropSeoRedirectCacheTask {
    task: JoinHandle<()>,
    healthy: Arc<AtomicBool>,
}

impl AbortOnDropSeoRedirectCacheTask {
    fn new(task: JoinHandle<()>, healthy: Arc<AtomicBool>) -> Self {
        Self { task, healthy }
    }

    fn is_running(&self) -> bool {
        !self.task.is_finished() && self.healthy.load(Ordering::Acquire)
    }

    fn abort(&self) {
        self.healthy.store(false, Ordering::Release);
        self.task.abort();
    }
}

impl Drop for AbortOnDropSeoRedirectCacheTask {
    fn drop(&mut self) {
        self.healthy.store(false, Ordering::Release);
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct SeoRedirectCacheReconciliationHandle(Arc<AbortOnDropSeoRedirectCacheTask>);

impl SeoRedirectCacheReconciliationHandle {
    fn new(task: JoinHandle<()>, healthy: Arc<AtomicBool>) -> Self {
        Self(Arc::new(AbortOnDropSeoRedirectCacheTask::new(
            task, healthy,
        )))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    fn abort(&self) {
        self.0.abort();
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
        tracing::warn!("SEO redirect cache reconciliation stopped; replacing runtime");
        existing.abort();
    }

    let healthy = Arc::new(AtomicBool::new(false));
    let task = tokio::spawn(supervise_seo_redirect_cache_reconciliation(
        ctx.db_clone(),
        Arc::clone(&healthy),
    ));
    ctx.shared_insert(SeoRedirectCacheReconciliationHandle::new(task, healthy));
}

async fn supervise_seo_redirect_cache_reconciliation(
    db: DatabaseConnection,
    healthy: Arc<AtomicBool>,
) {
    loop {
        healthy.store(false, Ordering::Release);
        let outcome = AssertUnwindSafe(run_seo_redirect_cache_reconciliation(
            db.clone(),
            Arc::clone(&healthy),
        ))
        .catch_unwind()
        .await;
        healthy.store(false, Ordering::Release);
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
        tokio::time::sleep(SEO_REDIRECT_CACHE_RESTART_DELAY).await;
    }
}

async fn seed_redirect_cache_state(
    db: &DatabaseConnection,
) -> rustok_seo::SeoResult<(Option<rustok_seo::services::SeoRedirectCacheCursor>, u64)> {
    // Read count first, then the high-water cursor, then clear. Any commit racing between these
    // reads either appears after the cursor and is consumed normally or changes the next count
    // delta and triggers another safe full-clear recovery.
    let observed_count = rustok_seo::services::redirect_cache_change_count(db).await?;
    let cursor = rustok_seo::services::latest_redirect_cache_cursor(db).await?;
    rustok_seo::services::invalidate_all_redirect_cache().await;
    Ok((cursor, observed_count))
}

async fn run_seo_redirect_cache_reconciliation(
    db: DatabaseConnection,
    healthy: Arc<AtomicBool>,
) -> rustok_seo::SeoResult<()> {
    let (mut cursor, mut observed_count) = seed_redirect_cache_state(&db).await?;
    healthy.store(true, Ordering::Release);

    loop {
        let current_count = rustok_seo::services::redirect_cache_change_count(&db).await?;
        let mut processed = 0_u64;

        for _ in 0..SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL {
            let changes = rustok_seo::services::redirect_cache_changes_after(
                &db,
                cursor.as_ref(),
                SEO_REDIRECT_CACHE_BATCH_LIMIT,
            )
            .await?;
            let page_len = changes.len() as u64;

            for change in changes {
                rustok_seo::services::invalidate_redirect_cache(change.tenant_id).await;
                cursor = Some(change.cursor);
            }
            processed = processed.saturating_add(page_len);

            if page_len < SEO_REDIRECT_CACHE_BATCH_LIMIT {
                break;
            }
            tokio::task::yield_now().await;
        }

        let expected_count = observed_count.saturating_add(processed);
        if current_count != expected_count {
            tracing::warn!(
                observed_count,
                current_count,
                processed,
                "SEO redirect cursor/count gap detected; clearing and reseeding cache"
            );
            rustok_telemetry::metrics::record_event_error(
                "seo.redirect.cache",
                "cursor_gap_recovery",
            );
            healthy.store(false, Ordering::Release);
            (cursor, observed_count) = seed_redirect_cache_state(&db).await?;
            healthy.store(true, Ordering::Release);
        } else {
            observed_count = current_count;
        }

        tokio::time::sleep(SEO_REDIRECT_CACHE_RECONCILE_INTERVAL).await;
    }
}

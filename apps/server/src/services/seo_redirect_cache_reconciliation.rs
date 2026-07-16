use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::services::server_runtime_context::ServerRuntimeContext;

const SEO_REDIRECT_CACHE_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const SEO_REDIRECT_CACHE_RESTART_DELAY: Duration = Duration::from_secs(1);
const SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256;

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
pub struct SeoRedirectCacheReconciliationHandle(Arc<AbortOnDropSeoRedirectCacheTask>);

impl SeoRedirectCacheReconciliationHandle {
    fn new(task: JoinHandle<()>) -> Self {
        Self(Arc::new(AbortOnDropSeoRedirectCacheTask::new(task)))
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

/// Ensure every serving runtime reconciles its process-local redirect cache from the
/// transactionally persisted SEO delivery log.
pub fn start_seo_redirect_cache_reconciliation(ctx: &ServerRuntimeContext) {
    if ctx.settings().runtime.is_registry_only() {
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

    let task = tokio::spawn(supervise_seo_redirect_cache_reconciliation(ctx.db_clone()));
    ctx.shared_insert(SeoRedirectCacheReconciliationHandle::new(task));
}

async fn supervise_seo_redirect_cache_reconciliation(db: DatabaseConnection) {
    loop {
        let outcome = AssertUnwindSafe(run_seo_redirect_cache_reconciliation(db.clone()))
            .catch_unwind()
            .await;
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

async fn run_seo_redirect_cache_reconciliation(
    db: DatabaseConnection,
) -> rustok_seo::SeoResult<()> {
    // Read the high-water mark before clearing. Transactions at or before this cursor are covered
    // by the full clear; transactions committed after it are consumed below. This ordering avoids
    // the startup race where a post-clear commit could otherwise be incorrectly treated as seeded.
    let mut cursor = rustok_seo::services::latest_redirect_cache_cursor(&db).await?;
    rustok_seo::services::invalidate_all_redirect_cache().await;

    loop {
        let changes = rustok_seo::services::redirect_cache_changes_after(
            &db,
            cursor.as_ref(),
            SEO_REDIRECT_CACHE_BATCH_LIMIT,
        )
        .await?;
        let full_batch = changes.len() as u64 == SEO_REDIRECT_CACHE_BATCH_LIMIT;

        for change in changes {
            rustok_seo::services::invalidate_redirect_cache(change.tenant_id).await;
            cursor = Some(change.cursor);
        }

        if full_batch {
            tokio::task::yield_now().await;
            continue;
        }

        tokio::time::sleep(SEO_REDIRECT_CACHE_RECONCILE_INTERVAL).await;
    }
}

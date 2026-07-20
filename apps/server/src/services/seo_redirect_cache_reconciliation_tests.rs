use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

use super::*;
use crate::common::settings::RustokSettings;

#[derive(Default)]
struct RecordingInvalidator {
    full_clears: AtomicU64,
    tenant_invalidations: Mutex<Vec<Uuid>>,
}

impl RecordingInvalidator {
    fn full_clear_count(&self) -> u64 {
        self.full_clears.load(Ordering::Acquire)
    }

    fn tenant_ids(&self) -> Vec<Uuid> {
        self.tenant_invalidations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl SeoRedirectCacheInvalidator for RecordingInvalidator {
    fn invalidate_tenant(&self, tenant_id: Uuid) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.tenant_invalidations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(tenant_id);
        })
    }

    fn invalidate_all(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {
            self.full_clears.fetch_add(1, Ordering::AcqRel);
        })
    }
}

async fn seo_delivery_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite SEO delivery fixture should connect");
    db.execute_unprepared(
        "CREATE TABLE seo_event_deliveries (\
           id TEXT PRIMARY KEY NOT NULL, \
           tenant_id TEXT NOT NULL, \
           event_type TEXT NOT NULL, \
           idempotency_key TEXT NOT NULL, \
           source_kind TEXT NULL, \
           source_id TEXT NULL, \
           status TEXT NOT NULL, \
           outbox_event_id TEXT NULL, \
           last_error TEXT NULL, \
           created_at TEXT NOT NULL, \
           updated_at TEXT NOT NULL, \
           dispatched_at TEXT NULL\
         )",
    )
    .await
    .expect("SEO delivery table should create");
    db
}

async fn insert_redirect_change(db: &DatabaseConnection, tenant_id: Uuid, sequence: i64) {
    let timestamp = (chrono::Utc::now() + chrono::TimeDelta::milliseconds(sequence)).to_rfc3339();
    let delivery_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO seo_event_deliveries (\
           id, tenant_id, event_type, idempotency_key, source_kind, source_id, status, \
           outbox_event_id, last_error, created_at, updated_at, dispatched_at\
         ) VALUES (\
           '{delivery_id}', '{tenant_id}', 'seo.redirect.changed', 'redirect:{delivery_id}', \
           'redirect', NULL, 'pending', NULL, NULL, '{timestamp}', '{timestamp}', NULL\
         )"
    ))
    .await
    .expect("SEO redirect delivery should insert");
}

async fn wait_for_replica(
    handle: &SeoRedirectCacheReconciliationHandle,
    expected_ready: bool,
    expected_count: Option<u64>,
) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let count_matches = expected_count
                .is_none_or(|count| handle.observed_count() == count);
            if handle.is_ready() == expected_ready && count_matches {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "SEO replica did not become ready={expected_ready}, count={expected_count:?}; actual ready={}, count={}",
            handle.is_ready(),
            handle.observed_count()
        )
    });
}

async fn wait_for_stopped(handle: &SeoRedirectCacheReconciliationHandle) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while handle.is_running() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("SEO reconciliation task did not stop after abort");
}

async fn wait_for_tenants(invalidator: &RecordingInvalidator, expected: &HashSet<Uuid>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let observed = invalidator.tenant_ids().into_iter().collect::<HashSet<_>>();
            if expected.is_subset(&observed) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("SEO replica did not invalidate every expected tenant");
}

async fn wait_for_full_clears(invalidator: &RecordingInvalidator, expected_minimum: u64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while invalidator.full_clear_count() < expected_minimum {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "SEO replica did not reach {expected_minimum} full clears; actual {}",
            invalidator.full_clear_count()
        )
    });
}

#[tokio::test]
#[serial_test::serial]
async fn seo_redirect_cache_reconciliation_recovers_two_replicas_across_cursor_faults() {
    let db = seo_delivery_db().await;
    let startup_a = Uuid::new_v4();
    let startup_b = Uuid::new_v4();
    insert_redirect_change(&db, startup_a, 10).await;
    insert_redirect_change(&db, startup_b, 20).await;

    let ctx_a = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    assert!(seo_redirect_cache_reconciliation_required(&ctx_a));
    assert!(seo_redirect_cache_reconciliation_required(&ctx_b));

    let invalidator_a = Arc::new(RecordingInvalidator::default());
    let invalidator_b = Arc::new(RecordingInvalidator::default());
    start_seo_redirect_cache_reconciliation_with_options(
        &ctx_a,
        invalidator_a.clone(),
        Duration::from_millis(20),
        Duration::from_millis(20),
        3,
        16,
    );
    start_seo_redirect_cache_reconciliation_with_options(
        &ctx_b,
        invalidator_b.clone(),
        Duration::from_millis(20),
        Duration::from_millis(20),
        3,
        16,
    );

    let handle_a = ctx_a
        .shared_get::<SeoRedirectCacheReconciliationHandle>()
        .expect("first SEO reconciliation handle");
    let handle_b = ctx_b
        .shared_get::<SeoRedirectCacheReconciliationHandle>()
        .expect("second SEO reconciliation handle");
    wait_for_replica(&handle_a, true, Some(2)).await;
    wait_for_replica(&handle_b, true, Some(2)).await;
    wait_for_full_clears(&invalidator_a, 1).await;
    wait_for_full_clears(&invalidator_b, 1).await;

    let exact_tenant = Uuid::new_v4();
    insert_redirect_change(&db, exact_tenant, 30).await;
    wait_for_replica(&handle_a, true, Some(3)).await;
    wait_for_replica(&handle_b, true, Some(3)).await;
    let exact = HashSet::from([exact_tenant]);
    wait_for_tenants(&invalidator_a, &exact).await;
    wait_for_tenants(&invalidator_b, &exact).await;

    let mut paged_tenants = HashSet::new();
    for sequence in 40..50 {
        let tenant_id = Uuid::new_v4();
        paged_tenants.insert(tenant_id);
        insert_redirect_change(&db, tenant_id, sequence).await;
    }
    wait_for_replica(&handle_a, true, Some(13)).await;
    wait_for_replica(&handle_b, true, Some(13)).await;
    wait_for_tenants(&invalidator_a, &paged_tenants).await;
    wait_for_tenants(&invalidator_b, &paged_tenants).await;

    // This row sorts behind the applied cursor. The count advances while the
    // cursor page is empty, forcing namespace-wide gap recovery on both replicas.
    insert_redirect_change(&db, Uuid::new_v4(), -10_000).await;
    wait_for_replica(&handle_a, true, Some(14)).await;
    wait_for_replica(&handle_b, true, Some(14)).await;
    wait_for_full_clears(&invalidator_a, 2).await;
    wait_for_full_clears(&invalidator_b, 2).await;

    db.execute_unprepared(
        "ALTER TABLE seo_event_deliveries RENAME TO seo_event_deliveries_unavailable",
    )
    .await
    .expect("SEO delivery table should become unavailable");
    wait_for_replica(&handle_a, false, Some(14)).await;
    wait_for_replica(&handle_b, false, Some(14)).await;
    wait_for_full_clears(&invalidator_a, 3).await;
    wait_for_full_clears(&invalidator_b, 3).await;

    db.execute_unprepared(
        "ALTER TABLE seo_event_deliveries_unavailable RENAME TO seo_event_deliveries",
    )
    .await
    .expect("SEO delivery table should recover");
    wait_for_replica(&handle_a, true, Some(14)).await;
    wait_for_replica(&handle_b, true, Some(14)).await;
    wait_for_full_clears(&invalidator_a, 4).await;
    wait_for_full_clears(&invalidator_b, 4).await;

    handle_a.abort();
    wait_for_stopped(&handle_a).await;
    assert!(!handle_a.is_running());
    assert!(!handle_a.is_ready());
    assert!(handle_b.is_running());
    assert!(handle_b.is_ready());
    handle_b.abort();
    wait_for_stopped(&handle_b).await;
}

#[test]
fn seo_redirect_cache_reconciles_from_transactional_delivery_rows() {
    let redirects = include_str!("../../../crates/rustok-seo/src/services/redirects.rs");
    let services = include_str!("../../../crates/rustok-seo/src/services/mod.rs");
    let cursor_migration = include_str!(
        "../../../crates/rustok-seo/src/migrations/m20260716_000007_add_redirect_cache_cursor_index.rs"
    );
    let migration_registry = include_str!("../../../crates/rustok-seo/src/migrations/mod.rs");
    let worker = include_str!("../src/services/seo_redirect_cache_reconciliation.rs");
    let evidence = include_str!("../src/services/seo_redirect_cache_reconciliation_tests.rs");
    let workflow = include_str!("../../../.github/workflows/cache-hardening.yml");
    let schema = include_str!("../src/services/graphql_schema.rs");
    let guardrails = include_str!("../src/services/runtime_guardrails.rs");

    let event_publish = redirects
        .find("publish_redirect_transition_in_tx(&txn")
        .expect("redirect transition must be published in the database transaction");
    let commit = redirects
        .find("txn.commit().await?")
        .expect("redirect mutation must commit after the event row is written");
    let local_invalidate = redirects
        .find("REDIRECT_CACHE.invalidate(&tenant.id).await")
        .expect("mutating replica must invalidate after commit");
    assert!(event_publish < commit);
    assert!(commit < local_invalidate);
    assert!(redirects.contains("source_kind: Set(Some(\"redirect\".to_string()))"));

    assert!(services.contains("include!(\"services_base.rs\")"));
    assert!(services.contains("pub struct SeoRedirectCacheCursor"));
    assert!(services.contains("pub async fn redirect_cache_change_count"));
    assert!(services.contains(".count(db)"));
    assert!(services.contains("Column::CreatedAt.gt(created_at.clone())"));
    assert!(services.contains("Column::CreatedAt.eq(created_at)"));
    assert!(services.contains("Column::Id.gt(cursor.id)"));
    assert!(services.contains(".limit(limit.clamp(1, 1_000))"));
    assert!(services.contains("pub async fn invalidate_all_redirect_cache"));

    assert!(cursor_migration.contains("idx_seo_event_deliveries_redirect_cursor"));
    let source_kind = cursor_migration
        .find(".col(SeoEventDeliveries::SourceKind)")
        .expect("cursor index must start with source_kind");
    let created_at = cursor_migration
        .find(".col(SeoEventDeliveries::CreatedAt)")
        .expect("cursor index must include created_at");
    let id = cursor_migration
        .find(".col(SeoEventDeliveries::Id)")
        .expect("cursor index must end with UUID tie-breaker");
    assert!(source_kind < created_at);
    assert!(created_at < id);
    assert!(
        migration_registry
            .contains("Box::new(m20260716_000007_add_redirect_cache_cursor_index::Migration)")
    );

    for required in [
        "trait SeoRedirectCacheInvalidator: Send + Sync",
        "struct GlobalSeoRedirectCacheInvalidator",
        "rustok_seo::services::invalidate_redirect_cache(tenant_id).await",
        "rustok_seo::services::invalidate_all_redirect_cache().await",
        "struct SeoRedirectCacheReconciliationState",
        "healthy: AtomicBool",
        "observed_count: AtomicU64",
        "pub fn is_running(&self) -> bool",
        "pub fn is_ready(&self) -> bool",
        "self.is_running() && self.state.healthy.load(Ordering::Acquire)",
        "pub fn observed_count(&self) -> u64",
        "start_seo_redirect_cache_reconciliation_with_options",
        "SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256",
        "SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL: usize = 16",
        "for _ in 0..max_pages_per_poll",
        "if page_len < batch_limit",
        "let expected_count = observed_count.saturating_add(processed)",
        "if current_count != expected_count",
        "cursor_gap_recovery",
        "state.healthy.store(false, Ordering::Release)",
        "invalidator.invalidate_all().await",
        "impl Drop for AbortOnDropSeoRedirectCacheTask",
        "SeoRedirectCacheReconciliationStartLock",
        "pub fn seo_redirect_cache_reconciliation_required",
        "RuntimeHostMode::RegistryOnly | RuntimeHostMode::Worker",
        "#[path = \"seo_redirect_cache_reconciliation_tests.rs\"]",
    ] {
        assert!(
            worker.contains(required),
            "SEO redirect reconciliation must retain {required}"
        );
    }

    let seed = worker
        .find("async fn seed_redirect_cache_state")
        .expect("worker must have startup seed logic");
    let count = worker[seed..]
        .find("redirect_cache_change_count(db).await?")
        .map(|offset| seed + offset)
        .expect("startup must read the independent delivery-row count");
    let latest = worker[seed..]
        .find("latest_redirect_cache_cursor(db).await?")
        .map(|offset| seed + offset)
        .expect("startup must read the durable high-water mark");
    let clear = worker[seed..]
        .find("invalidator.invalidate_all().await;")
        .map(|offset| seed + offset)
        .expect("startup must clear entries covered by the seed cursor");
    let run = worker
        .find("async fn run_seo_redirect_cache_reconciliation")
        .expect("worker must run a supervised reconciliation loop");
    let healthy = worker[run..]
        .find("state.healthy.store(true, Ordering::Release)")
        .map(|offset| run + offset)
        .expect("readiness must become healthy only after startup recovery");
    assert!(count < latest);
    assert!(latest < clear);
    assert!(clear < healthy);

    let poll_error = worker
        .find("if let Err(error) = poll_result")
        .expect("poll errors must fail closed");
    let poll_unhealthy = worker[poll_error..]
        .find("state.healthy.store(false, Ordering::Release)")
        .map(|offset| poll_error + offset)
        .expect("poll errors must clear readiness");
    let poll_clear = worker[poll_unhealthy..]
        .find("invalidator.invalidate_all().await;")
        .map(|offset| poll_unhealthy + offset)
        .expect("poll errors must clear cached redirects");
    assert!(poll_unhealthy < poll_clear);

    for required in [
        "seo_redirect_cache_reconciliation_recovers_two_replicas_across_cursor_faults",
        "RecordingInvalidator",
        "start_seo_redirect_cache_reconciliation_with_options",
        "Duration::from_millis(20)",
        "batch_limit",
        "insert_redirect_change(&db, exact_tenant, 30).await;",
        "for sequence in 40..50",
        "insert_redirect_change(&db, Uuid::new_v4(), -10_000).await;",
        "ALTER TABLE seo_event_deliveries RENAME TO seo_event_deliveries_unavailable",
        "wait_for_replica(&handle_a, false, Some(14)).await;",
        "wait_for_replica(&handle_b, true, Some(14)).await;",
        "assert!(!handle_a.is_running());",
        "assert!(handle_b.is_ready());",
    ] {
        assert!(
            evidence.contains(required),
            "SEO two-replica evidence must retain {required}"
        );
    }

    for required in [
        "apps/server/src/services/seo_redirect_cache_reconciliation*.rs",
        "cargo test -p rustok-server seo_redirect_cache_reconciliation --lib",
    ] {
        assert!(
            workflow.contains(required),
            "cache workflow must retain SEO evidence command: {required}"
        );
    }

    let start = schema
        .find("start_seo_redirect_cache_reconciliation(ctx)")
        .expect("GraphQL composition must ensure SEO reconciliation");
    let early_return = schema
        .find("if let Some(shared) = ctx.shared_get::<SharedGraphqlSchema>()")
        .expect("schema reuse path must remain present");
    assert!(start < early_return);

    assert!(guardrails.contains("seo_redirect_cache_reconciliation_required"));
    assert!(guardrails.contains("if seo_redirect_cache_reconciliation_required(ctx)"));
    assert!(guardrails.contains("SeoRedirectCacheReconciliationHandle"));
    assert!(guardrails.contains("SEO redirect durable cache reconciliation"));
    assert!(guardrails.contains(".map(|handle| handle.is_ready())"));
    assert!(guardrails.contains("RuntimeGuardrailStatus::Critical"));
}

#[test]
fn seo_redirect_cache_reconciles_from_transactional_delivery_rows() {
    let redirects = include_str!("../../../crates/rustok-seo/src/services/redirects.rs");
    let services = include_str!("../../../crates/rustok-seo/src/services/mod.rs");
    let cursor_migration = include_str!(
        "../../../crates/rustok-seo/src/migrations/m20260716_000007_add_redirect_cache_cursor_index.rs"
    );
    let migration_registry =
        include_str!("../../../crates/rustok-seo/src/migrations/mod.rs");
    let worker = include_str!("../src/services/seo_redirect_cache_reconciliation.rs");
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
    assert!(migration_registry.contains(
        "Box::new(m20260716_000007_add_redirect_cache_cursor_index::Migration)"
    ));

    let count = worker
        .find("redirect_cache_change_count(db).await?")
        .expect("startup must seed the independent delivery-row count");
    let latest = worker
        .find("latest_redirect_cache_cursor(db).await?")
        .expect("startup must read the durable high-water mark");
    let clear = worker
        .find("invalidate_all_redirect_cache().await")
        .expect("startup must clear entries covered by the seed cursor");
    let healthy = worker
        .find("healthy.store(true, Ordering::Release)")
        .expect("readiness must become healthy only after startup recovery");
    let changes = worker
        .find("redirect_cache_changes_after(")
        .expect("worker must consume rows after the seed cursor");
    assert!(count < latest);
    assert!(latest < clear);
    assert!(clear < healthy);
    assert!(healthy < changes);

    assert!(worker.contains("SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256"));
    assert!(worker.contains("SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL: usize = 16"));
    assert!(worker.contains("for _ in 0..SEO_REDIRECT_CACHE_MAX_PAGES_PER_POLL"));
    assert!(worker.contains("if page_len < SEO_REDIRECT_CACHE_BATCH_LIMIT"));
    assert!(worker.contains("let expected_count = observed_count.saturating_add(processed)"));
    assert!(worker.contains("if current_count != expected_count"));
    assert!(worker.contains("cursor_gap_recovery"));
    assert!(worker.contains("(cursor, observed_count) = seed_redirect_cache_state(&db).await?"));
    assert!(worker.contains("healthy: Arc<AtomicBool>"));
    assert!(worker.contains(
        "!self.task.is_finished() && self.healthy.load(Ordering::Acquire)"
    ));
    assert!(worker.contains("healthy.store(false, Ordering::Release)"));
    assert!(worker.contains("impl Drop for AbortOnDropSeoRedirectCacheTask"));
    assert!(worker.contains("SeoRedirectCacheReconciliationStartLock"));
    assert!(worker.contains("pub fn seo_redirect_cache_reconciliation_required"));
    assert!(worker.contains(
        "RuntimeHostMode::RegistryOnly | RuntimeHostMode::Worker"
    ));
    assert!(worker.contains("pub fn is_running(&self) -> bool"));

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
    assert!(guardrails.contains("RuntimeGuardrailStatus::Critical"));
}

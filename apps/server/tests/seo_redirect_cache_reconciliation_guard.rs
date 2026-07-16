#[test]
fn seo_redirect_cache_reconciles_from_transactional_delivery_rows() {
    let redirects = include_str!("../../../crates/rustok-seo/src/services/redirects.rs");
    let services = include_str!("../../../crates/rustok-seo/src/services/mod.rs");
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
    assert!(services.contains("Column::CreatedAt.gt(created_at.clone())"));
    assert!(services.contains("Column::CreatedAt.eq(created_at)"));
    assert!(services.contains("Column::Id.gt(cursor.id)"));
    assert!(services.contains(".limit(limit.clamp(1, 1_000))"));
    assert!(services.contains("pub async fn invalidate_all_redirect_cache"));

    let latest = worker
        .find("latest_redirect_cache_cursor(&db).await?")
        .expect("startup must read the durable high-water mark");
    let clear = worker
        .find("invalidate_all_redirect_cache().await")
        .expect("startup must clear entries covered by the seed cursor");
    let changes = worker
        .find("redirect_cache_changes_after(")
        .expect("worker must consume rows after the seed cursor");
    assert!(latest < clear);
    assert!(clear < changes);

    assert!(worker.contains("SEO_REDIRECT_CACHE_BATCH_LIMIT: u64 = 256"));
    assert!(worker.contains("if full_batch"));
    assert!(worker.contains("impl Drop for AbortOnDropSeoRedirectCacheTask"));
    assert!(worker.contains("SeoRedirectCacheReconciliationStartLock"));
    assert!(worker.contains("ctx.settings().runtime.is_registry_only()"));
    assert!(worker.contains("pub fn is_running(&self) -> bool"));

    let start = schema
        .find("start_seo_redirect_cache_reconciliation(ctx)")
        .expect("GraphQL composition must ensure SEO reconciliation");
    let early_return = schema
        .find("if let Some(shared) = ctx.shared_get::<SharedGraphqlSchema>()")
        .expect("schema reuse path must remain present");
    assert!(start < early_return);

    assert!(guardrails.contains("SeoRedirectCacheReconciliationHandle"));
    assert!(guardrails.contains("SEO redirect durable cache reconciliation"));
    assert!(guardrails.contains("RuntimeGuardrailStatus::Critical"));
}

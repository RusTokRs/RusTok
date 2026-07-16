#[test]
fn tenant_locale_cache_uses_the_durable_tenant_generation_channel() {
    let locale = include_str!("../src/middleware/locale.rs");
    let listener = include_str!("../src/services/tenant_locale_generation.rs");
    let middleware = include_str!("../src/middleware/mod.rs");
    let guardrails = include_str!("../src/services/runtime_guardrails.rs");

    assert!(locale.contains("include!(\"locale_base.rs\")"));
    assert!(locale.contains("pub async fn invalidate_all_tenant_locale_cache"));
    assert!(locale.contains("ctx.shared_get::<Arc<TenantLocaleCache>>()"));
    assert!(!locale.contains("tenant_locale_cache(ctx).invalidate_all"));

    for required in [
        "TENANT_CACHE_GENERATION_CHANNEL",
        "TENANT_CACHE_BACKEND_PREFIX",
        "if event.key == \"*\"",
        "invalidate_tenant_locale_cache(&self.ctx, tenant_id).await",
        "invalidate_all_tenant_locale_cache(&self.ctx).await",
        "CacheInvalidationObservation::UnverifiedFirst",
        "CacheInvalidationObservation::Gap",
        "CacheGenerationError::GenerationRegressed",
        "cache cleared and readiness remains failed",
        "async fn recover_if_advanced",
        "Some(previous) if generation > previous",
        "struct TenantLocaleGenerationHealth",
        "ready: AtomicBool",
        "Ok(()) => self.health.mark_ready()",
        "Err(_) => self.health.mark_failed()",
        "pub fn is_ready(&self) -> bool",
        "listener.health.mark_failed();",
        "run_periodic_reconciliation_with_interval",
        "#[path = \"tenant_locale_generation_tests.rs\"]",
        "let durable = self.current_generation().await?;",
        "if durable < event.generation",
        "self.handle_event(event, durable).await",
    ] {
        assert!(
            listener.contains(required),
            "tenant locale generation contract must retain {required}"
        );
    }
    assert!(!listener.contains("self.tracker.reset(TENANT_CACHE_GENERATION_CHANNEL)"));

    let subscription = listener
        .find("subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL)")
        .expect("locale listener must subscribe before startup recovery");
    let recovery = listener
        .find("listener.recover_if_advanced().await")
        .expect("locale listener must recover after subscribing");
    assert!(subscription < recovery);

    let durable_check = listener
        .find("let durable = self.current_generation().await?;")
        .expect("message handling must read the durable generation");
    let apply = listener
        .find("self.handle_event(event, durable).await")
        .expect("message handling must apply only after durable validation");
    let acknowledgement = listener
        .find("acknowledge_locale_applied(&self.tracker, generation)?;")
        .expect("exact invalidation must acknowledge only after apply");
    assert!(durable_check < apply);
    assert!(apply < acknowledgement);

    assert!(listener.contains("TenantLocaleGenerationStartLock"));
    assert!(listener.contains("impl Drop for AbortOnDropTenantLocaleTask"));
    assert!(listener.contains("pub fn is_running(&self) -> bool"));

    let tenant_init = middleware
        .find("super::tenant_legacy::init_tenant_cache_infrastructure")
        .expect("tenant cache infrastructure must initialize");
    let locale_start = middleware
        .find("start_tenant_locale_generation_listener")
        .expect("tenant middleware must start locale generation recovery");
    assert!(tenant_init < locale_start);

    assert!(middleware.contains(
        "let invalidation_key = tenant_id\n            .map(|tenant_id| tenant_id.to_string())\n            .unwrap_or_else(|| \"*\".to_string())"
    ));
    assert!(!middleware.contains("\"tenant-manual-invalidation\","));

    assert!(guardrails.contains("TenantLocaleGenerationListenerHandle"));
    assert!(guardrails.contains("tenant locale durable generation runtime"));
    assert!(guardrails.contains(".map(|handle| handle.is_ready())"));
    assert!(guardrails.contains("RuntimeGuardrailStatus::Critical"));
}

#[test]
fn permanent_gate_retains_multi_replica_tenant_locale_evidence() {
    let evidence = include_str!("../src/services/tenant_locale_generation_tests.rs");
    let workflow = include_str!("../../../.github/workflows/cache-hardening.yml");

    for required in [
        "exact_and_wildcard_invalidation_refresh_two_replica_locale_values",
        "deterministic_local_lag_recovers_two_replica_locale_values",
        "missed_redis_publication_recovers_remote_locale_via_periodic_generation",
        "redis_restart_fails_closed_until_generation_is_restored",
        "for _ in 0..300",
        "assert_eq!(outcome.local_subscribers, 3);",
        "RecvError::Lagged",
        "No PubSub publication occurs.",
        "run_periodic_reconciliation_with_interval",
        "restore_shared_generation",
        "rustok:cache-generation:v1",
        "assert!(!handle_a.is_ready());",
        "assert!(!handle_b.is_ready());",
        "assert_eq!(after_restore, before_restart + 1);",
        "wait_for_readiness(&handle_a, true",
        "wait_for_readiness(&handle_b, true",
    ] {
        assert!(
            evidence.contains(required),
            "tenant locale recovery evidence must retain {required}"
        );
    }

    for required in [
        "apps/server/src/services/tenant_locale_generation*.rs",
        "cargo test -p rustok-server tenant_locale_generation --lib",
        "cargo test -p rustok-server tenant_locale_generation --lib -- --ignored --nocapture --test-threads=1",
        "RUSTOK_CACHE_REDIS_SERVER_BIN: /usr/bin/redis-server",
    ] {
        assert!(
            workflow.contains(required),
            "cache workflow must retain tenant locale evidence command: {required}"
        );
    }
}

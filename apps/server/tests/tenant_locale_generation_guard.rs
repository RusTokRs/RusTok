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

    assert!(listener.contains("TENANT_CACHE_GENERATION_CHANNEL"));
    assert!(listener.contains("TENANT_CACHE_BACKEND_PREFIX"));
    assert!(listener.contains("invalidate_tenant_locale_cache(&self.ctx, tenant_id).await"));
    assert!(listener.contains("invalidate_all_tenant_locale_cache(&self.ctx).await"));
    assert!(listener.contains("CacheInvalidationObservation::UnverifiedFirst"));
    assert!(listener.contains("CacheInvalidationObservation::Gap"));
    assert!(listener.contains("async fn recover_if_advanced"));
    assert!(listener.contains("previous.is_none_or(|previous| generation > previous)"));

    let subscription = listener
        .find("subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL)")
        .expect("locale listener must subscribe before startup recovery");
    let recovery = listener
        .find("listener.recover_if_advanced().await")
        .expect("locale listener must recover after subscribing");
    assert!(subscription < recovery);

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

    assert!(guardrails.contains("TenantLocaleGenerationListenerHandle"));
    assert!(guardrails.contains("tenant locale durable generation runtime"));
    assert!(guardrails.contains("RuntimeGuardrailStatus::Critical"));
}

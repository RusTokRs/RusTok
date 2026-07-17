include!("locale_base.rs");

/// Invalidate every process-local tenant-locale entry after an unverified or gapped durable
/// tenant-generation recovery. Do not create the cache merely to clear an empty runtime.
pub async fn invalidate_all_tenant_locale_cache(ctx: &ServerRuntimeContext) {
    let Some(cache) = ctx.shared_get::<Arc<TenantLocaleCache>>() else {
        return;
    };
    cache.invalidations.fetch_add(1, Ordering::Relaxed);
    cache.cache.invalidate_all();
    cache.cache.run_pending_tasks().await;
}

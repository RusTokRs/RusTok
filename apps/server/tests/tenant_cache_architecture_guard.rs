use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn tenant_cache_uses_weighted_shared_client_backends() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains(".backend_weighted("));
    assert!(!tenant.contains(".backend(\n"));
    assert!(tenant.contains("TENANT_CACHE_MAX_WEIGHT_BYTES"));
    assert!(tenant.contains("TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES"));
}

#[test]
fn tenant_keys_are_canonical_and_schema_versioned() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("CanonicalCacheKeyBuilder::new"));
    assert!(tenant.contains("TENANT_CACHE_VERSION: &str = \"v2\""));
    assert!(tenant.contains("TENANT_CONTEXT_SCHEMA_VERSION"));
    assert!(tenant.contains("TENANT_NEGATIVE_SCHEMA_VERSION"));
}

#[test]
fn tenant_payloads_use_typed_envelopes_and_explicit_negative_policy() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("load_enveloped_or_fill("));
    assert!(tenant.contains("CacheEnvelope::new"));
    assert!(tenant.contains("NegativeCachePolicy::deterministic_jittered"));
    assert!(tenant.contains("get_negative::<CachedTenantMiss>"));
    assert!(tenant.contains("store_negative("));
    assert!(!tenant.contains("serde_json::from_slice::<TenantContext>"));
    assert!(!tenant.contains("serde_json::to_vec(&context)"));
}

#[test]
fn tenant_loader_and_direct_redis_metrics_are_bounded() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("TENANT_CACHE_LOADER_TIMEOUT"));
    assert!(tenant.contains("CacheTtlPolicy::deterministic_jitter"));
    assert!(tenant.contains("tenant_cache_redis_timeout("));
    assert!(tenant.contains("redis::cmd(\"INCR\")"));
    assert!(tenant.contains("redis::cmd(\"GET\")"));
}

#[test]
fn tenant_invalidation_payload_parser_rejects_extra_parts() {
    let tenant = source("apps/server/src/middleware/tenant.rs");
    assert!(tenant.contains("parts.next().is_some()"));
}

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
fn every_runtime_transport_uses_the_canonical_listener_gate() {
    let factory = source("apps/server/src/services/event_transport_factory.rs");
    let gate = source("apps/server/src/services/tenant_generation_delivery_gate.rs");
    let middleware = source("apps/server/src/middleware/mod.rs");

    assert!(factory.contains("TenantGenerationDeliveryGate::new("));
    assert!(factory.contains("TenantCacheGenerationTransport::new(gated, cache.clone())"));
    assert_eq!(
        factory
            .matches("tenant_generation_transport(ctx, &cache,")
            .count(),
        3
    );

    for required in [
        "tenant_cache_generation_listener_snapshot(&self.ctx)",
        "TenantCacheGenerationListenerStatus::Healthy",
        "snapshot.local_ready",
        "unrelated_cache_subscriber_cannot_satisfy_the_tenant_listener_gate",
        "canonical_local_listener_allows_downstream_delivery",
    ] {
        assert!(gate.contains(required), "delivery gate must retain {required}");
    }
    assert!(!gate.contains("local_subscribers"));

    assert!(middleware.contains("tenant_invalidation_listener_snapshot(ctx).await"));
    assert!(middleware.contains("TenantInvalidationListenerStatus::Healthy"));
    assert!(middleware.contains("listener.local_ready"));
    assert!(!middleware.contains("outcome.local_subscribers"));
}

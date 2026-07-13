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
fn generation_store_is_clone_shared_bounded_identity_safe_and_fail_closed() {
    let service = source("crates/rustok-cache/src/service.rs");
    let generation = source("crates/rustok-cache/src/generation.rs");
    let regression = source("crates/rustok-cache/tests/generation_store_regression.rs");

    for required in [
        "generation_store_identity",
        "DEFAULT_MAX_SHARED_GENERATION_STORES",
        "struct RegisteredGenerationStore",
        "_identity: Arc<dyn Any + Send + Sync>",
        "fn generation_store_registry()",
        "return registered.store.clone()",
        "GenerationStoreRegistryCapacityExceeded",
        "return store.reject_registry_capacity()",
        "generation_store_registry_saturation_fails_closed",
        "service_handles_share_generation_snapshots_without_cross_service_aliasing",
    ] {
        assert!(
            service.contains(required) || generation.contains(required),
            "shared generation store contract must retain {required}"
        );
    }

    assert!(generation.contains("registry.len() >= DEFAULT_MAX_SHARED_GENERATION_STORES"));
    assert!(generation.contains("store: store.clone()"));
    assert!(!generation.contains("returning an unshared bounded store"));
    assert!(regression.contains("cache_service_generation_handles_share_trusted_local_snapshots"));
    assert!(
        !generation.contains(
            "CacheNamespaceGenerationStore::new(self.redis_client().cloned())\n        }"
        ),
        "namespace_generations must not return a fresh Redis store on every call"
    );
}

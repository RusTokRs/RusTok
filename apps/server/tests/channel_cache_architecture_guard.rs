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
fn channel_cache_generations_are_bounded_and_fail_safe() {
    let channel = source("apps/server/src/middleware/channel.rs");

    for required in [
        "CHANNEL_CACHE_MAX_TENANT_VERSIONS",
        "struct ChannelCacheVersionState",
        "global_epoch: u32",
        "tenant_epochs: HashMap<Uuid, u32>",
        "if self.tenant_epochs.len() >= maximum_tenants.max(1)",
        "self.rotate_all();",
        "self.cache.invalidate_all();",
        "self.cache.run_pending_tasks().await;",
        "tenant_version_registry_rotates_without_reusing_stale_tokens",
        "repeated_tenant_invalidation_does_not_grow_the_registry",
    ] {
        assert!(
            channel.contains(required),
            "channel cache generation contract must retain {required}"
        );
    }

    assert!(!channel.contains("tenant_versions: Arc<RwLock<HashMap<Uuid, u64>>>") );
    let rotation = channel
        .find("self.rotate_all();")
        .expect("capacity exhaustion must rotate the global epoch");
    let full_invalidation = channel
        .find("self.cache.invalidate_all();")
        .expect("global rotation must clear cached resolutions");
    assert!(rotation < full_invalidation);
}

#[test]
fn channel_cache_is_registered_atomically() {
    let channel = source("apps/server/src/middleware/channel.rs");
    assert!(channel.contains("ctx.shared_insert_if_absent(candidate.clone())"));
    assert!(channel.contains(
        "ctx.shared_get::<Arc<ChannelResolutionCache>>()\n        .unwrap_or(candidate)"
    ));
    assert!(!channel.contains("ctx.shared_insert(cache.clone());"));
}

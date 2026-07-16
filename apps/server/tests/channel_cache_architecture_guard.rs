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
        "next_version: u64",
        "default_version: u64",
        "tenant_versions: HashMap<Uuid, u64>",
        "exhausted: bool",
        "self.next_version.checked_add(1)",
        "self.exhausted = true;",
        "self.tenant_versions.len() >= maximum_tenants.max(1)",
        "self.cache.invalidate_all();",
        "self.cache.run_pending_tasks().await;",
        "if let Some(version) = cache.tenant_version(facts.tenant_id)",
        "version_exhaustion_disables_cache_instead_of_reusing_a_token",
        "tenant_version_registry_rotates_without_reusing_stale_tokens",
        "repeated_tenant_invalidation_does_not_grow_the_registry",
    ] {
        assert!(
            channel.contains(required),
            "channel cache generation contract must retain {required}"
        );
    }

    assert!(!channel.contains("wrapping_add"));
    assert!(!channel.contains("tenant_versions: Arc<RwLock<HashMap<Uuid, u64>>>"));
    let exhaustion = channel
        .find("self.exhausted = true;")
        .expect("version exhaustion must be explicit");
    let full_invalidation = channel
        .find("self.cache.invalidate_all();")
        .expect("capacity or version exhaustion must clear cached resolutions");
    assert!(exhaustion < full_invalidation);
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

#[test]
fn native_channel_mutations_invalidate_resolution_cache() {
    let middleware_mod = source("apps/server/src/middleware/mod.rs");
    let wrapper = source("apps/server/src/middleware/channel_native_wrapper.rs");
    let adapter = source(
        "crates/rustok-channel/admin/src/transport/native_server_adapter.rs",
    );

    assert!(middleware_mod.contains(
        "#[path = \"channel_native_wrapper.rs\"]\npub mod channel;"
    ));
    assert!(wrapper.contains("response.status().is_success()"));
    assert!(wrapper.contains("base::invalidate_tenant_channel_cache(&ctx, tenant_id).await;"));

    let endpoint_marker = "endpoint = \"channel/";
    let endpoints = adapter
        .match_indices(endpoint_marker)
        .filter_map(|(start, _)| {
            let value = &adapter[start + "endpoint = \"".len()..];
            value.find('"').map(|end| &value[..end])
        })
        .collect::<Vec<_>>();

    assert!(endpoints.contains(&"channel/bootstrap"));
    assert!(endpoints.len() > 1, "expected channel mutation endpoints");

    for endpoint in endpoints
        .into_iter()
        .filter(|endpoint| *endpoint != "channel/bootstrap")
    {
        let path = format!("\"/api/fn/{endpoint}\"");
        assert!(
            wrapper.contains(&path),
            "native channel mutation {endpoint} must invalidate the resolution cache"
        );
    }

    assert!(!wrapper.contains("\"/api/fn/channel/bootstrap\""));
}

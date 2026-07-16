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
        "fn invalidate_all(&mut self)",
        "self.default_version = next_version;",
        "self.cache.invalidate_all();",
        "self.cache.run_pending_tasks().await;",
        "if let Some(version) = cache.tenant_version(facts.tenant_id)",
        "version_exhaustion_disables_cache_instead_of_reusing_a_token",
        "tenant_version_registry_rotates_without_reusing_stale_tokens",
        "namespace_invalidation_rotates_default_and_tracked_tokens",
        "repeated_tenant_invalidation_does_not_grow_the_registry",
    ] {
        assert!(
            channel.contains(required),
            "channel cache generation contract must retain {required}"
        );
    }

    assert!(!channel.contains("wrapping_add"));
    assert!(!channel.contains(
        "tenant_versions: Arc<RwLock<HashMap<Uuid, u64>>>"
    ));
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
fn native_and_rest_channel_mutations_publish_durable_invalidation() {
    let middleware_mod = source("apps/server/src/middleware/mod.rs");
    let wrapper = source("apps/server/src/middleware/channel_native_wrapper.rs");
    let adapter = source(
        "crates/rustok-channel/admin/src/transport/native_server_adapter.rs",
    );
    let controller = source("apps/server/src/controllers/channel.rs");

    assert!(middleware_mod.contains(
        "#[path = \"channel_native_wrapper.rs\"]\npub mod channel;"
    ));
    assert!(wrapper.contains("response.status().is_success()"));
    assert!(wrapper.contains("invalidate_tenant_channel_cache_local(ctx, tenant_id).await;"));
    assert!(wrapper.contains("publish_channel_resolution_invalidation(ctx, tenant_id).await;"));
    assert!(wrapper.contains("base::invalidate_all_channel_cache(ctx).await;"));
    assert!(controller.contains("invalidate_tenant_channel_cache(ctx, tenant_id).await;"));

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

#[test]
fn durable_channel_generation_is_database_owned_and_supervised() {
    let migration = source(
        "crates/rustok-channel/src/migrations/m20260716_000009_create_channel_resolution_invalidation_state.rs",
    );
    let migration_registry = source("crates/rustok-channel/src/migrations/mod.rs");
    let runtime = source("apps/server/src/services/channel_cache_invalidation.rs");
    let bootstrap = source("apps/server/src/services/server_bootstrap.rs");
    let guardrails = source("apps/server/src/services/runtime_guardrails.rs");

    assert!(migration_registry.contains(
        "m20260716_000009_create_channel_resolution_invalidation_state"
    ));
    for table in [
        "channels",
        "channel_targets",
        "channel_module_bindings",
        "channel_oauth_apps",
        "channel_resolution_policy_sets",
        "channel_resolution_policy_rules",
    ] {
        assert!(
            migration.contains(&format!("\"{table}\"")),
            "durable generation must cover {table}"
        );
    }
    for required in [
        "generation = generation + 1",
        "AFTER INSERT OR UPDATE OR DELETE",
        "AFTER {event_sql} ON {table}",
        "read_resolution_invalidation_generation",
        "CHANNEL_RESOLUTION_RECONCILE_INTERVAL",
        "subscribe_local_channel(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL)",
        "consume_subscription_with_ready",
        "invalidate_all_channel_cache_local(&self.ctx).await;",
        "spawn_supervised_worker",
        "struct ChannelCacheInvalidationHealth",
        "self.health.mark_failed()",
        "self.health.mark_ready()",
    ] {
        assert!(
            migration.contains(required) || runtime.contains(required),
            "durable channel invalidation contract must retain {required}"
        );
    }
    assert!(!runtime.contains("SELECT id FROM tenants"));
    assert!(bootstrap.contains("start_channel_cache_invalidation_listener"));
    assert!(guardrails.contains("ChannelCacheInvalidationListenerHandle"));
    assert!(guardrails.contains("channel resolution durable invalidation runtime"));
    assert!(guardrails.contains(".map(|handle| handle.is_ready())"));
}

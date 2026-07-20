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
fn native_and_rest_channel_mutations_publish_durable_invalidation() {
    let middleware_mod = source("apps/server/src/middleware/mod.rs");
    let wrapper = source("apps/server/src/middleware/channel_native_wrapper.rs");
    let adapter = source("crates/rustok-channel/admin/src/transport/native_server_adapter.rs");
    let controller = source("apps/server/src/controllers/channel.rs");

    assert!(middleware_mod.contains("#[path = \"channel_native_wrapper.rs\"]\npub mod channel;"));
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

    assert!(
        migration_registry
            .contains("m20260716_000009_create_channel_resolution_invalidation_state")
    );
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

#[test]
fn cache_workflow_retains_channel_compiled_evidence() {
    let workflow = source(".github/workflows/cache-hardening.yml");
    for required in [
        "crates/rustok-channel/**",
        "apps/server/src/services/channel_cache_invalidation*.rs",
        "apps/server/tests/channel_cache*.rs",
        "cargo check -p rustok-channel --lib",
        "cargo test -p rustok-channel invalidation_generation --lib",
        "cargo test -p rustok-server channel_cache_invalidation --lib",
        "cargo test -p rustok-server --test channel_cache_resolved_value",
        "cargo clippy -p flex --lib -- -D warnings",
        "cargo clippy -p rustok-channel --lib -- -D warnings",
        "postgres-channel:",
        "RUSTOK_CHANNEL_TEST_POSTGRES_URL",
        "cargo test -p rustok-channel --test postgres_invalidation_generation -- --ignored --nocapture --test-threads=1",
        "RUSTOK_CACHE_REDIS_SERVER_BIN: /usr/bin/redis-server",
        "sudo apt-get install -y redis-server redis-tools",
        "cargo test -p rustok-server redis_publication_drives_remote_replica_readiness_recovery --lib -- --ignored --nocapture --test-threads=1",
        "cargo test -p rustok-server --test channel_cache_resolved_value -- --ignored --nocapture --test-threads=1",
    ] {
        assert!(
            workflow.contains(required),
            "cache workflow must retain compiled evidence command: {required}"
        );
    }

    let generation = source("crates/rustok-channel/src/invalidation_generation.rs");
    for required in [
        "durable_generation_converges_across_replica_readers_without_pubsub",
        "missing_generation_state_fails_closed_and_recovers_after_restore",
    ] {
        assert!(
            generation.contains(required),
            "channel generation tests must retain {required}"
        );
    }

    let runtime = source("apps/server/src/services/channel_cache_invalidation_runtime_tests.rs");
    for required in [
        "independent_replicas_fail_closed_and_recover_without_redis",
        "local_listener_lag_fails_closed_and_recovers_from_durable_state",
        "redis_publication_drives_remote_replica_readiness_recovery",
        "for _ in 0..300",
        "RecvError::Lagged",
        "assert_eq!(outcome.local_subscribers, 2);",
        "tokio::time::timeout(Duration::from_secs(3)",
        "let ctx_c = ServerRuntimeContext::new",
        "assert!(!recovering_remote.is_ready());",
        "publish_redis_until_readiness(&cache_a, &recovering_remote, true, 2).await;",
    ] {
        assert!(
            runtime.contains(required),
            "channel replica recovery evidence must retain {required}"
        );
    }
    assert!(runtime.contains("wait_for_readiness(&handle_a, false).await;"));
    assert!(runtime.contains("wait_for_readiness(&handle_b, true).await;"));

    let services_mod = source("apps/server/src/services/mod.rs");
    assert!(services_mod.contains("mod channel_cache_invalidation_resolved_value_tests;"));
    let lag_resolved =
        source("apps/server/src/services/channel_cache_invalidation_resolved_value_tests.rs");
    for required in [
        "local_listener_lag_recovers_readiness_and_remote_resolved_value",
        "channel_middleware::resolve",
        "Some(\"Before listener lag\")",
        "for _ in 0..300",
        "assert_eq!(outcome.local_subscribers, 2);",
        "assert!(!outcome.redis_published);",
        "RecvError::Lagged",
        "wait_for_readiness(&handle, false).await;",
        "No replacement fast-path publication is sent.",
        "restore_generation_state(&db, generation).await;",
        "wait_for_channel_name(&app, &tenant_context, \"After listener lag\").await;",
        "wait_for_readiness(&handle, true).await;",
    ] {
        assert!(
            lag_resolved.contains(required),
            "combined channel lag/value evidence must retain {required}"
        );
    }
    assert!(!lag_resolved.contains("publish_channel_resolution_invalidation"));

    let resolved = source("apps/server/tests/channel_cache_resolved_value.rs");
    for required in [
        "missed_publication_refreshes_remote_resolved_value_via_durable_poll",
        "database_state_loss_fails_closed_and_recovers_resolved_value",
        "generation_regression_rebuilds_remote_resolved_value",
        "redis_invalidation_refreshes_remote_resolved_channel_value_before_poll",
        "redis_restart_reconnects_existing_replicas_and_refreshes_value_before_poll",
        "channel_middleware::resolve",
        "Before invalidation",
        "After database recovery",
        "After generation regression",
        "During Redis outage",
        "After Redis reconnect",
        "No publication occurs.",
        "PUBSUB",
        "NUMSUB",
        "tokio::time::timeout(Duration::from_secs(7)",
        "tokio::time::timeout(Duration::from_secs(3)",
        "publish_channel_resolution_invalidation(&ctx_a, tenant.id).await;",
    ] {
        assert!(
            resolved.contains(required),
            "resolved channel value evidence must retain {required}"
        );
    }

    let postgres = source("crates/rustok-channel/tests/postgres_invalidation_generation.rs");
    for required in [
        "postgres_generation_is_transactional_concurrent_and_recoverable",
        "ConnectOptions::new(url.to_string())",
        "let replica = connect_postgres(url.as_str()).await;",
        "let mutation_a = tokio::spawn",
        "let mutation_b = tokio::spawn",
        "migration.up(&manager).await.unwrap();",
    ] {
        assert!(
            postgres.contains(required),
            "PostgreSQL channel generation evidence must retain {required}"
        );
    }
}

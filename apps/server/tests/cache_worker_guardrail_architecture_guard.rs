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
fn runtime_guardrails_surface_cache_and_rbac_worker_failures() {
    let guardrails = source("apps/server/src/services/runtime_guardrails.rs");
    let base = source("apps/server/src/services/runtime_guardrails_base.rs");

    for required in [
        "include!(\"runtime_guardrails_base.rs\")",
        "pub use base::{",
        "base::collect_runtime_guardrail_snapshot(ctx).await",
        "RbacCacheInvalidationListenerHandle",
        "RbacInvalidationGenerationWatchdogHandle",
        "CacheRedisStatusMonitorHandle",
        "FieldDefinitionCacheInvalidationHandle",
        "RBAC cache invalidation runtime",
        "RBAC durable generation watchdog",
        "cache Redis status monitor",
        "field definition cache invalidation consumer",
        "RuntimeGuardrailStatus::Critical",
        "RuntimeGuardrailStatus::Degraded",
        "apply_rollout_status(&mut snapshot)",
        "terminal_critical_worker_respects_rollout_mode",
        "noncritical_worker_does_not_lower_existing_severity",
    ] {
        assert!(
            guardrails.contains(required),
            "runtime worker guardrail wrapper must retain {required}"
        );
    }

    for required in [
        "pub enum RuntimeGuardrailStatus",
        "pub struct RuntimeGuardrailSnapshot",
        "pub async fn collect_runtime_guardrail_snapshot",
        "collect_rate_limit_snapshot",
        "collect_remote_executor_snapshot",
    ] {
        assert!(
            base.contains(required),
            "base runtime guardrails must retain {required}"
        );
    }
}

#[test]
fn registry_only_mode_skips_runtime_worker_requirements() {
    let guardrails = source("apps/server/src/services/runtime_guardrails.rs");
    let skip = guardrails
        .find("if !snapshot.runtime_dependencies_enabled")
        .expect("registry-only guard must remain explicit");
    let first_worker = guardrails
        .find("RbacCacheInvalidationListenerHandle")
        .expect("worker checks must remain present");
    assert!(skip < first_worker);
}

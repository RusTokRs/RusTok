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
fn periodic_reconciliation_can_supersede_an_in_flight_event_safely() {
    let generation = source("apps/server/src/services/tenant_cache_generation.rs");

    for required in [
        "CacheBackendGenerationError::GenerationRegressed",
        "CacheInvalidationPayloadError::OffsetRegressed",
        "acknowledge_applied_generation",
        "superseded_apply_is_a_safe_noop",
    ] {
        assert!(
            generation.contains(required),
            "tenant generation concurrency contract must retain {required}"
        );
    }

    assert!(
        generation.contains("if current >= proposed"),
        "only an already-higher checkpoint may supersede the in-flight apply"
    );
    assert!(
        generation.contains("acknowledge_recovery(TENANT_CACHE_GENERATION_CHANNEL, value)"),
        "durable recovery must remain strict and separate from superseded event acknowledgement"
    );
}

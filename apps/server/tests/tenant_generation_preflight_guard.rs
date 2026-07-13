use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

#[test]
fn tenant_generation_validates_deterministic_fields_before_bump() {
    let path = repo_root().join("apps/server/src/services/tenant_cache_generation.rs");
    let generation = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    let timestamp = generation
        .find("u64::try_from(envelope.timestamp.timestamp_millis())")
        .expect("tenant invalidation timestamp must be checked");
    let preflight = generation
        .find("let _preflight = Self::validated_record(")
        .expect("tenant invalidation record must be preflighted");
    let bump = generation
        .find(".bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)")
        .expect("tenant generation must advance durably");
    let publish = generation
        .find(".publish_durable(&record)")
        .expect("validated tenant invalidation must be published");
    let commit = generation
        .find("successful_rotations.observe(envelope.id)")
        .expect("successful rotation must commit its event ID");

    assert!(
        timestamp < preflight && preflight < bump && bump < publish && publish < commit
    );
    assert!(generation.contains("PREFLIGHT_GENERATION: u64 = 1"));
    assert!(generation.contains("malformed_envelope_does_not_advance_generation"));
}

use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("apps/server parent")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn extract_function_block(source: &str, signature: &str) -> Option<String> {
    let start = source.find(signature)?;
    let rest = &source[start..];
    let body_start = rest.find('{')?;
    let mut depth = 0usize;
    let mut end_idx = None;

    for (idx, ch) in rest[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(body_start + idx + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.map(|end| rest[..end].to_string())
}

#[test]
fn graphql_module_composition_mutations_use_atomic_orchestration_service() {
    let path = repo_root().join("apps/server/src/graphql/mutations.rs");
    let source = fs::read_to_string(&path).expect("read mutations.rs");
    let request_helper =
        extract_function_block(&source, "async fn request_module_composition_build(")
            .expect("composition build request helper should exist");

    assert!(
        request_helper
            .contains("PlatformCompositionBuildService::apply_module_mutation_and_request_build("),
        "request helper must use atomic PlatformCompositionBuildService orchestration"
    );

    for signature in [
        "async fn install_module(",
        "async fn uninstall_module(",
        "async fn upgrade_module(",
    ] {
        let block = extract_function_block(&source, signature)
            .unwrap_or_else(|| panic!("expected mutation function {signature}"));
        assert!(
            block.contains("request_module_composition_build("),
            "{signature} must route through the atomic composition build service"
        );
        for required in [
            "expected_revision: i64",
            "idempotency_key: Uuid",
            "tenant_id: tenant.id",
            "actor_id: auth.user_id",
            "idempotency_key,",
        ] {
            assert!(
                block.contains(required),
                "{signature} must provide typed owner mutation field `{required}`"
            );
        }
        assert!(
            !block.contains("requested_by:"),
            "{signature} must not accept caller-controlled actor text"
        );
    }
}

#[test]
fn platform_composition_manifest_hash_uses_canonical_snapshot_contract() {
    let path = repo_root().join("apps/server/src/services/platform_composition.rs");
    let source = fs::read_to_string(&path).expect("read platform_composition.rs");

    let helper = extract_function_block(
        &source,
        "pub fn manifest_hash(manifest: &ModulesManifest) -> Result<String, PlatformCompositionError>",
    )
    .expect("manifest_hash helper should exist");

    assert!(
        helper.contains("Self::manifest_snapshot_json("),
        "manifest_hash must serialize through the canonical composition snapshot contract"
    );
    assert!(
        helper.contains("hash_manifest_snapshot(&"),
        "manifest_hash must hash the canonical composition snapshot"
    );
}

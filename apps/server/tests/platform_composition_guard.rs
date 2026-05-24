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

    let helper = extract_function_block(&source, "async fn persist_manifest_and_request_build(")
        .expect("persist helper should exist");

    assert!(
        helper.contains("PlatformCompositionBuildService::update_manifest_and_request_build("),
        "persist helper must use atomic PlatformCompositionBuildService orchestration"
    );
    assert!(
        !helper.contains("PlatformCompositionService::update_manifest("),
        "persist helper must not call composition update separately"
    );
    assert!(
        !helper.contains(".request_build("),
        "persist helper must not call build enqueue separately"
    );

    for signature in [
        "async fn install_module(",
        "async fn uninstall_module(",
        "async fn upgrade_module(",
    ] {
        let block = extract_function_block(&source, signature)
            .unwrap_or_else(|| panic!("expected mutation function {signature}"));
        assert!(
            block.contains("persist_manifest_and_request_build("),
            "{signature} must route through persist helper"
        );
    }
}


#[test]
fn platform_composition_manifest_hash_uses_shared_typed_hash_helper() {
    let path = repo_root().join("apps/server/src/services/platform_composition.rs");
    let source = fs::read_to_string(&path).expect("read platform_composition.rs");

    let helper = extract_function_block(&source, "pub fn manifest_hash(manifest: &ModulesManifest) -> String")
        .expect("manifest_hash helper should exist");

    assert!(
        helper.contains("hash_manifest(manifest)"),
        "manifest_hash must use shared rustok_api::manifest_hash::hash_manifest helper"
    );
}

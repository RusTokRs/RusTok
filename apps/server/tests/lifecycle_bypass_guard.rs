use std::fs;
use std::path::{Path, PathBuf};

fn collect_rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if name == "target" || name == ".git" || name == "node_modules" {
                    continue;
                }
                collect_rust_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
}

#[test]
fn bypass_toggle_api_is_not_used_in_production_paths() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");

    let apps_server_root = repo_root.join("apps/server");
    let apps_admin_root = repo_root.join("apps/admin");
    let allowed_files = [apps_server_root.join("src/models/tenant_modules.rs")];
    let ignored_files = [apps_server_root.join("tests/lifecycle_bypass_guard.rs")];
    let forbidden_pattern = "upsert_flag_without_lifecycle_for_migrations_only(";

    let mut rust_files = Vec::new();
    collect_rust_files(&apps_server_root, &mut rust_files);
    collect_rust_files(&apps_admin_root, &mut rust_files);

    let mut offenders = Vec::new();
    for file in rust_files {
        if allowed_files.iter().any(|allowed| allowed == &file) {
            continue;
        }
        if ignored_files.iter().any(|ignored| ignored == &file) {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&file) {
            if content.contains(forbidden_pattern) {
                let rel = file
                    .strip_prefix(repo_root)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| file.display().to_string());
                offenders.push(rel);
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Forbidden bypass API usage found outside tenant_modules model file: {offenders:?}"
    );
}

#[test]
fn graphql_mutations_do_not_reintroduce_duplicate_platform_composition_mapping_tests() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    let expected_unique_tests = [
        "fn platform_composition_error_maps_revision_conflict_with_expected_and_current()",
        "fn platform_composition_build_error_maps_build_failures_to_internal_error()",
        "fn platform_composition_build_error_maps_manifest_validation_to_user_facing_message()",
        "fn platform_composition_build_error_maps_revision_conflict_to_conflict_message()",
    ];

    for signature in expected_unique_tests {
        let occurrences = content.matches(signature).count();
        assert_eq!(
            occurrences, 1,
            "Expected exactly one `{signature}` test, found {occurrences}."
        );
    }

    assert!(
        !content.contains("\"queue unavailable\""),
        "Obsolete platform composition build error fixture (`queue unavailable`) reintroduced."
    );

    let forbidden_legacy_tests = [
        "fn platform_composition_error_maps_revision_conflict_to_conflict_message()",
        "fn platform_composition_build_error_maps_enqueue_failures_to_internal_error()",
        "fn platform_composition_build_error_maps_composition_conflict_consistently()",
    ];

    for signature in forbidden_legacy_tests {
        assert!(
            !content.contains(signature),
            "Legacy/duplicate platform composition mapping test signature reintroduced: {signature}"
        );
    }
}

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
fn bypass_toggle_api_is_not_public() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let tenant_modules_rs = repo_root.join("apps/server/src/models/tenant_modules.rs");
    let content = fs::read_to_string(&tenant_modules_rs).expect("tenant_modules.rs should be readable");

    let entity_impl_anchor = "impl Entity {";
    let entity_method_signature =
        "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only(";
    let wrapper_signature =
        "\n#[allow(dead_code)]\npub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only(";
    let public_signature = "pub async fn upsert_flag_without_lifecycle_for_migrations_only(";

    let entity_impl_pos = content
        .find(entity_impl_anchor)
        .expect("Entity impl block should exist");
    let entity_method_pos = content
        .find(entity_method_signature)
        .expect("Entity bypass helper should exist");
    assert!(
        entity_method_pos > entity_impl_pos,
        "Entity bypass helper should be declared inside the Entity impl section."
    );

    assert_eq!(
        content.matches(entity_method_signature).count(),
        2,
        "Expected exactly two crate-scoped bypass helper signatures (Entity method + module wrapper)."
    );
    assert_eq!(
        content.matches(wrapper_signature).count(),
        1,
        "Expected exactly one module-level bypass wrapper signature with dead_code annotation."
    );

    let wrapper_block = extract_function_block(&content, wrapper_signature)
        .expect("module-level bypass wrapper should stay crate-scoped with dead_code annotation");
    assert!(
        wrapper_block.contains(
            "Entity::upsert_flag_without_lifecycle_for_migrations_only(db, tenant_id, module_slug, enabled)"
        ),
        "Module-level bypass wrapper should delegate to Entity helper, not duplicate lifecycle bypass logic."
    );
    assert!(
        !wrapper_block.contains("Self::find("),
        "Module-level bypass wrapper must not introduce direct persistence logic."
    );

    assert!(
        !content.contains(public_signature),
        "Bypass helper must not be public."
    );
}

fn extract_function_block<'a>(content: &'a str, signature: &str) -> Option<&'a str> {
    let start = content.find(signature)?;
    let rest = &content[start..];
    let open_rel = rest.find('{')?;
    let mut depth = 0usize;
    let mut end_rel = None;

    for (idx, ch) in rest.char_indices().skip(open_rel) {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    end_rel = Some(idx + ch.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }

    end_rel.map(|end| &rest[..end])
}

#[test]
fn extract_function_block_handles_nested_braces() {
    let source = r#"
#[allow(dead_code)]
pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only() {
    if true {
        let nested = || {
            let map = std::collections::BTreeMap::<String, String>::new();
            map
        };
        let _ = nested();
    }
}

pub(crate) async fn other_helper() {}
"#;

    let extracted = extract_function_block(
        source,
        "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only()",
    )
    .expect("function should be extracted");
    assert!(extracted.contains("BTreeMap::<String, String>::new()"));
    assert!(extracted.trim_end().ends_with('}'));
    assert!(!extracted.contains("pub(crate) async fn other_helper()"));
}

#[test]
fn extract_function_block_returns_none_for_missing_signature() {
    let source = "pub(crate) async fn other_helper() {}";
    assert!(
        extract_function_block(
            source,
            "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only("
        )
        .is_none()
    );
}

#[test]
fn extract_function_block_returns_none_for_unbalanced_braces() {
    let source = r#"
pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only() {
    if true {
        let _x = 1;
}
"#;

    assert!(
        extract_function_block(
            source,
            "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only()"
        )
        .is_none()
    );
}

#[test]
fn extract_function_block_returns_none_when_body_brace_missing() {
    let source = "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only()";
    assert!(
        extract_function_block(
            source,
            "pub(crate) async fn upsert_flag_without_lifecycle_for_migrations_only()"
        )
        .is_none()
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

#[test]
fn graphql_mutations_toggle_error_mapping_tests_stay_matrix_based() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    let expected_unique_tests = [
        "fn toggle_error_maps_database_and_policy_to_internal_errors()",
        "fn toggle_error_taxonomy_matrix_stays_stable()",
        "fn toggle_error_mapping_sets_expected_error_codes()",
    ];

    for signature in expected_unique_tests {
        let occurrences = content.matches(signature).count();
        assert_eq!(
            occurrences, 1,
            "Expected exactly one `{signature}` test, found {occurrences}."
        );
    }

    let forbidden_legacy_tests = [
        "fn toggle_error_maps_unknown_module()",
        "fn toggle_error_maps_core_module_disable()",
        "fn toggle_error_maps_dependency_errors()",
        "fn toggle_error_maps_hook_failure()",
    ];

    for signature in forbidden_legacy_tests {
        assert!(
            !content.contains(signature),
            "Legacy/duplicate toggle mapping test signature reintroduced: {signature}"
        );
    }
}

#[test]
fn lifecycle_hook_phases_adr_is_linked_from_indexes_and_backlog() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");

    let adr_path = "DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md";
    let decisions_readme = fs::read_to_string(repo_root.join("DECISIONS/README.md"))
        .expect("DECISIONS/README.md should be readable");
    let docs_index =
        fs::read_to_string(repo_root.join("docs/index.md")).expect("docs/index.md should be readable");
    let remediation = fs::read_to_string(
        repo_root.join("docs/research/control-plane-module-lifecycle-remediation-plan.md"),
    )
    .expect("remediation plan should be readable");

    assert!(
        decisions_readme.contains(adr_path),
        "ADR index must link lifecycle hook phases ADR: {adr_path}"
    );
    assert!(
        docs_index.contains(adr_path),
        "docs/index.md must link lifecycle hook phases ADR: {adr_path}"
    );
    assert!(
        remediation.contains("2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md"),
        "remediation backlog must reference lifecycle hook phases ADR explicitly"
    );
}

#[test]
fn toggle_graphql_error_mapper_uses_typed_error_categories() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    let mapper_body = extract_function_block(
        &content,
        "fn map_toggle_module_error(error: ToggleModuleError) -> FieldError",
    )
    .expect("toggle mapper should exist");

    assert!(
        mapper_body.contains("FieldError::new(toggle_err_hook_failed("),
        "toggle mapper must use explicit hook-failure builder for structured MODULE_HOOK_FAILED extensions"
    );
    assert!(
        mapper_body.contains("<FieldError as GraphQLError>::bad_user_input("),
        "toggle mapper must contain BAD_USER_INPUT mapping for user-facing cases"
    );
    assert!(
        mapper_body.contains("<FieldError as GraphQLError>::internal_error("),
        "toggle mapper must contain INTERNAL_ERROR mapping for internal failures"
    );
}

#[test]
fn toggle_graphql_error_mapper_preserves_expected_variant_contract() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    let mapper_body = extract_function_block(
        &content,
        "fn map_toggle_module_error(error: ToggleModuleError) -> FieldError",
    )
    .expect("toggle mapper should exist");

    let expected_branches = [
        "ToggleModuleError::UnknownModule",
        "ToggleModuleError::CoreModuleCannotBeDisabled(",
        "ToggleModuleError::MissingDependencies(",
        "ToggleModuleError::HasDependents(",
        "ToggleModuleError::PreHookFailed(",
        "ToggleModuleError::PostHookFailed(",
        "ToggleModuleError::Database(",
        "ToggleModuleError::Policy(",
    ];

    for branch in expected_branches {
        assert!(
            mapper_body.contains(branch),
            "toggle mapper branch missing: {branch}"
        );
    }

    assert!(
        mapper_body.contains("toggle_err_hook_failed"),
        "hook-failure branch must use explicit helper message contract"
    );
    assert!(
        mapper_body.contains("ext.set(\"retryable_issue\""),
        "hook-failure mapping must expose retryable_issue extension"
    );
    assert!(
        mapper_body.contains("ext.set(\"operation_issue\""),
        "hook-failure mapping must expose operation_issue extension"
    );
}

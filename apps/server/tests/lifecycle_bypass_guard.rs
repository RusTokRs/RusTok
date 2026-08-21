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
    let ignored_files = [apps_server_root.join("tests/lifecycle_bypass_guard.rs")];
    let forbidden_pattern = ["upsert_flag_without_lifecycle_for_migrations_only", "("].concat();

    let mut rust_files = Vec::new();
    collect_rust_files(&apps_server_root, &mut rust_files);
    collect_rust_files(&apps_admin_root, &mut rust_files);

    let mut offenders = Vec::new();
    for file in rust_files {
        if ignored_files.iter().any(|ignored| ignored == &file) {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&file)
            && content.contains(&forbidden_pattern)
        {
            let rel = file
                .strip_prefix(repo_root)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| file.display().to_string());
            offenders.push(rel);
        }
    }

    assert!(
        offenders.is_empty(),
        "Forbidden lifecycle bypass usage found: {offenders:?}"
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
fn graphql_mutations_do_not_reintroduce_duplicate_platform_composition_mapping_tests() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    let expected_unique_tests = [
        "fn platform_composition_error_maps_revision_conflict_with_expected_and_current()",
        "fn platform_composition_error_matrix_preserves_taxonomy_for_internal_and_user_paths()",
        "fn platform_composition_build_error_matrix_preserves_message_and_code_contract()",
        "fn platform_composition_build_error_mapping_never_mentions_partial_rollback()",
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

    let adr_file = "2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md";
    let decisions_readme = fs::read_to_string(repo_root.join("DECISIONS/README.md"))
        .expect("DECISIONS/README.md should be readable");
    let docs_index = fs::read_to_string(repo_root.join("docs/index.md"))
        .expect("docs/index.md should be readable");
    assert!(
        decisions_readme.contains(adr_file),
        "ADR index must link lifecycle hook phases ADR: {adr_file}"
    );
    assert!(
        docs_index.contains(adr_file),
        "docs/index.md must link lifecycle hook phases ADR: {adr_file}"
    );
}

#[test]
fn control_plane_lifecycle_docs_capture_final_parity_contract() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let central_modules = fs::read_to_string(repo_root.join("docs/architecture/modules.md"))
        .expect("docs/architecture/modules.md should be readable");
    let server_docs = fs::read_to_string(repo_root.join("apps/server/docs/README.md"))
        .expect("apps/server docs should be readable");
    let admin_docs = fs::read_to_string(repo_root.join("apps/admin/docs/README.md"))
        .expect("apps/admin docs should be readable");
    for required in [
        "ModuleLifecycleService::toggle_module()",
        "BAD_USER_INPUT",
        "MODULE_HOOK_FAILED",
        "INTERNAL_ERROR",
        "Leptos SSR/admin",
    ] {
        assert!(
            central_modules.contains(required),
            "central module architecture docs must capture final lifecycle parity fragment `{required}`"
        );
    }

    for required in [
        "validated/running/committed/failed",
        "GraphQL maps canonical lifecycle/recovery facts",
        "module_operations",
        "admin/SSR clients must not remap",
    ] {
        assert!(
            server_docs.contains(required),
            "server local docs must capture final lifecycle contract fragment `{required}`"
        );
    }

    for required in [
        "GraphQL-only entrypoint contract",
        "correlation_id",
        "requested_by",
        "retryable_issue",
        "client-side remap",
        "Lifecycle recovery",
    ] {
        assert!(
            admin_docs.contains(required),
            "admin local docs must capture final lifecycle parity fragment `{required}`"
        );
    }
}

#[test]
fn lifecycle_operation_status_model_is_exposed_through_recovery_surface() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let types_rs = repo_root.join("apps/server/src/graphql/types.rs");
    let queries_rs = repo_root.join("apps/server/src/graphql/queries.rs");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let operation_store_rs = repo_root.join("crates/rustok-modules/src/operation_store.rs");
    let lifecycle_writer_rs = repo_root.join("crates/rustok-modules/src/lifecycle_writer.rs");
    let admin_api_rs = repo_root.join("apps/admin/src/features/modules/transport/types.rs");
    let types = fs::read_to_string(&types_rs).expect("graphql/types.rs should be readable");
    let queries = fs::read_to_string(&queries_rs).expect("graphql/queries.rs should be readable");
    let mutations =
        fs::read_to_string(&mutations_rs).expect("graphql/mutations.rs should be readable");
    let operation_store =
        fs::read_to_string(&operation_store_rs).expect("owner operation store should be readable");
    let lifecycle_writer = fs::read_to_string(&lifecycle_writer_rs)
        .expect("owner lifecycle writer should be readable");
    let admin_api =
        fs::read_to_string(&admin_api_rs).expect("admin module transport should be readable");

    for required in [
        "Validated",
        "Running",
        "Committed",
        "Failed",
        "ModuleOperationStatus::Validated.as_str()",
        "ModuleOperationStatus::Running",
        "ModuleOperationStatus::Committed",
        "ModuleOperationStatus::Failed",
        "pub async fn mark_running",
        "pub async fn mark_committed",
        "pub async fn mark_failed",
    ] {
        assert!(
            operation_store.contains(required),
            "owner operation store must preserve explicit operation status fragment `{required}`"
        );
    }

    for field in [
        "pub status: String",
        "pub issue: String",
        "pub retryable: bool",
        "pub recommended_action: String",
        "pub correlation_id: Option<String>",
        "pub requested_by: Option<String>",
        "pub error_message: Option<String>",
        "status: plan.status.as_str().to_string()",
    ] {
        assert!(
            types.contains(field),
            "GraphQL recovery plan type must expose lifecycle read-side field `{field}`"
        );
    }

    for surface in [queries.as_str(), mutations.as_str()] {
        assert!(
            surface.contains("ModuleOperationRecoveryPlan::from(&plan)"),
            "GraphQL recovery read/write surface must map service recovery plans through the typed GraphQL plan"
        );
    }

    for service_fragment in [
        "if plan.issue != ModuleOperationIssue::PostHookFailed",
        "ModuleOperationRecoveryError::NotRetryable",
        "current_override_enabled != plan.requested_override_enabled",
        "plan.previous_override_enabled",
    ] {
        assert!(
            lifecycle_writer.contains(service_fragment),
            "compensation must be limited to failed committed post-hook operations and restore previous state via `{service_fragment}`"
        );
    }

    for admin_fragment in [
        "status issue retryable recommendedAction correlationId requestedBy errorMessage",
        "retryFailedModuleOperationPostHook(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision)",
        "compensateFailedModuleOperation(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision)",
        "updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey)",
    ] {
        assert!(
            admin_api.contains(admin_fragment),
            "admin recovery GraphQL contract must consume lifecycle status/read-side fragment `{admin_fragment}`"
        );
    }
}

#[test]
fn control_plane_graphql_taxonomy_uses_canonical_error_codes() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mutations_rs = repo_root.join("apps/server/src/graphql/mutations.rs");
    let content = fs::read_to_string(&mutations_rs).expect("mutations.rs should be readable");

    for required in [
        r#"Some("BAD_USER_INPUT")"#,
        r#"Some("MODULE_HOOK_FAILED")"#,
        r#"Some("INTERNAL_ERROR")"#,
    ] {
        assert!(
            content.contains(required),
            "control-plane GraphQL taxonomy must preserve canonical code fragment `{required}`"
        );
    }

    assert!(
        !content.contains("INTERNAL_SERVER_ERROR"),
        "control-plane GraphQL taxonomy must use INTERNAL_ERROR, not legacy INTERNAL_SERVER_ERROR"
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
        "ToggleModuleError::InvalidCommandIdentity",
        "ToggleModuleError::InvalidIdempotencyKey",
        "ToggleModuleError::IdempotencyConflict",
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

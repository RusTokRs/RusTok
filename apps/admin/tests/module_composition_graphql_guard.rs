use std::fs;
use std::path::Path;

#[test]
fn native_module_composition_endpoints_are_not_declared() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for endpoint in [
        "endpoint = \"admin/install-module\"",
        "endpoint = \"admin/uninstall-module\"",
        "endpoint = \"admin/upgrade-module\"",
        "endpoint = \"admin/toggle-module\"",
    ] {
        assert!(
            !content.contains(endpoint),
            "Forbidden native module composition endpoint found: {endpoint}"
        );
    }
}

#[test]
fn module_composition_client_helpers_do_not_call_native_paths() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for native_call in [
        "install_module_native(",
        "uninstall_module_native(",
        "upgrade_module_native(",
        "toggle_module_native(",
    ] {
        assert!(
            !content.contains(native_call),
            "Forbidden native composition call path found: {native_call}"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_use_native_graphql_fallback_combiner() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for helper in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
    ] {
        let helper_body = extract_function_block(&content, helper)
            .unwrap_or_else(|| panic!("helper signature not found: {helper}"));

        assert!(
            !helper_body.contains("combine_native_and_graphql_error"),
            "Forbidden native/graphql fallback combiner found in module composition helper: {helper}"
        );
    }
}

#[test]
fn module_composition_helpers_use_graphql_contract_payloads() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    assert_graphql_only_helper(
        &content,
        "pub async fn install_module(",
        "INSTALL_MODULE_MUTATION",
        "InstallModuleVariables {",
        &["slug,", "version,"],
        "Ok(response.install_module)",
    );
    assert_graphql_only_helper(
        &content,
        "pub async fn uninstall_module(",
        "UNINSTALL_MODULE_MUTATION",
        "UninstallModuleVariables {",
        &["slug,"],
        "Ok(response.uninstall_module)",
    );
    assert_graphql_only_helper(
        &content,
        "pub async fn upgrade_module(",
        "UPGRADE_MODULE_MUTATION",
        "UpgradeModuleVariables {",
        &["slug,", "version,"],
        "Ok(response.upgrade_module)",
    );
}


#[test]
fn module_composition_helpers_forward_auth_context_without_local_overrides() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        assert_eq!(
            helper_body.matches("request(").count(),
            1,
            "{signature} must perform exactly one GraphQL request call"
        );
        assert!(
            helper_body.contains("token,"),
            "{signature} must forward token to canonical GraphQL request"
        );
        assert!(
            helper_body.contains("tenant_slug,"),
            "{signature} must forward tenant_slug to canonical GraphQL request"
        );
        assert!(
            !helper_body.contains(".map_err("),
            "{signature} must not remap canonical GraphQL ApiError taxonomy"
        );
        assert!(
            !helper_body.contains("Some("),
            "{signature} must not locally override auth context when forwarding request"
        );
        assert!(
            !helper_body.contains("None"),
            "{signature} must not locally null auth context when forwarding request"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_branch_on_runtime_error_taxonomy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for forbidden in [
            "UNKNOWN_MODULE",
            "CORE_MODULE",
            "MISSING_DEPENDENCIES",
            "HAS_DEPENDENTS",
            "MODULE_HOOK_FAILED",
            "extensions.code",
            "reason_code",
            "module_operations",
            "correlation_id",
            "requested_by",
        ] {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not branch on runtime taxonomy fragment `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_cross_wire_foreign_mutation_contracts() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            "INSTALL_MODULE_MUTATION",
            [
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
        (
            "pub async fn uninstall_module(",
            "UNINSTALL_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
        (
            "pub async fn upgrade_module(",
            "UPGRADE_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
    ];

    for (signature, required, forbidden_list) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));
        assert!(
            helper_body.contains(required),
            "{signature} must reference canonical mutation constant `{required}`"
        );
        for forbidden in forbidden_list {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not cross-wire foreign mutation constant `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_use_typed_responses_and_direct_payload_returns() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            "let response: InstallModuleResponse",
            "Ok(response.install_module)",
            [
                "response.uninstall_module",
                "response.upgrade_module",
                "response.toggle_module",
            ],
        ),
        (
            "pub async fn uninstall_module(",
            "let response: UninstallModuleResponse",
            "Ok(response.uninstall_module)",
            [
                "response.install_module",
                "response.upgrade_module",
                "response.toggle_module",
            ],
        ),
        (
            "pub async fn upgrade_module(",
            "let response: UpgradeModuleResponse",
            "Ok(response.upgrade_module)",
            [
                "response.install_module",
                "response.uninstall_module",
                "response.toggle_module",
            ],
        ),
    ];

    for (signature, typed_response, canonical_return, forbidden_returns) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        assert!(
            helper_body.contains(typed_response),
            "{signature} must decode GraphQL response into `{typed_response}`"
        );
        assert!(
            helper_body.contains(canonical_return),
            "{signature} must return canonical payload `{canonical_return}`"
        );
        for forbidden in forbidden_returns {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not return foreign payload fragment `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_mutation_constants_are_declared_once() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for constant in [
        "pub const INSTALL_MODULE_MUTATION: &str =",
        "pub const UNINSTALL_MODULE_MUTATION: &str =",
        "pub const UPGRADE_MODULE_MUTATION: &str =",
    ] {
        let occurrences = content.matches(constant).count();
        assert_eq!(
            occurrences, 1,
            "expected exactly one mutation constant declaration `{constant}`, found {occurrences}"
        );
    }
}

#[test]
fn toggle_module_helper_uses_graphql_only_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    assert!(
        !helper_body.contains("combine_native_and_graphql_error"),
        "toggle_module must not compose native/graphql fallback errors"
    );
    assert!(
        !helper_body.contains("toggle_module_native("),
        "toggle_module must not call native helper"
    );
    assert!(
        helper_body.contains("TOGGLE_MODULE_MUTATION"),
        "toggle_module must use canonical TOGGLE_MODULE_MUTATION contract"
    );
    assert_eq!(
        helper_body.matches("TOGGLE_MODULE_MUTATION").count(),
        1,
        "toggle_module must reference TOGGLE_MODULE_MUTATION exactly once"
    );
    assert!(
        helper_body.contains("request("),
        "toggle_module must call GraphQL request path"
    );
    assert!(
        helper_body.contains("let response: ToggleModuleResponse"),
        "toggle_module must decode into typed ToggleModuleResponse before returning payload"
    );
    assert!(
        helper_body.contains("ToggleModuleVariables"),
        "toggle_module must use typed ToggleModuleVariables payload"
    );
    assert!(
        helper_body.contains("ToggleModuleVariables {"),
        "toggle_module must construct ToggleModuleVariables struct literal"
    );
    assert!(
        helper_body.contains("module_slug,"),
        "toggle_module must forward module_slug into ToggleModuleVariables payload"
    );
    assert!(
        helper_body.contains("enabled,"),
        "toggle_module must forward enabled flag into ToggleModuleVariables payload"
    );
    assert!(
        helper_body.contains("Ok(response.toggle_module)"),
        "toggle_module must return GraphQL toggle payload directly without native fallback mapping"
    );
    assert!(
        !helper_body.contains(".map_err("),
        "toggle_module must not remap canonical GraphQL ApiError taxonomy"
    );
    assert!(
        !helper_body.contains("ApiError::"),
        "toggle_module must not synthesize local ApiError variants and must preserve server taxonomy"
    );
}

#[test]
fn toggle_module_helper_forwards_auth_context_without_local_overrides() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    assert_eq!(
        helper_body.matches("request(").count(),
        1,
        "toggle_module must perform exactly one GraphQL request call"
    );
    assert!(
        helper_body.contains("token,"),
        "toggle_module must forward token to canonical GraphQL request"
    );
    assert!(
        helper_body.contains("tenant_slug,"),
        "toggle_module must forward tenant_slug to canonical GraphQL request"
    );
    assert!(
        !helper_body.contains("Some("),
        "toggle_module must not locally override auth context when forwarding request"
    );
    assert!(
        !helper_body.contains("None"),
        "toggle_module must not locally null auth context when forwarding request"
    );
}


#[test]
fn toggle_module_helper_does_not_parse_journal_metadata_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    for forbidden in [
        "module_operations",
        "correlation_id",
        "requested_by",
        "requested_enabled",
        "previous_effective_enabled",
        "ModuleOperationStatus",
    ] {
        assert!(
            !helper_body.contains(forbidden),
            "toggle_module helper must not parse journal metadata fragment `{forbidden}`"
        );
    }
}

#[test]
fn toggle_module_helper_does_not_cross_wire_other_mutation_contracts() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    for forbidden in [
        "INSTALL_MODULE_MUTATION",
        "UNINSTALL_MODULE_MUTATION",
        "UPGRADE_MODULE_MUTATION",
    ] {
        assert!(
            !helper_body.contains(forbidden),
            "toggle_module helper must not reference foreign mutation constant {forbidden}"
        );
    }
}


#[test]
fn toggle_module_helper_does_not_branch_on_runtime_error_taxonomy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    for forbidden in [
        "UNKNOWN_MODULE",
        "CORE_MODULE",
        "MISSING_DEPENDENCIES",
        "HAS_DEPENDENTS",
        "MODULE_HOOK_FAILED",
        "extensions.code",
        "reason_code",
    ] {
        assert!(
            !helper_body.contains(forbidden),
            "toggle_module helper must not branch on runtime taxonomy fragment `{forbidden}`"
        );
    }
}

#[test]
fn toggle_module_mutation_contract_shape_stays_stable() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let declaration = "pub const TOGGLE_MODULE_MUTATION: &str = \"";
    let start = content
        .find(declaration)
        .expect("TOGGLE_MODULE_MUTATION declaration not found");
    let rest = &content[start + declaration.len()..];
    let end = rest
        .find("\";")
        .expect("TOGGLE_MODULE_MUTATION declaration terminator not found");
    let mutation = &rest[..end];

    for required_fragment in [
        "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!)",
        "toggleModule(moduleSlug: $moduleSlug, enabled: $enabled)",
        "moduleSlug",
        "enabled",
        "settings",
    ] {
        assert!(
            mutation.contains(required_fragment),
            "toggle mutation contract drifted: missing fragment `{required_fragment}`"
        );
    }

    for forbidden_fragment in [
        "$module_slug",
        "module_slug:",
        "toggleModule(moduleSlug: $module_slug",
    ] {
        assert!(
            !mutation.contains(forbidden_fragment),
            "toggle mutation contract must keep canonical camelCase variable naming, found forbidden fragment `{forbidden_fragment}`"
        );
    }

    assert!(
        !mutation.contains("__typename"),
        "toggle mutation contract must stay minimal and not introduce opaque response-only fields"
    );
}

#[test]
fn toggle_module_mutation_constant_is_declared_once() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let occurrences = content
        .matches("pub const TOGGLE_MODULE_MUTATION: &str =")
        .count();
    assert_eq!(
        occurrences, 1,
        "Expected exactly one TOGGLE_MODULE_MUTATION declaration, found {occurrences}"
    );
}

#[test]
fn toggle_module_helper_uses_only_toggle_response_field() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    assert!(
        helper_body.contains("Ok(response.toggle_module)"),
        "toggle_module helper must return toggle_module field from typed response"
    );
    for forbidden in [
        "response.install_module",
        "response.uninstall_module",
        "response.upgrade_module",
        "response.update_module_settings",
    ] {
        assert!(
            !helper_body.contains(forbidden),
            "toggle_module helper must not return foreign response field `{forbidden}`"
        );
    }

    assert_eq!(
        helper_body.matches("response.toggle_module").count(),
        1,
        "toggle_module helper must read toggle response field exactly once"
    );
}

#[test]
fn toggle_module_helper_uses_only_canonical_toggle_variable_names() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let helper_body = extract_function_block(&content, "pub async fn toggle_module(")
        .expect("toggle_module helper signature not found");

    for required in ["module_slug,", "enabled,"] {
        assert!(
            helper_body.contains(required),
            "toggle_module helper must forward canonical variable `{required}` in ToggleModuleVariables"
        );
    }

    for forbidden in ["moduleSlug:", "enabled:", "module_slug:", "$moduleSlug", "$enabled"] {
        assert!(
            !helper_body.contains(forbidden),
            "toggle_module helper must stay on Rust-side variable wiring and not contain forbidden fragment `{forbidden}`"
        );
    }
}

fn assert_graphql_only_helper(
    content: &str,
    signature: &str,
    mutation_name: &str,
    variables_literal: &str,
    forwarded_fields: &[&str],
    return_expr: &str,
) {
    let helper_body = extract_function_block(content, signature)
        .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

    assert!(
        helper_body.contains(mutation_name),
        "expected helper {signature} to call canonical mutation {mutation_name}"
    );
    assert!(
        helper_body.contains("request("),
        "expected helper {signature} to call GraphQL request path"
    );
    assert!(
        helper_body.contains(variables_literal),
        "expected helper {signature} to construct typed GraphQL variables payload"
    );
    assert!(
        helper_body.contains(return_expr),
        "expected helper {signature} to return GraphQL payload directly"
    );
    for field in forwarded_fields {
        assert!(
            helper_body.contains(field),
            "expected helper {signature} to forward field `{field}` into typed GraphQL payload"
        );
    }
    assert!(
        !helper_body.contains("combine_native_and_graphql_error"),
        "helper {signature} must not compose native/graphql fallback errors"
    );
}

#[test]
fn toggle_module_helper_signature_is_unique() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let signature = "pub async fn toggle_module(";
    let occurrences = content.matches(signature).count();
    assert_eq!(
        occurrences, 1,
        "Expected exactly one toggle_module helper signature, found {occurrences}"
    );
}

#[test]
fn module_composition_helper_signatures_are_unique() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
    ] {
        let occurrences = content.matches(signature).count();
        assert_eq!(
            occurrences, 1,
            "Expected exactly one `{signature}` helper signature, found {occurrences}"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_call_toggle_mutation_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for helper in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
    ] {
        let helper_body = extract_function_block(&content, helper)
            .unwrap_or_else(|| panic!("helper signature not found: {helper}"));

        assert!(
            !helper_body.contains("TOGGLE_MODULE_MUTATION"),
            "module composition helper must not accidentally call toggle mutation contract: {helper}"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_cross_wire_mutation_constants() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            ["UNINSTALL_MODULE_MUTATION", "UPGRADE_MODULE_MUTATION"],
        ),
        (
            "pub async fn uninstall_module(",
            ["INSTALL_MODULE_MUTATION", "UPGRADE_MODULE_MUTATION"],
        ),
        (
            "pub async fn upgrade_module(",
            ["INSTALL_MODULE_MUTATION", "UNINSTALL_MODULE_MUTATION"],
        ),
    ];

    for (signature, forbidden_mutations) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for forbidden in forbidden_mutations {
            assert!(
                !helper_body.contains(forbidden),
                "helper {signature} must not reference foreign mutation constant {forbidden}"
            );
        }
    }
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
pub async fn toggle_module() -> Result<(), ()> {
    if true {
        let nested = || {
            let map = std::collections::BTreeMap::<String, String>::new();
            map
        };
        let _ = nested();
    }
    Ok(())
}

pub async fn other_helper() {}
"#;

    let extracted = extract_function_block(source, "pub async fn toggle_module()")
        .expect("toggle_module function should be extracted");
    assert!(extracted.contains("BTreeMap::<String, String>::new()"));
    assert!(extracted.trim_end().ends_with('}'));
    assert!(!extracted.contains("pub async fn other_helper()"));
}

#[test]
fn extract_function_block_returns_none_when_signature_missing() {
    let source = "pub async fn other_helper() {}";
    assert!(extract_function_block(source, "pub async fn toggle_module(").is_none());
}

#[test]
fn extract_function_block_returns_none_when_braces_are_unbalanced() {
    let source = r#"
pub async fn toggle_module() {
    if true {
        let _x = 1;
}
"#;
    assert!(extract_function_block(source, "pub async fn toggle_module()").is_none());
}

#[test]
fn extract_function_block_returns_none_when_body_brace_missing() {
    let source = "pub async fn toggle_module() -> Result<(), ()>";
    assert!(extract_function_block(source, "pub async fn toggle_module()").is_none());
}

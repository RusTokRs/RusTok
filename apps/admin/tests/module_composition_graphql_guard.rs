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
    assert!(
        helper_body.contains("request("),
        "toggle_module must call GraphQL request path"
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

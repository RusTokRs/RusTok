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

    for helper in ["pub async fn install_module(", "pub async fn uninstall_module(", "pub async fn upgrade_module("] {
        let helper_start = content
            .find(helper)
            .unwrap_or_else(|| panic!("helper signature not found: {helper}"));
        let helper_body = &content[helper_start..content.len().min(helper_start + 900)];

        assert!(
            !helper_body.contains("combine_native_and_graphql_error"),
            "Forbidden native/graphql fallback combiner found in module composition helper: {helper}"
        );
    }
}

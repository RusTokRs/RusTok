const SUPPORT: &str = include_str!("../src/transport/native_server_adapter/support.rs");
const READ_BOOTSTRAP: &str =
    include_str!("../src/transport/native_server_adapter/read_bootstrap.rs");
const READ_DIAGNOSTICS: &str =
    include_str!("../src/transport/native_server_adapter/read_diagnostics.rs");
const READ_ANALYTICS: &str =
    include_str!("../src/transport/native_server_adapter/read_analytics.rs");
const WRITE_RUNTIME: &str = include_str!("../src/transport/native_server_adapter/write_runtime.rs");
const WRITE_DICTIONARY: &str =
    include_str!("../src/transport/native_server_adapter/write_dictionary.rs");

fn endpoint_source() -> String {
    [
        READ_BOOTSTRAP,
        READ_DIAGNOSTICS,
        READ_ANALYTICS,
        WRITE_RUNTIME,
        WRITE_DICTIONARY,
    ]
    .join("\n")
}

#[test]
fn every_authenticated_search_admin_endpoint_uses_the_scoped_permission_helpers() {
    let source = endpoint_source();
    let read_calls = source
        .matches("ensure_settings_read_permission(&auth, tenant.id)?;")
        .count();
    let manage_calls = source
        .matches("ensure_settings_manage_permission(&auth, tenant.id)?;")
        .count();
    let auth_extracts = source
        .matches("leptos_axum::extract::<AuthContext>()")
        .count();
    let tenant_extracts = source
        .matches("leptos_axum::extract::<TenantContext>()")
        .count();

    assert_eq!(
        read_calls, 7,
        "all Search Admin read endpoints must be scoped"
    );
    assert_eq!(
        manage_calls, 8,
        "all Search Admin manage endpoints must be scoped"
    );
    assert_eq!(
        auth_extracts,
        read_calls + manage_calls,
        "every authenticated endpoint must apply exactly one scoped permission helper"
    );
    assert_eq!(
        tenant_extracts,
        auth_extracts + 1,
        "track-click is the single tenant-scoped endpoint without AuthContext"
    );
    assert!(!source.contains("ensure_settings_read_permission(&auth.permissions)?;"));
    assert!(!source.contains("ensure_settings_manage_permission(&auth.permissions)?;"));
}

#[test]
fn tenant_equality_precedes_read_and_manage_permission_admission() {
    let tenant_scope = SUPPORT
        .find("fn ensure_search_admin_tenant_scope(")
        .expect("shared Search Admin tenant guard must exist");
    let read_helper = SUPPORT
        .find("fn ensure_settings_read_permission(")
        .expect("read permission helper must exist");
    let manage_helper = SUPPORT
        .find("fn ensure_settings_manage_permission(")
        .expect("manage permission helper must exist");
    let parse_helper = SUPPORT
        .find("fn parse_required_uuid(")
        .expect("permission helper section must remain bounded");

    let tenant_scope_source = &SUPPORT[tenant_scope..read_helper];
    let read_source = &SUPPORT[read_helper..manage_helper];
    let manage_source = &SUPPORT[manage_helper..parse_helper];

    assert!(tenant_scope_source.contains("if auth.tenant_id == resolved_tenant_id"));
    assert!(tenant_scope_source.contains("Search admin access is denied"));
    assert!(tenant_scope_source.contains("search.admin_tenant_scope_mismatch"));
    assert!(tenant_scope_source.contains("auth_tenant_id = %auth.tenant_id"));
    assert!(tenant_scope_source.contains("resolved_tenant_id = %resolved_tenant_id"));

    for helper in [read_source, manage_source] {
        let scope_check = helper
            .find("ensure_search_admin_tenant_scope(auth, resolved_tenant_id)?;")
            .expect("tenant scope must be enforced");
        let permission_check = helper
            .find("has_effective_permission(")
            .expect("permission admission must remain");
        assert!(
            scope_check < permission_check,
            "tenant equality must precede permission admission"
        );
    }
}

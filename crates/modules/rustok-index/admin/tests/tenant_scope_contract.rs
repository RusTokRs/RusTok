const SOURCE: &str = include_str!("../src/transport/native_server_adapter.rs");

#[test]
fn index_admin_binds_authenticated_authority_to_the_resolved_tenant() {
    let tenant_guard = SOURCE
        .find("require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;")
        .expect("index admin must bind authenticated and resolved tenants");
    let permission_check = SOURCE
        .find("has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)")
        .expect("index admin must retain SETTINGS_READ admission");

    assert!(
        tenant_guard < permission_check,
        "tenant equality must be enforced before permission admission"
    );
    assert_eq!(
        SOURCE
            .matches("leptos_axum::extract::<AuthContext>()")
            .count(),
        1
    );
    assert_eq!(
        SOURCE
            .matches("leptos_axum::extract::<TenantContext>()")
            .count(),
        1
    );
    assert!(SOURCE.contains("if auth_tenant_id == resolved_tenant_id"));
    assert!(SOURCE.contains("Index admin access is denied"));
    assert!(SOURCE.contains("index.admin_tenant_scope_mismatch"));
    assert!(SOURCE.contains("auth_tenant_id = %auth_tenant_id"));
    assert!(SOURCE.contains("resolved_tenant_id = %resolved_tenant_id"));
}

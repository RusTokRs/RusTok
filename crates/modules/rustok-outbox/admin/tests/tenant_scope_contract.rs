const SOURCE: &str = include_str!("../src/transport/native_server_adapter.rs");

#[test]
fn outbox_admin_binds_logs_authority_to_the_resolved_tenant() {
    let tenant_guard = SOURCE
        .find("require_outbox_admin_tenant_scope(auth.tenant_id, tenant.id)?;")
        .expect("Outbox Admin must bind authenticated and resolved tenants");
    let permission_check = SOURCE
        .find("has_effective_permission(&auth.permissions, &Permission::LOGS_READ)")
        .expect("Outbox Admin must retain LOGS_READ admission");

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
            .matches("leptos_axum::extract::<OptionalTenant>()")
            .count(),
        1
    );
    assert!(SOURCE.contains("if auth_tenant_id == resolved_tenant_id"));
    assert!(SOURCE.contains("Outbox admin access is denied"));
    assert!(SOURCE.contains("outbox.admin_tenant_scope_mismatch"));
    assert!(SOURCE.contains("auth_tenant_id = %auth_tenant_id"));
    assert!(SOURCE.contains("resolved_tenant_id = %resolved_tenant_id"));
    assert!(SOURCE.contains("boundary = \"outbox_admin_native_transport\""));
}

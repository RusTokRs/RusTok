const SOURCE: &str = include_str!("../src/transport/native_server_adapter.rs");

#[test]
fn every_channel_admin_permission_check_is_bound_to_the_resolved_tenant() {
    let scoped_calls = SOURCE
        .matches("ensure_manage_permission(&auth, tenant.id)?;")
        .count();
    let auth_extracts = SOURCE
        .matches("leptos_axum::extract::<AuthContext>()")
        .count();
    let tenant_extracts = SOURCE
        .matches("leptos_axum::extract::<TenantContext>()")
        .count();

    assert_eq!(
        scoped_calls, 16,
        "all native channel admin endpoints must use the scoped guard"
    );
    assert_eq!(
        auth_extracts, scoped_calls,
        "every authenticated endpoint must apply the guard"
    );
    assert_eq!(
        tenant_extracts, scoped_calls,
        "every routed tenant must be bound to authenticated authority"
    );
    assert!(!SOURCE.contains("ensure_manage_permission(&auth.permissions)?;"));
}

#[test]
fn tenant_equality_is_checked_before_permission_admission() {
    let helper_start = SOURCE
        .find("fn ensure_manage_permission(")
        .expect("scoped permission helper must exist");
    let helper_end = SOURCE[helper_start..]
        .find("\n}\n\n#[cfg(feature = \"ssr\")]\nfn parse_uuid")
        .map(|offset| helper_start + offset)
        .expect("scoped permission helper must remain isolated");
    let helper = &SOURCE[helper_start..helper_end];

    let tenant_check = helper
        .find("if auth.tenant_id != resolved_tenant_id")
        .expect("tenant equality must be checked");
    let permission_check = helper
        .find("has_any_effective_permission(")
        .expect("permission admission must remain");

    assert!(
        tenant_check < permission_check,
        "tenant equality must precede permission admission"
    );
    assert!(helper.contains("Channel admin access is denied"));
    assert!(helper.contains("channel.admin_tenant_scope_mismatch"));
    assert!(helper.contains("auth_tenant_id = %auth.tenant_id"));
    assert!(helper.contains("resolved_tenant_id = %resolved_tenant_id"));
}

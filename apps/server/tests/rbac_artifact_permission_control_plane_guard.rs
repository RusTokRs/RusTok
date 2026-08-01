use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn artifact_role_permission_routes_require_owner_direct_principal_admission_first() {
    let controller = source("apps/server/src/controllers/artifact_permissions.rs");

    assert!(controller.contains("AuthPrincipalContext"));
    assert!(!controller.contains("extractors::{auth::CurrentUser"));
    assert_eq!(controller.matches("auth: AuthContext,").count(), 2);
    assert_eq!(
        controller
            .matches("principal_context: AuthPrincipalContext,")
            .count(),
        2
    );
    assert_eq!(
        controller
            .matches("ensure_artifact_permission_control_plane(&auth, principal_context, tenant.id)?;")
            .count(),
        2
    );
    assert_eq!(
        controller
            .matches("assign(&ctx, tenant.id, auth.user_id")
            .count(),
        2
    );

    let helper_start = controller
        .find("fn ensure_artifact_permission_control_plane")
        .expect("artifact control-plane helper must exist");
    let permission_start = controller[helper_start..]
        .find("fn ensure_modules_manage")
        .map(|offset| helper_start + offset)
        .expect("permission helper must follow control-plane helper");
    let helper = &controller[helper_start..permission_start];
    assert!(helper.contains("RbacControlPlanePrincipal"));
    assert!(helper.contains("principal_kind: principal_context.kind"));
    let principal_guard = helper
        .find("require_direct_control_plane_user(principal, tenant_id)")
        .expect("owner typed principal admission must be applied");
    let permission_guard = helper
        .find("ensure_modules_manage(&auth.permissions)")
        .expect("modules:manage must be checked after principal admission");
    assert!(principal_guard < permission_guard);
    assert!(!helper.contains("auth.client_id"));
    assert!(!helper.contains("auth.grant_type"));
    assert!(!helper.contains("auth.session_id"));
}

#[test]
fn native_admin_bootstrap_uses_the_same_owner_principal_policy() {
    let native = source("crates/rustok-rbac/admin/src/transport/native_server_adapter.rs");

    for required in [
        "AuthPrincipalContext",
        "leptos_axum::extract::<AuthPrincipalContext>()",
        "RbacControlPlanePrincipal",
        "tenant_id: auth.tenant_id",
        "principal_kind: principal_context.kind",
        "require_direct_control_plane_user(principal, tenant.id)",
        "code = \"rbac.admin_control_plane_denied\"",
        "ServerFnError::new(\"RBAC admin access is denied\")",
    ] {
        assert!(native.contains(required), "native adapter must retain {required}");
    }

    let principal_guard = native
        .find("require_direct_control_plane_user(principal, tenant.id)")
        .expect("native admin must use owner principal admission");
    let permission_guard = native
        .find("Permission::SETTINGS_READ")
        .expect("native admin must retain settings:read admission");
    assert!(principal_guard < permission_guard);
    assert!(!native.contains("fn require_rbac_admin_tenant_scope<T>("));
    assert!(!native.contains("fn rbac_admin_scope_matches<T: PartialEq>("));
    assert!(!native.contains("session_id: auth.session_id"));
    assert!(!native.contains("client_id: auth.client_id"));
    assert!(!native.contains("grant_type: &auth.grant_type"));
}

#[test]
fn module_owned_control_plane_guard_denies_delegated_service_and_cross_tenant_principals() {
    let owner = source("crates/rustok-rbac/src/control_plane.rs");
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let graphql = source("crates/rustok-rbac/src/graphql/control_plane.rs");

    for required in [
        "pub struct RbacControlPlanePrincipal",
        "pub principal_kind: AuthPrincipalKind",
        "!principal.principal_kind.is_direct_user()",
        "principal.tenant_id != tenant_id",
        "delegated_and_service_principals_are_denied_even_with_management_permission",
        "cross_tenant_context_is_denied",
    ] {
        assert!(owner.contains(required), "owner guard must retain {required}");
    }

    for forbidden in [
        "principal.client_id",
        "principal.grant_type",
        "principal.session_id",
    ] {
        assert!(!owner.contains(forbidden), "owner must not infer from {forbidden}");
    }

    assert!(exports.contains("RbacControlPlanePrincipal"));
    assert!(exports.contains("require_direct_control_plane_user"));
    assert!(graphql.contains("AuthPrincipalContext"));
    assert!(graphql.contains("principal_kind: principal_context.kind"));
    assert!(graphql.contains("crate::require_direct_control_plane_user(principal, tenant_id)"));
}

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
fn artifact_role_permission_routes_require_owner_direct_session_admission_first() {
    let controller = source("apps/server/src/controllers/artifact_permissions.rs");

    assert!(controller.contains("use rustok_api::{AuthContext, Permission"));
    assert!(!controller.contains("extractors::{auth::CurrentUser"));
    assert_eq!(controller.matches("auth: AuthContext,").count(), 2);
    assert_eq!(
        controller
            .matches("ensure_artifact_permission_control_plane(&auth, tenant.id)?;")
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
    let principal_guard = helper
        .find("require_direct_control_plane_user(principal, tenant_id)")
        .expect("owner direct-session admission must be applied");
    let permission_guard = helper
        .find("ensure_modules_manage(&auth.permissions)")
        .expect("modules:manage must be checked after principal admission");
    assert!(principal_guard < permission_guard);
}

#[test]
fn native_admin_bootstrap_uses_the_same_owner_principal_policy() {
    let native = source("crates/rustok-rbac/admin/src/transport/native_server_adapter.rs");

    for required in [
        "RbacControlPlanePrincipal",
        "tenant_id: auth.tenant_id",
        "session_id: auth.session_id",
        "client_id: auth.client_id",
        "grant_type: &auth.grant_type",
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
}

#[test]
fn module_owned_control_plane_guard_denies_oauth_and_cross_tenant_principals() {
    let owner = source("crates/rustok-rbac/src/control_plane.rs");
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let graphql = source("crates/rustok-rbac/src/graphql/control_plane.rs");

    for required in [
        "pub struct RbacControlPlanePrincipal",
        "principal.client_id.is_some()",
        "principal.grant_type != \"direct\"",
        "principal.session_id.is_nil()",
        "principal.tenant_id != tenant_id",
        "oauth_principals_are_denied_even_with_management_permission",
        "cross_tenant_context_is_denied",
    ] {
        assert!(owner.contains(required), "owner guard must retain {required}");
    }

    assert!(exports.contains("RbacControlPlanePrincipal"));
    assert!(exports.contains("require_direct_control_plane_user"));
    assert!(graphql.contains("crate::RbacControlPlanePrincipal"));
    assert!(graphql.contains("crate::require_direct_control_plane_user(principal, tenant_id)"));
}

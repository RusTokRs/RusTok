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
fn permission_resolver_is_read_only() {
    let contract = source("crates/rustok-rbac/src/services/permission_resolver.rs");

    for forbidden in [
        "async fn assign_role_permissions(",
        "async fn replace_user_role(",
        "async fn remove_tenant_role_assignments(",
        "async fn remove_user_role_assignment(",
    ] {
        assert!(
            !contract.contains(forbidden),
            "PermissionResolver must not expose mutation method {forbidden}"
        );
    }

    assert!(contract.contains("Read-only owner contract"));
    assert!(contract.contains("transaction-owned or committed RBAC mutation"));
}

#[test]
fn permission_resolver_test_doubles_are_read_only() {
    let authorizer = source("crates/rustok-rbac/src/services/permission_authorizer.rs");

    for forbidden in [
        "async fn assign_role_permissions(",
        "async fn replace_user_role(",
        "async fn remove_tenant_role_assignments(",
        "async fn remove_user_role_assignment(",
    ] {
        assert!(
            !authorizer.contains(forbidden),
            "permission resolver test doubles must not retain removed mutation method {forbidden}"
        );
    }

    assert!(
        !authorizer.contains("use rustok_core::UserRole;"),
        "permission resolver test doubles must not depend on role mutation payloads"
    );
}

#[test]
fn runtime_permission_resolver_has_no_mutation_composition_surface() {
    let runtime = source("crates/rustok-rbac/src/services/runtime_permission_resolver.rs");
    let impl_start = runtime
        .find("impl<S, C, E> PermissionResolver for RuntimePermissionResolver")
        .expect("runtime resolver PermissionResolver implementation must exist");
    let impl_end = runtime[impl_start..]
        .find("#[cfg(test)]")
        .map(|offset| impl_start + offset)
        .expect("runtime resolver implementation must end before tests");
    let resolver_impl = &runtime[impl_start..impl_end];

    assert!(resolver_impl.contains("resolve_permissions_with_cache"));
    for forbidden in [
        "RoleAssignmentStore",
        "assignment_store",
        "assign_role_permissions",
        "replace_user_role",
        "remove_tenant_role_assignments",
        "remove_user_role_assignment",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "runtime permission resolver must not retain mutation composition: {forbidden}"
        );
    }
}

#[test]
fn server_runtime_does_not_reintroduce_assignment_store_adapter() {
    let runtime = source("apps/server/src/services/rbac_runtime.rs");

    for forbidden in [
        "RoleAssignmentStore",
        "ServerRoleAssignmentStore",
        "remove_tenant_role_assignments_via_store",
        "remove_user_role_assignment_via_store",
        "replace_user_role_via_store",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "server permission runtime must not retain obsolete mutation adapter: {forbidden}"
        );
    }

    assert!(runtime.contains("RuntimePermissionResolver<SeaOrmRelationPermissionStore"));
    assert!(runtime.contains("RuntimePermissionResolver::new("));
}

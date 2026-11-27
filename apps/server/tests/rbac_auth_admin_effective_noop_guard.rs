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

fn update_user_block(admin: &str) -> &str {
    let start = admin
        .find("    async fn update_user(")
        .expect("Auth admin update_user must exist");
    let end = admin[start..]
        .find("    async fn delete_user(")
        .map(|offset| start + offset)
        .expect("Auth admin update_user must end before delete_user");
    &admin[start..end]
}

#[test]
fn owner_policy_retains_exact_noop_and_malformed_repair() {
    let policy = source("crates/modules/rustok-rbac/src/role_mutation.rs");

    for required in [
        "RbacRoleMutationOutcome::Noop",
        "RbacRoleMutationChange::AssignmentRepaired",
        "facts.assignment_is_exact && facts.target_role == facts.requested_role",
        "exact_same_role_is_noop_but_malformed_same_role_is_repair",
    ] {
        assert!(
            policy.contains(required),
            "owner policy must retain {required}"
        );
    }
}

#[test]
fn auth_admin_status_and_row_writes_follow_effective_change() {
    let admin = source("apps/server/src/services/auth_admin_mutation_provider/user_admin.rs");
    let update = update_user_block(&admin);

    for required in [
        "status_change_requested(&target_status, requested_status.as_ref())",
        "let user_row_update_requested = command.email.is_some()",
        "let user = if user_row_update_requested",
        "RbacRoleMutationOutcome::Noop => None",
        "let status_disables_user = status_changed",
        "let invalidates_authorization = role_mutation_plan.is_some() || status_changed;",
        "status_effective_change_ignores_exact_replay",
    ] {
        assert!(
            update.contains(required) || admin.contains(required),
            "effective-change path must retain {required}"
        );
    }

    for forbidden in [
        "let invalidates_authorization = role_mutation_plan.is_some() || requested_status.is_some();",
        "let status_disables_user = requested_status",
        "let user = active\n            .update(&tx)",
    ] {
        assert!(
            !update.contains(forbidden),
            "presence-based or unconditional mutation returned: {forbidden}"
        );
    }

    let lock = update
        .find("let locked_user = lock_user_for_mutation")
        .expect("target row must be locked");
    let status = update
        .find("status_change_requested(&target_status, requested_status.as_ref())")
        .expect("status must compare locked state");
    let plan = update
        .find("let role_mutation_plan: Option<RolePlan>")
        .expect("owner role plan must be used");
    let row_update = update
        .find("let user = if user_row_update_requested")
        .expect("user row update must be conditional");
    let invalidation = update
        .find("let invalidates_authorization = role_mutation_plan.is_some() || status_changed;")
        .expect("effective invalidation decision must exist");
    let flush = update
        .find("flush_rbac_static_relation_generation(&tx)")
        .expect("effective role change must flush its relation generation");
    let reserve = update
        .find("reserve_rbac_invalidation_generation(&tx)")
        .expect("status-only change must reserve a generation");

    assert!(lock < status);
    assert!(status < plan);
    assert!(plan < row_update);
    assert!(row_update < invalidation);
    assert!(invalidation < flush && flush < reserve);
    let response_role = update.find("let final_role = match role_mutation_plan.as_ref()")
        .expect("response role must be resolved");
    let response_tenant = update.find("let tenant_name = self.tenant_name(&tx, context.tenant_id)")
        .expect("response tenant must be resolved");
    let deadline = update.rfind("self.ensure_user_admin_credential_deadline(&tx, credential_deadline)")
        .expect("credential deadline must be checked at commit");
    assert!(response_role < flush && response_tenant < flush);
    assert!(reserve < deadline);
}

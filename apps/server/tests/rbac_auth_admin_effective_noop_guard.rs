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
    let policy = source("crates/rustok-rbac/src/role_mutation.rs");

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
        .find("let role_mutation_plan: Option<RbacRoleMutationPlan>")
        .expect("owner role plan must be used");
    let row_update = update
        .find("let user = if user_row_update_requested")
        .expect("user row update must be conditional");
    let invalidation = update
        .find("let invalidates_authorization = role_mutation_plan.is_some() || status_changed;")
        .expect("effective invalidation decision must exist");
    let reserve = update
        .find("reserve_rbac_invalidation_generation(&tx)")
        .expect("effective change must reserve a generation");

    assert!(lock < status);
    assert!(status < plan);
    assert!(plan < row_update);
    assert!(row_update < invalidation);
    assert!(invalidation < reserve);
}

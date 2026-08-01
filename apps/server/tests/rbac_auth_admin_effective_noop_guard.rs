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
fn transaction_role_writer_reports_effective_change_without_hiding_relation_repair() {
    let committed = source("apps/server/src/services/rbac_committed_mutations.rs");

    for required in [
        "replace_user_role_in_transaction_if_changed",
        "if has_exact_tenant_role_assignment(db, user_id, tenant_id, &role).await?",
        "return Ok(false);",
        "Self::replace_user_role_in_transaction(db, user_id, tenant_id, role).await?;",
        "Ok(true)",
        "transaction_role_replacement_reports_exact_noop",
        "transaction_role_replacement_repairs_multiple_assignments",
    ] {
        assert!(
            committed.contains(required),
            "transaction role writer must retain {required}"
        );
    }

    let exact = committed
        .find("if has_exact_tenant_role_assignment(db, user_id, tenant_id, &role).await?")
        .expect("exact relation check must exist");
    let mutation = committed
        .find("Self::replace_user_role_in_transaction(db, user_id, tenant_id, role).await?")
        .expect("relation repair must exist");
    assert!(exact < mutation, "exact no-op check must precede relation repair");
}

#[test]
fn auth_admin_reserves_generation_only_for_effective_role_or_status_change() {
    let admin = source("apps/server/src/services/auth_admin_mutation_provider/user_admin.rs");

    for required in [
        "status_change_requested(&locked_user.status, requested_status.as_ref())",
        "let user_row_update_requested = command.email.is_some()",
        "let user = if user_row_update_requested",
        "replace_user_role_in_transaction_if_changed",
        "let invalidates_authorization = role_assignment_changed || status_changed;",
        "let status_disables_user = status_changed",
        "status_effective_change_ignores_exact_replay",
    ] {
        assert!(
            admin.contains(required),
            "Auth admin effective-change path must retain {required}"
        );
    }

    for forbidden in [
        "let invalidates_authorization = requested_role.is_some() || requested_status.is_some();",
        "RbacService::replace_user_role_in_transaction(&tx, &user.id, &context.tenant_id, role)",
        "let user = active\n            .update(&tx)",
    ] {
        assert!(
            !admin.contains(forbidden),
            "Auth admin path must not restore presence-based or unconditional mutation marker {forbidden}"
        );
    }

    let locked = admin
        .find("let locked_user = lock_user_for_mutation")
        .expect("target row must be locked");
    let status = admin
        .find("status_change_requested(&locked_user.status, requested_status.as_ref())")
        .expect("status comparison must use locked state");
    let row_update = admin
        .find("let user = if user_row_update_requested")
        .expect("user row update must be conditional");
    let role = admin
        .find("let role_assignment_changed = if let Some(role) = requested_role")
        .expect("role change result must be captured");
    let invalidation = admin
        .find("let invalidates_authorization = role_assignment_changed || status_changed;")
        .expect("effective invalidation decision must exist");
    let reserve = admin
        .find("reserve_rbac_invalidation_generation(&tx)")
        .expect("effective authorization change must reserve a generation");

    assert!(locked < status);
    assert!(status < row_update);
    assert!(row_update < role);
    assert!(role < invalidation);
    assert!(invalidation < reserve);
}

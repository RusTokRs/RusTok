use std::path::{Path, PathBuf};

const ROLE_REPLACEMENT_CALL: &str = "RbacService::replace_user_role(";
const AUTH_LIFECYCLE_PATH: &str = "services/auth_lifecycle.rs";

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

fn rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn production_source<'a>(path: &Path, content: &'a str) -> &'a str {
    if !path.ends_with(AUTH_LIFECYCLE_PATH) {
        return content;
    }

    let tests_module = content
        .find("mod tests {")
        .expect("auth_lifecycle.rs must retain its cfg(test) module");
    let cfg_test = content[..tests_module]
        .rfind("#[cfg(test)]")
        .expect("auth_lifecycle.rs tests module must retain cfg(test)");
    assert!(
        content[cfg_test + "#[cfg(test)]".len()..tests_module]
            .trim()
            .is_empty(),
        "auth_lifecycle.rs tests module must remain directly guarded by cfg(test)"
    );

    &content[..cfg_test]
}

#[test]
fn initial_role_assignment_is_private_and_confined_to_new_user_creation() {
    let service = source("apps/server/src/services/rbac_service.rs");
    assert!(service.contains("pub(crate) async fn replace_user_role("));
    assert!(!service.contains("    pub async fn replace_user_role("));
    assert!(service.contains("initialize_user_role"));
    assert!(!service.contains("replace_user_role_legacy_transaction"));

    let server_src = repo_root().join("apps/server/src");
    let mut files = Vec::new();
    rust_sources(&server_src, &mut files);
    let mut call_sites = Vec::new();
    for path in files {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for _ in production_source(&path, &content).match_indices(ROLE_REPLACEMENT_CALL) {
            call_sites.push(path.clone());
        }
    }

    assert_eq!(
        call_sites.len(),
        1,
        "new-user role initialization must have one production call site"
    );
    assert!(call_sites[0].ends_with(AUTH_LIFECYCLE_PATH));

    let lifecycle = source("apps/server/src/services/auth_lifecycle.rs");
    let create_start = lifecycle
        .find("pub(crate) async fn create_user_in_tx")
        .expect("transactional user creation must exist");
    let create_end = lifecycle[create_start..]
        .find("pub async fn update_profile_runtime")
        .map(|offset| create_start + offset)
        .expect("user creation block must end before profile update");
    let create_block = &lifecycle[create_start..create_end];
    assert_eq!(create_block.matches(ROLE_REPLACEMENT_CALL).count(), 1);
}

#[test]
fn production_call_scan_excludes_only_auth_lifecycle_test_module() {
    let source = r#"
fn production() {
    RbacService::replace_user_role(...);
}

#[cfg(test)]
fn test_helper() {
    helper();
}

#[cfg(test)]
mod tests {
    fn first() {
        RbacService::replace_user_role(...);
    }

    fn second() {
        RbacService::replace_user_role(...);
    }
}
"#;

    let auth_lifecycle = Path::new("/tmp/services/auth_lifecycle.rs");
    let production = production_source(auth_lifecycle, source);
    assert_eq!(production.matches(ROLE_REPLACEMENT_CALL).count(), 1);
    assert!(
        production.contains("fn test_helper()"),
        "production scan must exclude only the auth_lifecycle cfg(test) module"
    );

    let other_file = Path::new("/tmp/services/other.rs");
    assert_eq!(
        production_source(other_file, source)
            .matches(ROLE_REPLACEMENT_CALL)
            .count(),
        3,
        "test-module exclusion must remain narrowly scoped to auth_lifecycle.rs"
    );
}

#[test]
fn runtime_mutation_paths_use_explicit_transaction_or_committed_entrypoints() {
    let committed = source("apps/server/src/services/rbac_committed_mutations.rs");
    let admin = source("apps/server/src/services/auth_admin_mutation_provider/user_admin.rs");
    let superadmin = source("apps/server/src/initializers/superadmin.rs");

    assert!(committed.contains("replace_user_role_in_transaction"));
    assert!(admin.contains("RbacService::replace_user_role_in_transaction("));
    assert!(superadmin.contains("RbacService::replace_user_role_committed("));
    assert!(!admin.contains("RbacService::replace_user_role("));
    assert!(!superadmin.contains("RbacService::replace_user_role("));
}

#[test]
fn committed_role_replacement_locks_target_and_checks_noop_before_generation_bump() {
    let committed = source("apps/server/src/services/rbac_committed_mutations.rs");

    for required in [
        "lock_target_user_for_role_mutation",
        "query().lock_exclusive().one(db).await?",
        "Expr::col(users::Column::UpdatedAt)",
        "has_exact_tenant_role_assignment",
        "exact_single_role_replacement_is_a_generation_noop",
        "matching_role_among_multiple_assignments_is_not_treated_as_noop",
    ] {
        assert!(
            committed.contains(required),
            "committed role path must retain {required}"
        );
    }

    let lock = committed
        .find("let target = lock_target_user_for_role_mutation")
        .expect("target user must be locked");
    let noop = committed
        .find("if has_exact_tenant_role_assignment")
        .expect("exact role no-op must be checked");
    let reserve = committed
        .find("reserve_rbac_invalidation_generation(&tx)")
        .expect("real role change must reserve durable generation");
    assert!(lock < noop);
    assert!(noop < reserve);
}

#[test]
fn public_role_repair_surface_splits_read_only_plan_from_transactional_apply() {
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let server = source("apps/server/src/services/rbac_repair.rs");

    assert!(exports.contains("mod repair;"));
    assert!(!exports.contains("pub mod repair;"));
    assert!(exports.contains("pub async fn plan_system_role_repair("));
    assert!(exports.contains("pub async fn apply_system_role_repair_in_transaction("));
    assert!(exports.contains("db: &sea_orm::DatabaseTransaction"));
    assert!(!exports.contains("pub use repair::{\n    repair_system_roles"));
    assert!(server.contains("rustok_rbac::plan_system_role_repair(db, tenant_id)"));
    assert!(
        server.contains("rustok_rbac::apply_system_role_repair_in_transaction(&tx, tenant_id)")
    );
}

#[test]
fn operational_cli_applies_repair_and_generation_in_one_transaction() {
    let cli = source("crates/rustok-rbac/cli/src/lib.rs");
    let cargo = source("crates/rustok-rbac/cli/Cargo.toml");

    let required = "sea-orm.workspace = true";
    assert!(
        cargo.contains(required),
        "RBAC CLI manifest must retain {required}"
    );
    for required in [
        "apply_system_role_repair_in_transaction",
        "plan_system_role_repair",
        "reserve_permission_invalidation_generation",
        "let tx = db.begin().await",
        "tx.commit().await",
        "rollback_command_failure",
        "report.runtime_restart_required = false",
        "durable_generation",
    ] {
        assert!(cli.contains(required), "RBAC CLI must retain {required}");
    }

    let repair = cli
        .find("apply_system_role_repair_in_transaction(&tx, tenant_id)")
        .expect("CLI apply must repair inside the transaction");
    let reserve = cli
        .find("reserve_permission_invalidation_generation(&tx)")
        .expect("CLI apply must reserve durable generation");
    let commit = cli
        .find("tx.commit().await")
        .expect("CLI apply must commit after repair and generation");
    assert!(repair < reserve);
    assert!(reserve < commit);
}

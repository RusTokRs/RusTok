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

#[test]
fn legacy_role_replacement_alias_is_private_and_confined_to_new_user_creation() {
    let service = source("apps/server/src/services/rbac_service.rs");
    assert!(service.contains("pub(crate) async fn replace_user_role("));
    assert!(!service.contains("    pub async fn replace_user_role("));

    let server_src = repo_root().join("apps/server/src");
    let mut files = Vec::new();
    rust_sources(&server_src, &mut files);
    let mut call_sites = Vec::new();
    for path in files {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for _ in content.match_indices("RbacService::replace_user_role(") {
            call_sites.push(path.clone());
        }
    }

    assert_eq!(call_sites.len(), 1, "legacy alias must have one call site");
    assert!(call_sites[0].ends_with("services/auth_lifecycle.rs"));

    let lifecycle = source("apps/server/src/services/auth_lifecycle.rs");
    let create_start = lifecycle
        .find("pub(crate) async fn create_user_in_tx")
        .expect("transactional user creation must exist");
    let create_end = lifecycle[create_start..]
        .find("pub async fn update_profile_runtime")
        .map(|offset| create_start + offset)
        .expect("user creation block must end before profile update");
    let create_block = &lifecycle[create_start..create_end];
    assert_eq!(create_block.matches("RbacService::replace_user_role(").count(), 1);
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
fn public_role_repair_boundary_requires_database_transaction() {
    let exports = source("crates/rustok-rbac/src/lib.rs");
    let server = source("apps/server/src/services/rbac_repair.rs");

    assert!(exports.contains("mod repair;"));
    assert!(!exports.contains("pub mod repair;"));
    assert!(exports.contains("db: &sea_orm::DatabaseTransaction"));
    assert!(exports.contains("repair::repair_system_roles_in_transaction(db, options).await"));
    assert!(server.contains("rustok_rbac::repair_system_roles_in_transaction(\n            &tx,"));
}

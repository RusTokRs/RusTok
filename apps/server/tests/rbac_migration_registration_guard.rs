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
fn rbac_integrity_migration_is_registered_once_through_auth_module() {
    let root = repo_root();
    let registry = source("crates/rustok-auth/src/migrations/mod.rs");
    let migration = source(
        "crates/rustok-auth/src/migrations/m20260714_900001_enforce_rbac_relation_tenant_integrity.rs",
    );

    assert!(registry.contains("mod m20260714_900001_enforce_rbac_relation_tenant_integrity;"));
    assert!(
        registry.contains(
            "Box::new(m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration)"
        )
    );
    assert!(!root
        .join(
            "crates/rustok-migrations/src/m20260714_900001_enforce_rbac_relation_tenant_integrity.rs"
        )
        .exists());

    for required in [
        "trg_rbac_user_roles_tenant_insert",
        "trg_rbac_role_permissions_tenant_insert",
        "trg_rbac_users_tenant_update",
        "trg_rbac_roles_tenant_update",
        "trg_rbac_permissions_tenant_update",
    ] {
        assert!(
            migration.contains(required),
            "migration must retain {required}"
        );
    }
}

#[test]
fn durable_rbac_generation_migration_is_registered_once_through_auth_module() {
    let root = repo_root();
    let registry = source("crates/rustok-auth/src/migrations/mod.rs");
    let migration = source(
        "crates/rustok-auth/src/migrations/m20260714_900002_create_rbac_invalidation_state.rs",
    );

    assert!(registry.contains("mod m20260714_900002_create_rbac_invalidation_state;"));
    assert!(
        registry.contains("Box::new(m20260714_900002_create_rbac_invalidation_state::Migration)")
    );
    assert!(
        !root
            .join("crates/rustok-migrations/src/m20260714_900002_create_rbac_invalidation_state.rs")
            .exists()
    );
    for required in [
        "rbac_invalidation_state",
        "RBAC_PERMISSION_SCOPE",
        "Generation",
        "UpdatedAt",
    ] {
        assert!(
            migration.contains(required),
            "migration must retain {required}"
        );
    }
}

#[test]
fn central_migrator_keeps_existing_dependency_and_inventory_tests() {
    let central = source("crates/rustok-migrations/src/lib.rs");

    for required in [
        "module_migration_sources_cover_server_module_crates",
        "dependency_sort_rejects_missing_dependency",
        "dependency_sort_rejects_cycle",
        "dependency_sort_rejects_duplicate_descriptor_for_same_migration",
        "migrator_includes_auth_migrations_in_sorted_order",
        "collected_descriptors_reference_existing_migrations",
        "migrator_includes_search_storage_migrations",
        "migrator_includes_content_module_migrations",
    ] {
        assert!(
            central.contains(required),
            "central migration suite must retain {required}"
        );
    }
}

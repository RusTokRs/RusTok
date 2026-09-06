use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/rustok-installer should live under workspace root")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn typed_seed_executor_requires_composite_principal_port() {
    let seed = source("crates/rustok-installer/src/seed.rs");

    assert!(seed.contains("principal_port: &dyn SeedPrincipalPort"));
    assert!(seed.contains("ensure_seed_principal(admin, UserRole::SuperAdmin)"));
    assert!(seed.contains("UserRole::Customer"));
    assert!(!seed.contains("identity_port.ensure_seed_user"));
    assert!(!seed.contains("role_port.assign_seed_role"));
}

#[test]
fn seaorm_principal_adapter_uses_one_transaction() {
    let adapter = source("crates/rustok-installer-persistence/src/seaorm_ports.rs");
    let start = adapter
        .find("impl SeedPrincipalPort for SeaOrmInstallerBootstrapPorts")
        .expect("SeaORM adapter must implement SeedPrincipalPort");
    let body = &adapter[start..];

    assert!(body.contains("let tx = self.db.begin().await"));
    assert!(body.contains("AuthUserBootstrapDbWriter::ensure_user_on(\n                &tx"));
    assert!(
        body.contains(
            "RbacRoleAssignmentDbWriter::assign_role_permissions_on(\n                &tx"
        )
    );
    assert!(body.contains("tx.commit().await"));
    assert!(body.contains("tx.rollback().await"));
}

#[test]
fn installer_cli_uses_three_port_seed_contract() {
    let cli = source("crates/rustok-installer-cli/src/lib.rs");
    let call = cli
        .find("let result = execute_seed_profile(")
        .expect("installer CLI must call typed seed executor");
    let tail = &cli[call..];
    let end = tail
        .find(".await")
        .expect("typed seed executor call must be awaited");
    let invocation = &tail[..end];

    assert_eq!(invocation.matches("&ports").count(), 3);
}

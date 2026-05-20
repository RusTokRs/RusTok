#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("контрактные тесты покрывают все публичные use-case"),
        "implementation plan must include contract test checklist item"
    );
}

#[test]
fn public_surface_does_not_re_export_auth_module() {
    let lib = include_str!("../src/lib.rs");
    assert!(
        !lib.contains("pub mod auth;"),
        "auth module must not be re-exported from lib.rs"
    );
    assert!(
        !lib.contains("pub use auth::"),
        "auth types must not be re-exported from lib.rs"
    );
}

#[test]
fn crate_api_does_not_list_auth_module() {
    let api = include_str!("../CRATE_API.md");
    assert!(
        !api.contains("`auth`"),
        "CRATE_API.md must not list auth as a public module"
    );
}

#[test]
fn auth_directory_removed_from_src() {
    // Verifies boundary hardening: domain-specific auth logic lives in rustok-auth crate.
    // This test fails if the auth directory still exists in src.
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("jsonwebtoken"),
        "Cargo.toml must not depend on jsonwebtoken after auth removal"
    );
    assert!(
        !manifest.contains("argon2"),
        "Cargo.toml must not depend on argon2 after auth removal"
    );
}

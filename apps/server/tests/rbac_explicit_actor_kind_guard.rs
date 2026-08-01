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

fn assert_ordered(content: &str, first: &str, second: &str, context: &str) {
    let first_position = content
        .find(first)
        .unwrap_or_else(|| panic!("{context} is missing marker: {first}"));
    let second_position = content
        .find(second)
        .unwrap_or_else(|| panic!("{context} is missing marker: {second}"));
    assert!(
        first_position < second_position,
        "{context} must evaluate {first} before {second}"
    );
}

#[test]
fn shared_context_publishes_a_typed_principal_kind() {
    let kind = source("crates/rustok-api/src/context/principal_kind.rs");
    for marker in [
        "pub enum AuthPrincipalKind",
        "DirectUser",
        "DelegatedUser",
        "Service",
    ] {
        assert!(kind.contains(marker), "principal kind contract missing {marker}");
    }

    let principal = source("crates/rustok-api/src/context/principal.rs");
    for marker in [
        "pub fn validated_principal_kind",
        "AuthPrincipalKind::DirectUser",
        "AuthPrincipalKind::DelegatedUser",
        "AuthPrincipalKind::Service",
        "InvalidAuthenticatedFacts",
        "malformed_authenticated_facts_fail_closed",
    ] {
        assert!(
            principal.contains(marker),
            "shared principal classifier missing {marker}"
        );
    }
}

#[test]
fn rbac_owner_policy_does_not_reinterpret_legacy_auth_facts() {
    let owner = source("crates/rustok-rbac/src/control_plane.rs");
    for marker in [
        "pub kind: AuthPrincipalKind",
        "AuthPrincipalKind::DirectUser",
        "principal.session_id.is_nil()",
    ] {
        assert!(owner.contains(marker), "RBAC owner policy missing {marker}");
    }
    for forbidden in ["grant_type", "client_id", "authorization_code", "client_credentials"] {
        assert!(
            !owner.contains(forbidden),
            "RBAC owner policy must not reinterpret legacy auth fact: {forbidden}"
        );
    }
}

#[test]
fn every_control_plane_adapter_validates_kind_before_permission_admission() {
    let graphql = source("crates/rustok-rbac/src/graphql/control_plane.rs");
    assert!(graphql.contains("auth.validated_principal_kind()"));
    assert!(graphql.contains("kind,"));

    let rest = source("apps/server/src/controllers/artifact_permissions.rs");
    assert_ordered(
        &rest,
        "auth.validated_principal_kind()",
        "ensure_modules_manage(&auth.permissions)",
        "artifact-permission REST admission",
    );
    assert!(rest.contains("kind,"));

    let native = source(
        "crates/rustok-rbac/admin/src/transport/native_server_adapter.rs",
    );
    assert_ordered(
        &native,
        "auth.validated_principal_kind()",
        "has_effective_permission(&auth.permissions",
        "native RBAC admin admission",
    );
    assert!(native.contains("kind,"));
}

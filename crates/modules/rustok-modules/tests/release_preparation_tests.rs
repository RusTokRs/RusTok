use uuid::Uuid;

use rustok_modules::{
    ModuleInstallationScope, OciArtifactReference, ReleasePreparation, ReleasePreparationError,
};

fn sample_reference() -> OciArtifactReference {
    OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "modules/analytics".to_string(),
        digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
    }
}

#[test]
fn test_release_preparation_lifecycle_transitions() {
    let prep_id = Uuid::new_v4();
    let mut prep = ReleasePreparation::new(
        prep_id,
        ModuleInstallationScope::Platform,
        sample_reference(),
        true,
    );

    assert_eq!(prep.state.name(), "received");
    assert!(!prep.state.is_terminal());

    // Advance to verifying
    assert!(prep.advance_to_verifying().is_ok());
    assert_eq!(prep.state.name(), "verifying");

    // Advance to validating
    assert!(prep.advance_to_validating().is_ok());
    assert_eq!(prep.state.name(), "validating");

    // Advance to publishing
    assert!(prep.advance_to_publishing().is_ok());
    assert_eq!(prep.state.name(), "publishing");

    // Finalize admitted
    let digest =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    assert!(prep.finalize_admitted(digest).is_ok());
    assert_eq!(prep.state.name(), "admitted");
    assert!(prep.state.is_terminal());
    assert!(prep.state.is_admitted());

    // Further mutation on terminal state fails
    assert!(prep.advance_to_verifying().is_err());
    assert!(prep.reject("late error".to_string()).is_err());
}

#[test]
fn test_tenant_authorization_and_metadata_isolation() {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    // 1. Private Tenant Preparation: owned by Tenant A
    let private_prep = ReleasePreparation::new(
        Uuid::new_v4(),
        ModuleInstallationScope::Tenant {
            tenant_id: tenant_a,
        },
        sample_reference(),
        false,
    );

    // Tenant A has access
    assert!(private_prep.can_share_metadata_with(tenant_a));
    // Tenant B has NO access
    assert!(!private_prep.can_share_metadata_with(tenant_b));

    // 2. Public Platform Catalog Release
    let public_prep = ReleasePreparation::new(
        Uuid::new_v4(),
        ModuleInstallationScope::Platform,
        sample_reference(),
        true,
    );
    // Both Tenant A and Tenant B can share catalog metadata
    assert!(public_prep.can_share_metadata_with(tenant_a));
    assert!(public_prep.can_share_metadata_with(tenant_b));

    // 3. Private Platform Release (internal only, not published to catalog)
    let internal_platform_prep = ReleasePreparation::new(
        Uuid::new_v4(),
        ModuleInstallationScope::Platform,
        sample_reference(),
        false,
    );
    assert!(!internal_platform_prep.can_share_metadata_with(tenant_a));
    assert!(!internal_platform_prep.can_share_metadata_with(tenant_b));
}

#[test]
fn test_sanitized_evidence_projection() {
    let prep_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let prep = ReleasePreparation::new(
        prep_id,
        ModuleInstallationScope::Tenant { tenant_id },
        sample_reference(),
        false,
    );

    let evidence = prep.sanitized_evidence();
    assert_eq!(evidence.preparation_id, prep_id);
    assert_eq!(
        evidence.scope,
        ModuleInstallationScope::Tenant { tenant_id }
    );
    assert_eq!(evidence.state_name, "received");
    assert_eq!(evidence.artifact_digest, sample_reference().digest);
    assert!(!evidence.is_public_catalog);
}

#[test]
fn test_isolated_transition_operation_id_derivation() {
    let prep_id = Uuid::new_v4();
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    let mut public_prep = ReleasePreparation::new(
        prep_id,
        ModuleInstallationScope::Platform,
        sample_reference(),
        true,
    );

    // Cannot derive operation_id until admitted
    assert!(matches!(
        public_prep.derive_transition_operation_id(Some(tenant_a), "cmd_1"),
        Err(ReleasePreparationError::NotAdmitted(..))
    ));

    // Admit the release
    public_prep.advance_to_verifying().expect("verifying");
    public_prep.advance_to_validating().expect("validating");
    public_prep.advance_to_publishing().expect("publishing");
    public_prep
        .finalize_admitted(
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
        )
        .expect("finalize");

    // Derive operation_ids for Tenant A and Tenant B
    let op_a = public_prep
        .derive_transition_operation_id(Some(tenant_a), "install_key_1")
        .expect("op_a");
    let op_b = public_prep
        .derive_transition_operation_id(Some(tenant_b), "install_key_1")
        .expect("op_b");

    // Must be completely distinct operation_ids ensuring no shared authority
    assert_ne!(op_a, op_b);

    // Same tenant and idempotency key yields idempotent operation_id
    let op_a_replay = public_prep
        .derive_transition_operation_id(Some(tenant_a), "install_key_1")
        .expect("op_a_replay");
    assert_eq!(op_a, op_a_replay);

    // Tenant-private preparation rejects unauthorized tenant
    let mut private_prep = ReleasePreparation::new(
        Uuid::new_v4(),
        ModuleInstallationScope::Tenant {
            tenant_id: tenant_a,
        },
        sample_reference(),
        false,
    );
    private_prep
        .advance_to_verifying()
        .expect("verifying private");
    private_prep
        .advance_to_validating()
        .expect("validating private");
    private_prep
        .advance_to_publishing()
        .expect("publishing private");
    private_prep
        .finalize_admitted(
            "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
        )
        .expect("finalize private");

    let err = private_prep
        .derive_transition_operation_id(Some(tenant_b), "install_key_1")
        .unwrap_err();
    assert!(matches!(
        err,
        ReleasePreparationError::UnauthorizedTenant { .. }
    ));
}

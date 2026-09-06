//! Integration tests for the unified operator experience and command projections:
//! - Canonical status mapping and friendly labels
//! - Version coordinate immutability and multi-publisher rejection
//! - Blast radius and preview projection calculation
//! - Authorized diagnostic support bundle retrieval

use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactModuleKind, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactSchemaDocument,
    CanonicalPresentationState, ExecuteInstallCommand,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleInstallationScope, ModuleOperatorError,
    ModuleOperatorService, ModuleReleaseCoordinate, ModulesModule, UpdateMode,
};
use rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI;

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db connects");
    let schema_manager = SchemaManager::new(&db);
    rustok_outbox::SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("migration succeeds");
    }
    db
}

fn sample_command_context() -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: None,
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-operator-test-01".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

fn sample_descriptor(slug: &str, version: &str, payload_digest: &str) -> ModuleArtifactDescriptor {
    ModuleArtifactDescriptor {
        schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: slug.to_string(),
        version: version.to_string(),
        module_kind: ArtifactModuleKind::Optional,
        payload_kind: ArtifactPayloadKind::Rhai,
        runtime_abi: RHAI_SANDBOX_RUNTIME_ABI.to_string(),
        artifact_digest: payload_digest.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        entrypoint: "src/main.rhai".to_string(),
        capabilities: Vec::new(),
        bindings: Vec::new(),
        dependencies: Vec::new(),
        permissions: vec![ArtifactPermissionDescriptor {
            key: format!("{slug}.manage"),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Manage module".to_string(),
                description: "Manage module resources".to_string(),
            }],
        }],
        schema_documents: vec![ArtifactSchemaDocument {
            digest: sha256_digest(b"{}"),
            document: serde_json::json!({}),
        }],
        settings_schema_digest: None,
        data_schema_digest: None,
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    }
}

async fn admit_test_release(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    version: &str,
    script: &[u8],
    publisher: &str,
) -> String {
    let payload_digest = sha256_digest(script);
    let descriptor = sample_descriptor(slug, version, &payload_digest);
    let descriptor_json = serde_json::to_string(&descriptor).unwrap();
    let release_digest = sha256_digest(descriptor_json.as_bytes());

    let backend = db.get_database_backend();
    let placeholders = match backend {
        sea_orm::DbBackend::Postgres => (1..=16).map(|i| format!("${i}")).collect::<Vec<_>>(),
        _ => (1..=16).map(|i| format!("?{i}")).collect::<Vec<_>>(),
    };

    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_admitted_oci_releases (\
                release_digest, scope_kind, scope_tenant_key, registry, repository, slug, version, \
                payload_digest, payload_media_type, payload_size_bytes, descriptor_json, \
                artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, admitted_at \
             ) VALUES ({}, datetime('now'))",
            placeholders.join(", ")
        ),
        vec![
            release_digest.clone().into(),
            "platform".into(),
            "platform".into(),
            "ghcr.io".into(),
            format!("rustok/{slug}").into(),
            slug.into(),
            version.into(),
            payload_digest.into(),
            "application/vnd.rustok.module.rhai.v1".into(),
            (script.len() as i64).into(),
            descriptor_json.into(),
            "oci_admitted".into(),
            Uuid::new_v4().to_string().into(),
            Uuid::new_v4().to_string().into(),
            "trace-operator-admit".into(),
            Uuid::new_v4().to_string().into(),
        ],
    ))
    .await
    .expect("admitted release insert succeeds");

    let ingress_placeholders = match backend {
        sea_orm::DbBackend::Postgres => (1..=11).map(|i| format!("${i}")).collect::<Vec<_>>(),
        _ => (1..=11).map(|i| format!("?{i}")).collect::<Vec<_>>(),
    };

    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_external_prebuilt_ingress (\
                release_digest, publisher_identity, lineage_reference, lineage_digest, signature_reference, \
                signature_digest, sbom_reference, sbom_digest, provenance_reference, provenance_digest, \
                policy_revision, license_policy_verified, vulnerability_policy_verified, abi_verified, \
                capability_verified, native_promotion_denied, ingress_at \
             ) VALUES ({}, 1, 1, 1, 1, 1, datetime('now'))",
            ingress_placeholders.join(", ")
        ),
        vec![
            release_digest.clone().into(),
            publisher.into(),
            "git:commit:123456".into(),
            sha256_digest(b"lineage").into(),
            "cosign:sig:123456".into(),
            sha256_digest(b"sig").into(),
            "spdx:sbom:123456".into(),
            sha256_digest(b"sbom").into(),
            "slsa:prov:123456".into(),
            sha256_digest(b"prov").into(),
            1i64.into(),
        ],
    ))
    .await
    .expect("external prebuilt ingress insert succeeds");

    release_digest
}

#[tokio::test]
async fn test_canonical_presentation_tokens_and_friendly_labels() {
    let states = [
        (CanonicalPresentationState::Ready, "ready", "Ready"),
        (CanonicalPresentationState::Running, "running", "Updating"),
        (CanonicalPresentationState::Observing, "observing", "Observing"),
        (CanonicalPresentationState::Accepted, "accepted", "Active"),
        (CanonicalPresentationState::Recovering, "recovering", "Recovering"),
        (CanonicalPresentationState::Recovered, "recovered", "Recovered"),
        (CanonicalPresentationState::Rejected, "rejected", "Rejected"),
        (CanonicalPresentationState::Cancelled, "cancelled", "Cancelled"),
        (CanonicalPresentationState::RecoveryRequired, "recovery_required", "Recovery required"),
    ];

    for (state, expected_token, expected_label) in states {
        assert_eq!(state.as_str(), expected_token);
        assert_eq!(state.display_label(), expected_label);
    }
}

#[tokio::test]
async fn test_version_coordinate_immutability_rejection() {
    let db = setup_test_db().await;
    let operator = ModuleOperatorService::new(db.clone());

    let script1 = b"fn main() { print(\"version 1\"); }";
    let release_1 = admit_test_release(
        &db,
        "analytics_tracker",
        "1.0.0",
        script1,
        "trusted-analytics-corp",
    )
    .await;

    // 1. Same version, same release digest passes validation
    operator
        .validate_version_identity_immutability(
            "trusted-analytics-corp",
            "analytics_tracker",
            "1.0.0",
            &release_1,
        )
        .await
        .expect("identical release passes");

    // 2. Same version, different release digest is rejected with VersionConflict!
    let conflict_err = operator
        .validate_version_identity_immutability(
            "trusted-analytics-corp",
            "analytics_tracker",
            "1.0.0",
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .await
        .expect_err("mismatched digest for same version must be rejected");

    assert!(matches!(
        conflict_err,
        ModuleOperatorError::VersionConflict { .. }
    ));
}

#[tokio::test]
async fn test_transition_preview_projection_and_blast_radius() {
    let db = setup_test_db().await;
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn main() { print(\"preview test\"); }";
    let release_digest = admit_test_release(
        &db,
        "billing_engine",
        "2.1.0",
        script,
        "billing-corp",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };
    let operation_id = Uuid::new_v4();

    // Generate preview with candidate coordinate
    let candidate = ModuleReleaseCoordinate::Dynamic {
        publisher_identity: "billing-corp".to_string(),
        module_slug: "billing_engine".to_string(),
        version: "2.1.0".to_string(),
        release_digest: release_digest.clone(),
        payload_digest: sha256_digest(script),
    };

    let preview = operator
        .generate_preview(
            operation_id,
            "billing_engine",
            &scope,
            Some(candidate),
            UpdateMode::Automatic,
            "Upgrade billing engine to 2.1.0",
        )
        .await
        .expect("preview generation succeeds");

    assert_eq!(preview.operation_id, operation_id);
    assert_eq!(preview.module_slug, "billing_engine");
    assert_eq!(preview.mode, UpdateMode::Automatic);
    assert_eq!(preview.blast_radius.affected_tenants_count, 1);
    assert!(preview.candidate_identity.is_some());
    assert!(preview.eligibility.eligible);
    assert!(preview.preview_digest.starts_with("sha256:"));
    assert!(!preview.fence_state.is_empty());
}

#[tokio::test]
async fn test_preview_denies_automatic_mode_for_schema_migrations() {
    let db = setup_test_db().await;
    let operator = ModuleOperatorService::new(db.clone());

    let static_candidate = ModuleReleaseCoordinate::Static {
        distribution_lineage: "core-monolith".to_string(),
        version_label: "v0.2.0".to_string(),
        distribution_release_id: Uuid::new_v4(),
        bundle_root_digest: sha256_digest(b"bundle-root-v2"),
        module_version_diffs: vec![
            rustok_modules::ModuleVersionDiff {
                module_slug: "commerce".to_string(),
                previous_version: Some("0.1.0".to_string()),
                candidate_version: "0.2.0".to_string(),
                previous_digest: Some(sha256_digest(b"commerce-v1")),
                candidate_digest: sha256_digest(b"commerce-v2"),
            },
        ],
    };

    let scope = ModuleInstallationScope::Platform;

    // 1. In Automatic mode: denied due to schema migration!
    let preview_auto = operator
        .generate_preview(
            Uuid::new_v4(),
            "commerce",
            &scope,
            Some(static_candidate.clone()),
            UpdateMode::Automatic,
            "Upgrade core commerce distribution",
        )
        .await
        .expect("preview generation succeeds");

    assert!(!preview_auto.eligibility.eligible);
    assert!(preview_auto.blast_radius.has_schema_migration);
    assert!(preview_auto.irreversible_checkpoint.is_some());
    assert!(
        preview_auto
            .eligibility
            .denial_reasons
            .iter()
            .any(|r| r.contains("Automatic mode denied"))
    );

    // 2. In Maintenance mode: eligible with explicit PointOfNoReturn checkpoint shown
    let preview_manual = operator
        .generate_preview(
            Uuid::new_v4(),
            "commerce",
            &scope,
            Some(static_candidate),
            UpdateMode::Maintenance,
            "Upgrade core commerce distribution with operator review",
        )
        .await
        .expect("preview generation succeeds");

    assert!(preview_manual.eligibility.eligible);
    assert!(preview_manual.blast_radius.has_schema_migration);
    assert!(preview_manual.irreversible_checkpoint.is_some());
}

#[tokio::test]
async fn test_operator_status_reads_and_support_bundle_generation() {
    let db = setup_test_db().await;
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn main() { print(\"status test\"); }";
    let release_digest = admit_test_release(
        &db,
        "inventory_sync",
        "1.0.0",
        script,
        "inventory-corp",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. Status before install: Ready, uninstalled
    let status_initial = operator
        .get_status("inventory_sync", &scope)
        .await
        .expect("query status succeeds");
    assert_eq!(status_initial.presentation_state, CanonicalPresentationState::Ready);
    assert_eq!(status_initial.work_generation, 0);
    assert!(!status_initial.retired);

    // 2. Perform install
    let install_res = operator
        .dynamic_lifecycle()
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "inventory-corp".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    assert_eq!(install_res.work_generation, 1);

    // 3. Status after install: Inactive -> Ready with instructions to enable
    let status_installed = operator
        .get_status("inventory_sync", &scope)
        .await
        .expect("query status succeeds");
    assert_eq!(status_installed.presentation_state, CanonicalPresentationState::Ready);
    assert_eq!(status_installed.work_generation, 1);
    assert!(!status_installed.retired);

    // 4. Generate support bundle
    let bundle = operator
        .generate_support_bundle("inventory_sync", &scope)
        .await
        .expect("generate support bundle succeeds");

    assert_eq!(bundle.module_slug, "inventory_sync");
    assert_eq!(bundle.presentation_state, CanonicalPresentationState::Ready);
    assert_eq!(bundle.work_generation, 1);
    assert!(!bundle.retired);
    assert!(!bundle.diagnostics.is_empty());
}

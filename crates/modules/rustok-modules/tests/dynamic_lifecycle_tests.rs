//! Integration tests for distinct dynamic lifecycle semantics (admit/install/enable/update/disable/remove/uninstall/rollback/purge),
//! first-install recovery to absent serving baseline, atomic uninstall with work-generation race protection,
//! retained data on uninstall, and publisher continuity enforcement.

use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactAdmissionStatus, ArtifactModuleKind, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactSchemaDocument,
    DynamicLifecycleError, DynamicLifecycleService, ExecuteDataPurgeCommand,
    ExecuteDisableCommand, ExecuteEnableCommand, ExecuteInstallCommand,
    ExecuteSettingsPurgeCommand, ExecuteUninstallCommand, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    ModuleArtifactDescriptor, ModuleCommandContext, ModuleInstallationScope,
    ModulesModule, ReinstallChoice,
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
        trace_id: "trace-dlc-test-01".to_string(),
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
            "trace-admit-test".into(),
            Uuid::new_v4().to_string().into(),
        ],
    ))
    .await
    .expect("admitted release insert succeeds");

    // Also record publisher ingress
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
async fn test_distinct_lifecycle_semantics_install_enable_disable() {
    let db = setup_test_db().await;
    let service = DynamicLifecycleService::new(db.clone());

    let script = b"fn main() { return 42; }";
    let release_digest = admit_test_release(
        &db,
        "payment_gateway",
        "1.0.0",
        script,
        "publisher-corp-certified",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. INSTALL: creates inactive installation (non-routable, non-executing)
    let install_res = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "publisher-corp-certified".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    assert_eq!(install_res.work_generation, 1);
    assert_eq!(install_res.status, ArtifactAdmissionStatus::Inactive);
    assert!(!install_res.data_owner_id.is_nil());
    assert!(!install_res.settings_instance_id.is_nil());

    let work_gen = service
        .get_work_generation(install_res.installation_id)
        .await
        .expect("work generation exists");
    assert_eq!(work_gen.work_generation, 1);
    assert!(!work_gen.retired);

    // 2. ENABLE: activates installation and sets tenant intent
    service
        .execute_enable(ExecuteEnableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            smoke_test_passed: true,
            is_first_install: true,
            context: sample_command_context(),
        })
        .await
        .expect("enable succeeds");

    // 3. DISABLE: fences traffic, marks inactive, preserves data/settings intact
    service
        .execute_disable(ExecuteDisableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            context: sample_command_context(),
        })
        .await
        .expect("disable succeeds");

    // Stale work generation rejects disable
    let stale_err = service
        .execute_disable(ExecuteDisableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 99,
            context: sample_command_context(),
        })
        .await
        .expect_err("stale work generation must be rejected");

    assert!(matches!(stale_err, DynamicLifecycleError::StaleWorkGeneration(99, 1)));
}

#[tokio::test]
async fn test_first_install_enable_failure_recovery() {
    let db = setup_test_db().await;
    let service = DynamicLifecycleService::new(db.clone());

    let script = b"fn main() { return 10; }";
    let release_digest = admit_test_release(
        &db,
        "analytics_tracker",
        "1.0.0",
        script,
        "publisher-analytics-corp",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // Install candidate
    let install_res = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "publisher-analytics-corp".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    // First install enable fails smoke evaluation
    let err = service
        .execute_enable(ExecuteEnableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            smoke_test_passed: false, // SMOKE TEST FAILED!
            is_first_install: true,
            context: sample_command_context(),
        })
        .await
        .expect_err("first install enable must fail when smoke test fails");

    assert!(matches!(err, DynamicLifecycleError::FirstInstallFailed(_)));

    // Inactive candidate remains incident-retained in database (NOT deleted!)
    let work_gen = service
        .get_work_generation(install_res.installation_id)
        .await
        .expect("installation still exists in database for incident retention");
    assert!(!work_gen.retired);
}

#[tokio::test]
async fn test_atomic_uninstall_and_work_generation_invalidation() {
    let db = setup_test_db().await;
    let service = DynamicLifecycleService::new(db.clone());

    let script = b"fn main() { return 99; }";
    let release_digest = admit_test_release(
        &db,
        "billing_connector",
        "1.0.0",
        script,
        "publisher-billing-solutions",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    let install_res = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "publisher-billing-solutions".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    // Enable first
    service
        .execute_enable(ExecuteEnableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            smoke_test_passed: true,
            is_first_install: true,
            context: sample_command_context(),
        })
        .await
        .expect("enable succeeds");

    // Disable (so it is disabled-selected)
    service
        .execute_disable(ExecuteDisableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            context: sample_command_context(),
        })
        .await
        .expect("disable succeeds");

    // UNINSTALL: atomically clears tenant intent and advances work generation to 2, marking retired
    let uninstall_res = service
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Module decommissioned".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall succeeds");

    assert_eq!(uninstall_res.advanced_work_generation, 2);
    assert!(uninstall_res.retired);
    assert!(uninstall_res.tenant_intent_cleared);

    let retired_gen = service
        .get_work_generation(install_res.installation_id)
        .await
        .expect("work generation exists");
    assert_eq!(retired_gen.work_generation, 2);
    assert!(retired_gen.retired);

    // Any delayed enable command or background work with stale work generation (1) is rejected!
    let delayed_err = service
        .execute_enable(ExecuteEnableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_res.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            smoke_test_passed: true,
            is_first_install: false,
            context: sample_command_context(),
        })
        .await
        .expect_err("delayed enable on retired installation must fail");

    assert!(matches!(delayed_err, DynamicLifecycleError::InstallationAlreadyRetired(_)));
}

#[tokio::test]
async fn test_retained_data_on_uninstall_and_publisher_continuity() {
    let db = setup_test_db().await;
    let service = DynamicLifecycleService::new(db.clone());

    let script_v1 = b"fn main() { return 1; }";
    let script_v2 = b"fn main() { return 2; }";

    let release_v1 = admit_test_release(
        &db,
        "crm_integration",
        "1.0.0",
        script_v1,
        "trusted-crm-corp",
    )
    .await;

    let release_v2 = admit_test_release(
        &db,
        "crm_integration",
        "2.0.0",
        script_v2,
        "trusted-crm-corp",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. Initial install and uninstall
    let install_1 = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_v1,
            scope: scope.clone(),
            publisher_identity: "trusted-crm-corp".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("first install succeeds");

    service
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_1.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Decommission v1".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall succeeds");

    // 2. Foreign publisher reusing the slug cannot attach retained data!
    let foreign_err = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_v2.clone(),
            scope: scope.clone(),
            publisher_identity: "rogue-imposter-corp".to_string(), // FOREIGN PUBLISHER!
            reinstall_choice: Some(ReinstallChoice::AttachRetained {
                continuity_token: "token-abc".to_string(),
            }),
            context: sample_command_context(),
        })
        .await
        .expect_err("foreign publisher must not inherit retained data");

    assert!(matches!(
        foreign_err,
        DynamicLifecycleError::PublisherContinuityViolation { .. }
    ));

    // 3. Legitimate publisher with AttachRetained successfully inherits data_owner_id!
    let reinstall_legit = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_v2.clone(),
            scope: scope.clone(),
            publisher_identity: "trusted-crm-corp".to_string(), // LEGITIMATE PUBLISHER!
            reinstall_choice: Some(ReinstallChoice::AttachRetained {
                continuity_token: "token-abc".to_string(),
            }),
            context: sample_command_context(),
        })
        .await
        .expect("reinstall by legitimate publisher succeeds");

    assert_eq!(reinstall_legit.data_owner_id, install_1.data_owner_id);
    assert_eq!(reinstall_legit.settings_instance_id, install_1.settings_instance_id);

    service
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id: reinstall_legit.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Decommission v2".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall v2 succeeds");

    let script_v3 = br#"
        fn rustok_module_init() {
            print("crm v3");
        }
    "#;
    let release_v3 = admit_test_release(
        &db,
        "crm_integration",
        "3.0.0",
        script_v3,
        "trusted-crm-corp",
    )
    .await;

    // 4. StartEmpty choice creates a fresh data owner
    let reinstall_empty = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_v3,
            scope: scope.clone(),
            publisher_identity: "trusted-crm-corp".to_string(),
            reinstall_choice: Some(ReinstallChoice::StartEmpty),
            context: sample_command_context(),
        })
        .await
        .expect("reinstall with StartEmpty succeeds");

    assert_ne!(reinstall_empty.data_owner_id, install_1.data_owner_id);
}

#[tokio::test]
async fn test_guarded_data_and_settings_purge_requires_retirement() {
    let db = setup_test_db().await;
    let service = DynamicLifecycleService::new(db.clone());

    let script = b"fn main() { return 55; }";
    let release_digest = admit_test_release(
        &db,
        "support_chat",
        "1.0.0",
        script,
        "chat-vendor-authorized",
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    let install = service
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest,
            scope: scope.clone(),
            publisher_identity: "chat-vendor-authorized".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    // 1. Purge attempted while installation is active or unretired -> STRICTLY DENIED!
    let data_purge_err = service
        .execute_data_purge(ExecuteDataPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install.installation_id,
            scope: scope.clone(),
            data_owner_id: install.data_owner_id,
            context: sample_command_context(),
        })
        .await
        .expect_err("data purge must be denied for unretired installation");

    assert!(matches!(
        data_purge_err,
        DynamicLifecycleError::PurgeDeniedInstallationNotRetired(_)
    ));

    let settings_purge_err = service
        .execute_settings_purge(ExecuteSettingsPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install.installation_id,
            scope: scope.clone(),
            settings_instance_id: install.settings_instance_id,
            context: sample_command_context(),
        })
        .await
        .expect_err("settings purge must be denied for unretired installation");

    assert!(matches!(
        settings_purge_err,
        DynamicLifecycleError::PurgeDeniedInstallationNotRetired(_)
    ));

    // 2. Uninstall to retire the installation
    service
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Retiring for purge".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall succeeds");

    // 3. Purge after retirement -> SUCCEEDS!
    service
        .execute_data_purge(ExecuteDataPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install.installation_id,
            scope: scope.clone(),
            data_owner_id: install.data_owner_id,
            context: sample_command_context(),
        })
        .await
        .expect("data purge succeeds after retirement");

    service
        .execute_settings_purge(ExecuteSettingsPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install.installation_id,
            scope: scope.clone(),
            settings_instance_id: install.settings_instance_id,
            context: sample_command_context(),
        })
        .await
        .expect("settings purge succeeds after retirement");
}

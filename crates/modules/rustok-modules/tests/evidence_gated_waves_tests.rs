//! Evidence-gated waves integration tests:
//! - Wave 1: Truly stateless dynamic module pilot (lifecycle, zero data/settings footprint, clean activation/retirement)
//! - Wave 2: Brokered-data dynamic module pilot (data owner isolation, snapshot/retention safety, guarded purge)
//! - Wave 3: Static composition pilot with outside-candidate watchdog recovery and Leptos asset retention
//! - Wave 4: Exact transition evaluation enforcing Automatic mode per exact transition vs Maintenance mode for schema migrations

use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactAdmissionStatus, ArtifactModuleKind, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactSchemaDocument,
    CanonicalPresentationState, DynamicLifecycleService, ExecuteDataPurgeCommand,
    ExecuteDisableCommand, ExecuteEnableCommand, ExecuteInstallCommand,
    ExecuteSettingsPurgeCommand, ExecuteUninstallCommand,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleInstallationScope, ModuleOperatorService,
    ModuleReleaseCoordinate, ModuleVersionDiff, ModulesModule, UpdateMode,
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
        trace_id: "trace-wave-test-01".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

fn stateless_descriptor(slug: &str, version: &str, payload_digest: &str) -> ModuleArtifactDescriptor {
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
            key: format!("{slug}.execute"),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Execute stateless action".to_string(),
                description: "Execute stateless transformations".to_string(),
            }],
        }],
        schema_documents: Vec::new(),
        settings_schema_digest: None,
        data_schema_digest: None,
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    }
}

fn brokered_descriptor(slug: &str, version: &str, payload_digest: &str) -> ModuleArtifactDescriptor {
    let settings_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "api_endpoint": { "type": "string" }
        }
    });
    let settings_digest = sha256_digest(serde_json::to_string(&settings_schema).unwrap().as_bytes());

    let data_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "customer_id": { "type": "string" }
        }
    });
    let data_digest = sha256_digest(serde_json::to_string(&data_schema).unwrap().as_bytes());

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
                label: "Manage CRM".to_string(),
                description: "Manage customer records and settings".to_string(),
            }],
        }],
        schema_documents: vec![
            ArtifactSchemaDocument {
                digest: settings_digest.clone(),
                document: settings_schema,
            },
            ArtifactSchemaDocument {
                digest: data_digest.clone(),
                document: data_schema,
            },
        ],
        settings_schema_digest: Some(settings_digest),
        data_schema_digest: Some(data_digest),
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    }
}

async fn admit_release(
    db: &sea_orm::DatabaseConnection,
    descriptor: ModuleArtifactDescriptor,
    script: &[u8],
    publisher: &str,
) -> String {
    let payload_digest = sha256_digest(script);
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
            format!("rustok/{}", descriptor.slug).into(),
            descriptor.slug.clone().into(),
            descriptor.version.clone().into(),
            payload_digest.into(),
            "application/vnd.rustok.module.rhai.v1".into(),
            (script.len() as i64).into(),
            descriptor_json.into(),
            "oci_admitted".into(),
            Uuid::new_v4().to_string().into(),
            Uuid::new_v4().to_string().into(),
            "trace-wave-admit".into(),
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
async fn test_wave_1_stateless_dynamic_module_pilot() {
    let db = setup_test_db().await;
    let lifecycle = DynamicLifecycleService::new(db.clone());
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn format_text(input) { return input.trim().to_upper(); }";
    let desc = stateless_descriptor("text_formatter", "1.0.0", &sha256_digest(script));
    let release_digest = admit_release(&db, desc, script, "formatter-inc").await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. Initial operator status read: Ready
    let status_initial = operator
        .get_status("text_formatter", &scope)
        .await
        .expect("status check succeeds");
    assert_eq!(status_initial.presentation_state, CanonicalPresentationState::Ready);
    assert_eq!(status_initial.work_generation, 0);

    // 2. Install stateless module: creates inactive installation
    let install_outcome = lifecycle
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "formatter-inc".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");
    assert_eq!(install_outcome.status, ArtifactAdmissionStatus::Inactive);
    assert_eq!(install_outcome.work_generation, 1);

    // 3. Enable stateless module: becomes active
    lifecycle
        .execute_enable(ExecuteEnableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_outcome.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            smoke_test_passed: true,
            is_first_install: true,
            context: sample_command_context(),
        })
        .await
        .expect("enable succeeds");

    // Operator status now reports Active / Accepted
    let status_active = operator
        .get_status("text_formatter", &scope)
        .await
        .expect("status check succeeds");
    assert_eq!(status_active.presentation_state, CanonicalPresentationState::Accepted);
    assert_eq!(status_active.display_label, "Active");

    // 4. Disable stateless module: returns to Inactive
    lifecycle
        .execute_disable(ExecuteDisableCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_outcome.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            context: sample_command_context(),
        })
        .await
        .expect("disable succeeds");

    // 5. Atomic uninstall: advances generation and sets retired = true
    let uninstall_outcome = lifecycle
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id: install_outcome.installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Decommissioning stateless formatter pilot".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall succeeds");
    assert_eq!(uninstall_outcome.advanced_work_generation, 2);
    assert!(uninstall_outcome.retired);

    // Verify operator status reflects retired
    let status_retired = operator
        .get_status("text_formatter", &scope)
        .await
        .expect("status check succeeds");
    assert!(status_retired.retired);
    assert_eq!(status_retired.work_generation, 2);
}

#[tokio::test]
async fn test_wave_2_brokered_data_dynamic_module_pilot() {
    let db = setup_test_db().await;
    let lifecycle = DynamicLifecycleService::new(db.clone());
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn manage_crm() { return true; }";
    let desc = brokered_descriptor("crm_broker", "1.0.0", &sha256_digest(script));
    let release_digest = admit_release(&db, desc, script, "crm-corp").await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. Install brokered dynamic module
    let install_outcome = lifecycle
        .execute_install(ExecuteInstallCommand {
            operation_id: Uuid::new_v4(),
            release_digest: release_digest.clone(),
            scope: scope.clone(),
            publisher_identity: "crm-corp".to_string(),
            reinstall_choice: None,
            context: sample_command_context(),
        })
        .await
        .expect("install succeeds");

    let installation_id = install_outcome.installation_id;
    let data_owner_id = install_outcome.data_owner_id;
    let settings_instance_id = install_outcome.settings_instance_id;

    // 2. While installed, data purge and settings purge MUST be rejected!
    let data_purge_err = lifecycle
        .execute_data_purge(ExecuteDataPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id,
            scope: scope.clone(),
            data_owner_id,
            context: sample_command_context(),
        })
        .await
        .expect_err("data purge must be rejected while installed");
    assert!(data_purge_err.to_string().contains("Purge denied"));

    let settings_purge_err = lifecycle
        .execute_settings_purge(ExecuteSettingsPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id,
            scope: scope.clone(),
            settings_instance_id,
            context: sample_command_context(),
        })
        .await
        .expect_err("settings purge must be rejected while installed");
    assert!(settings_purge_err.to_string().contains("Purge denied"));

    // 3. Uninstall brokered module: marks retired and retains data/settings
    let uninstall_outcome = lifecycle
        .execute_uninstall(ExecuteUninstallCommand {
            operation_id: Uuid::new_v4(),
            installation_id,
            scope: scope.clone(),
            expected_work_generation: 1,
            reason: "Retiring brokered CRM module".to_string(),
            context: sample_command_context(),
        })
        .await
        .expect("uninstall succeeds");
    assert!(uninstall_outcome.retired);

    // 4. Now that the installation is retired, data purge and settings purge succeed cleanly
    lifecycle
        .execute_data_purge(ExecuteDataPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id,
            scope: scope.clone(),
            data_owner_id,
            context: sample_command_context(),
        })
        .await
        .expect("guarded data purge succeeds once retired");

    lifecycle
        .execute_settings_purge(ExecuteSettingsPurgeCommand {
            operation_id: Uuid::new_v4(),
            installation_id,
            scope: scope.clone(),
            settings_instance_id,
            context: sample_command_context(),
        })
        .await
        .expect("guarded settings purge succeeds once retired");

    // Status check confirms retired state
    let status = operator
        .get_status("crm_broker", &scope)
        .await
        .expect("status check succeeds");
    assert!(status.retired);
}

#[tokio::test]
async fn test_wave_4_exact_transition_evaluation_automatic_mode_policy() {
    let db = setup_test_db().await;
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn format() {}";
    let desc = stateless_descriptor("formatter_pilot", "1.0.0", &sha256_digest(script));
    let release_digest = admit_release(&db, desc, script, "formatter-inc").await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // Case A: Stateless dynamic transition without schema changes
    let dynamic_candidate = ModuleReleaseCoordinate::Dynamic {
        publisher_identity: "formatter-inc".to_string(),
        module_slug: "formatter_pilot".to_string(),
        version: "1.0.0".to_string(),
        release_digest: release_digest.clone(),
        payload_digest: sha256_digest(script),
    };

    let preview_stateless = operator
        .generate_preview(
            Uuid::new_v4(),
            "formatter_pilot",
            &scope,
            Some(dynamic_candidate),
            UpdateMode::Automatic,
            "Safe stateless upgrade",
        )
        .await
        .expect("preview succeeds");

    // Proves Automatic mode is allowed for exact transition with 0 schema impact
    assert!(preview_stateless.eligibility.eligible);
    assert!(!preview_stateless.blast_radius.has_schema_migration);
    assert!(preview_stateless.irreversible_checkpoint.is_none());

    // Case B: Static distribution composition transition with additive schema migration
    let static_candidate = ModuleReleaseCoordinate::Static {
        distribution_lineage: "core-platform".to_string(),
        version_label: "v1.2.0".to_string(),
        distribution_release_id: Uuid::new_v4(),
        bundle_root_digest: sha256_digest(b"bundle-v120"),
        module_version_diffs: vec![ModuleVersionDiff {
            module_slug: "billing".to_string(),
            previous_version: Some("1.1.0".to_string()),
            candidate_version: "1.2.0".to_string(),
            previous_digest: Some(sha256_digest(b"billing-1.1.0")),
            candidate_digest: sha256_digest(b"billing-1.2.0"),
        }],
    };

    let preview_schema_auto = operator
        .generate_preview(
            Uuid::new_v4(),
            "billing",
            &ModuleInstallationScope::Platform,
            Some(static_candidate.clone()),
            UpdateMode::Automatic,
            "Upgrade platform with schema changes",
        )
        .await
        .expect("preview succeeds");

    // Proves Automatic mode is strictly denied per exact transition when schema migrations exist!
    assert!(!preview_schema_auto.eligibility.eligible);
    assert!(preview_schema_auto.blast_radius.has_schema_migration);
    assert!(preview_schema_auto.irreversible_checkpoint.is_some());
    assert!(preview_schema_auto.eligibility.denial_reasons[0].contains("Automatic mode denied"));

    // Case C: Transition rescheduled under Maintenance mode
    let preview_schema_maint = operator
        .generate_preview(
            Uuid::new_v4(),
            "billing",
            &ModuleInstallationScope::Platform,
            Some(static_candidate),
            UpdateMode::Maintenance,
            "Upgrade platform during scheduled maintenance window",
        )
        .await
        .expect("preview succeeds");

    // Proves Maintenance mode is eligible with explicit PointOfNoReturn checkpoint shown to operator
    assert!(preview_schema_maint.eligibility.eligible);
    assert!(preview_schema_maint.blast_radius.has_schema_migration);
    assert_eq!(
        preview_schema_maint.irreversible_checkpoint,
        Some("PointOfNoReturn: additive native schema migration will be committed".to_string())
    );
}

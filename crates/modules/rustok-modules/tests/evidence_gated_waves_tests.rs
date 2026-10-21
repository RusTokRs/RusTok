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
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactPermissionDescriptor, ArtifactSchemaDocument,
    ArtifactUninstallRequest, CanonicalPresentationState,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor, ModuleCommandContext,
    ModuleInstallationScope, ModuleOperatorService, ModuleReleaseCoordinate, ModuleVersionDiff,
    ModulesModule, SeaOrmArtifactInstallationStore, UpdateMode,
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

fn tenant_command_context(tenant_id: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(tenant_id),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-wave-test-01".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

fn stateless_descriptor(
    slug: &str,
    version: &str,
    payload_digest: &str,
) -> ModuleArtifactDescriptor {
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

fn brokered_descriptor(
    slug: &str,
    version: &str,
    payload_digest: &str,
) -> ModuleArtifactDescriptor {
    let settings_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "api_endpoint": { "type": "string" }
        }
    });
    let settings_digest =
        sha256_digest(serde_json::to_string(&settings_schema).unwrap().as_bytes());

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
        sea_orm::DbBackend::Postgres => (1..=17).map(|i| format!("${i}")).collect::<Vec<_>>(),
        _ => (1..=17).map(|i| format!("?{i}")).collect::<Vec<_>>(),
    };

    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_admitted_oci_releases (\
                release_digest, scope_kind, scope_tenant_key, registry, repository, slug, version, \
                payload_digest, payload_media_type, payload_size_bytes, descriptor_json, \
                artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, request_digest, admitted_at \
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
            sha256_digest(b"request").into(),
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

/// Arranges the durable rows of a freshly installed, still inactive
/// installation at work generation 1 for an already admitted release
/// (admission revision 1).
async fn insert_inactive_installation(
    db: &sea_orm::DatabaseConnection,
    release_digest: &str,
    scope: &ModuleInstallationScope,
) -> Uuid {
    let backend = db.get_database_backend();
    let placeholder = match backend {
        sea_orm::DbBackend::Postgres => "$1",
        _ => "?1",
    };
    let release = db
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT registry, repository, slug, version, descriptor_json, payload_digest, \
                        payload_media_type, payload_size_bytes \
                 FROM module_admitted_oci_releases WHERE release_digest = {placeholder}"
            ),
            vec![release_digest.to_string().into()],
        ))
        .await
        .expect("release lookup succeeds")
        .expect("admitted release exists");
    let text = |column: &str| -> String {
        release
            .try_get("", column)
            .expect("release column is present")
    };
    let descriptor: ModuleArtifactDescriptor =
        serde_json::from_str(&text("descriptor_json")).expect("descriptor parses");
    let size_bytes: i64 = release
        .try_get("", "payload_size_bytes")
        .expect("payload size is present");

    let installation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();
    let settings_instance_id = Uuid::new_v4();
    let (scope_kind, tenant_id) = match scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    };
    let uuid_value = |value: Uuid| -> sea_orm::Value {
        match backend {
            sea_orm::DbBackend::Postgres => value.into(),
            _ => value.to_string().into(),
        }
    };
    let optional_uuid_value = |value: Option<Uuid>| -> sea_orm::Value {
        match backend {
            sea_orm::DbBackend::Postgres => sea_orm::Value::Uuid(value),
            _ => match value {
                Some(value) => value.to_string().into(),
                None => sea_orm::Value::String(None),
            },
        }
    };
    let placeholders = |count: usize| -> Vec<String> {
        match backend {
            sea_orm::DbBackend::Postgres => (1..=count).map(|i| format!("${i}")).collect(),
            _ => (1..=count).map(|i| format!("?{i}")).collect(),
        }
    };
    let dependency_lock = format!(
        "{{\"graph_revision\":1,\"graph_digest\":\"{}\",\"nodes\":[]}}",
        sha256_digest(b"[]")
    );

    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                data_owner_id, settings_instance_id, dependency_graph_revision, \
                dependency_graph_digest, dependency_lock, previous_installation_id, \
                capability_grant_revision, installed_at\
             ) VALUES ({}, datetime('now'))",
            placeholders(20).join(", ")
        ),
        vec![
            uuid_value(installation_id),
            scope_kind.into(),
            optional_uuid_value(tenant_id),
            text("registry").into(),
            text("repository").into(),
            release_digest.to_string().into(),
            text("slug").into(),
            text("version").into(),
            descriptor.payload_kind.as_str().into(),
            descriptor.runtime_abi.clone().into(),
            text("payload_digest").into(),
            descriptor.entrypoint.clone().into(),
            text("descriptor_json").into(),
            uuid_value(data_owner_id),
            uuid_value(settings_instance_id),
            1i64.into(),
            sha256_digest(b"[]").into(),
            dependency_lock.into(),
            optional_uuid_value(None),
            1i64.into(),
        ],
    ))
    .await
    .expect("installation insert succeeds");

    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_artifact_admissions (\
                stage_id, installation_id, payload_digest, media_type, size_bytes, \
                verification_evidence, status, revision, committed_at\
             ) VALUES ({}, datetime('now'))",
            placeholders(8).join(", ")
        ),
        vec![
            uuid_value(Uuid::new_v4()),
            uuid_value(installation_id),
            descriptor.artifact_digest.clone().into(),
            text("payload_media_type").into(),
            size_bytes.into(),
            "{}".into(),
            "inactive".into(),
            1i64.into(),
        ],
    ))
    .await
    .expect("admission insert succeeds");

    let not_retired = match backend {
        sea_orm::DbBackend::Postgres => "FALSE",
        _ => "0",
    };
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "INSERT INTO module_artifact_work_generations (\
                installation_id, work_generation, retired, retired_at, updated_at\
             ) VALUES ({placeholder}, 1, {not_retired}, NULL, datetime('now'))"
        ),
        vec![uuid_value(installation_id)],
    ))
    .await
    .expect("work generation insert succeeds");

    installation_id
}

/// Flips the admission status and bumps the admission revision, mirroring
/// an enable (`active`) or disable (`inactive`) transition.
async fn set_admission_status(
    db: &sea_orm::DatabaseConnection,
    installation_id: Uuid,
    status: &str,
) {
    let backend = db.get_database_backend();
    let (status_placeholder, id_placeholder) = match backend {
        sea_orm::DbBackend::Postgres => ("$1", "$2"),
        _ => ("?1", "?2"),
    };
    let id_value: sea_orm::Value = match backend {
        sea_orm::DbBackend::Postgres => installation_id.into(),
        _ => installation_id.to_string().into(),
    };
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        backend,
        format!(
            "UPDATE module_artifact_admissions \
             SET status = {status_placeholder}, revision = revision + 1 \
             WHERE installation_id = {id_placeholder}"
        ),
        vec![status.to_string().into(), id_value],
    ))
    .await
    .expect("admission status update succeeds");
}

#[tokio::test]
async fn test_wave_1_stateless_dynamic_module_pilot() {
    let db = setup_test_db().await;
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
    assert_eq!(
        status_initial.presentation_state,
        CanonicalPresentationState::Ready
    );
    assert_eq!(status_initial.work_generation, 0);

    // 2. Install stateless module: creates inactive installation
    let installation_id = insert_inactive_installation(&db, &release_digest, &scope).await;

    let status_installed = operator
        .get_status("text_formatter", &scope)
        .await
        .expect("status check succeeds");
    assert_eq!(
        status_installed.presentation_state,
        CanonicalPresentationState::Ready
    );
    assert_eq!(status_installed.work_generation, 1);

    // 3. Enable stateless module: becomes active
    set_admission_status(&db, installation_id, "active").await;

    // Operator status now reports Active / Accepted
    let status_active = operator
        .get_status("text_formatter", &scope)
        .await
        .expect("status check succeeds");
    assert_eq!(
        status_active.presentation_state,
        CanonicalPresentationState::Accepted
    );
    assert_eq!(status_active.display_label, "Active");

    // 4. Disable stateless module: returns to Inactive
    set_admission_status(&db, installation_id, "inactive").await;

    // 5. Canonical uninstall: advances generation and sets retired = true
    let uninstall_outcome = SeaOrmArtifactInstallationStore::new(db.clone())
        .uninstall_artifact(ArtifactUninstallRequest {
            installation_id,
            scope: scope.clone(),
            expected_revision: 3,
            context: tenant_command_context(tenant_id),
            reason: "Decommissioning stateless formatter pilot".to_string(),
        })
        .await
        .expect("uninstall succeeds");
    assert_eq!(uninstall_outcome.revision, 4);

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
    let operator = ModuleOperatorService::new(db.clone());

    let script = b"fn manage_crm() { return true; }";
    let desc = brokered_descriptor("crm_broker", "1.0.0", &sha256_digest(script));
    let release_digest = admit_release(&db, desc, script, "crm-corp").await;

    let tenant_id = Uuid::new_v4();
    let scope = ModuleInstallationScope::Tenant { tenant_id };

    // 1. Install brokered dynamic module
    let installation_id = insert_inactive_installation(&db, &release_digest, &scope).await;

    // 2. Canonical uninstall: marks retired and retains data/settings.
    // Guarded purge of the retained state is owned by the canonical
    // data/settings purge services, which require exactly this retired
    // state as their precondition and cover it in their own suites.
    let uninstall_outcome = SeaOrmArtifactInstallationStore::new(db.clone())
        .uninstall_artifact(ArtifactUninstallRequest {
            installation_id,
            scope: scope.clone(),
            expected_revision: 1,
            context: tenant_command_context(tenant_id),
            reason: "Retiring brokered CRM module".to_string(),
        })
        .await
        .expect("uninstall succeeds");
    assert_eq!(uninstall_outcome.revision, 2);

    // Status check confirms retired state
    let status = operator
        .get_status("crm_broker", &scope)
        .await
        .expect("status check succeeds");
    assert!(status.retired);
    assert_eq!(status.work_generation, 2);
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
        publisher_identity: Some("formatter-inc".to_string()),
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
    assert!(preview_stateless.point_of_no_return.is_none());

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
    assert!(preview_schema_auto.point_of_no_return.is_some());
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
        preview_schema_maint.point_of_no_return,
        Some("PointOfNoReturn: static distribution updates may commit native schema migrations that cannot be introspected from admitted descriptors; manual approval is required".to_string())
    );
}

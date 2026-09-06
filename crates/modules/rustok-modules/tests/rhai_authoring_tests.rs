//! Integration tests for Rhai authoring pipeline and immutable release packaging.

use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactBlobStore, ArtifactPermissionDescriptor, ArtifactPersistenceContract,
    ArtifactSchemaDocument, InMemoryArtifactBlobStore, ModuleBindingIdempotency,
    ModuleControlPlane, ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModulesModule,
    RhaiAuthoringError, RhaiAuthoringPackageCommand,
};
use rustok_sandbox::{RhaiWorkspace, RhaiWorkspaceFile, RhaiWorkspaceFileKind};
use sea_orm::Database;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[tokio::test]
async fn test_rhai_authoring_pipeline_lifecycle() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }

    let blob_store = Arc::new(InMemoryArtifactBlobStore::default());
    let control_plane = ModuleControlPlane::new(database.clone());
    let authoring = control_plane.rhai_authoring(blob_store.clone());

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let alloy_script_id = Uuid::new_v4();
    let review_decision_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    // 1. Build bounded workspace
    let workspace = RhaiWorkspace {
        schema_version: 1,
        entrypoint: "src/main.rhai".to_string(),
        files: vec![
            RhaiWorkspaceFile {
                path: "src/main.rhai".to_string(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: "fn main() { return 42; }".to_string(),
            },
            RhaiWorkspaceFile {
                path: "src/event_handler.rhai".to_string(),
                kind: RhaiWorkspaceFileKind::Source,
                contents: "fn on_order(e) { return true; }".to_string(),
            },
        ],
    };

    // 2. Build schema document
    let schema_doc = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "order_id": { "type": "string" }
        }
    });
    let schema_bytes = serde_json::to_vec(&schema_doc).unwrap();
    let schema_digest = format!("sha256:{}", hex::encode(Sha256::digest(&schema_bytes)));

    // 3. Build command
    let command = RhaiAuthoringPackageCommand {
        tenant_id,
        actor_id,
        slug: "orders_notifier".to_string(),
        version: "1.0.0".to_string(),
        alloy_script_id,
        alloy_revision: 2,
        review_decision_id,
        review_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        workspace: workspace.clone(),
        bindings: vec![ModuleRuntimeBinding {
            id: "order_event".to_string(),
            kind: ModuleRuntimeBindingKind::Event,
            entrypoint: "src/event_handler.rhai".to_string(),
            input_schema_digest: schema_digest.clone(),
            output_schema_digest: schema_digest.clone(),
            permission: "orders_notifier.read".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "standard".to_string(),
            capabilities: vec![],
            event_topics: vec!["orders.created".to_string()],
            schedule: None,
            http: None,
        }],
        permissions: vec![ArtifactPermissionDescriptor {
            key: "orders_notifier.read".to_string(),
            localizations: vec![rustok_api::ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Read Orders".to_string(),
                description: "Allows reading orders".to_string(),
            }],
        }],
        schema_documents: vec![ArtifactSchemaDocument {
            digest: schema_digest.clone(),
            document: schema_doc,
        }],
        settings_schema_digest: None,
        data_schema_digest: Some(schema_digest.clone()),
        persistence_contract: Some(ArtifactPersistenceContract {
            revision: 1,
            schema_digest,
            indexes: vec![],
        }),
        capabilities: vec![],
        trace_id: "trace-authoring-1".to_string(),
        idempotency_key,
    };

    // 4. Package release
    let release = authoring
        .package_release(command.clone())
        .await
        .expect("package release");

    assert_eq!(release.descriptor.slug, "orders_notifier");
    assert_eq!(release.descriptor.version, "1.0.0");
    assert_eq!(release.source_cas_receipt.created, true);
    assert_eq!(
        release.descriptor.artifact_digest,
        release.source_cas_receipt.source_digest
    );
    assert_eq!(
        release.oci_payload.digest,
        release.source_cas_receipt.source_digest
    );
    assert_eq!(
        release.oci_payload.annotations.get("io.rustok.alloy.script_id").unwrap(),
        &alloy_script_id.to_string()
    );

    // 5. Verify blob exists in CAS and matches canonical workspace bytes
    let canonical_bytes = workspace.canonical_bytes().unwrap();
    let cas_bytes = blob_store
        .get_verified(&release.source_cas_receipt.source_digest)
        .await
        .expect("cas bytes");
    assert_eq!(cas_bytes, canonical_bytes);

    // 6. Test Idempotency: exact retry reuses existing package without recreating CAS
    let retry_release = authoring
        .package_release(command.clone())
        .await
        .expect("retry package release");

    assert_eq!(retry_release.package_id, release.package_id);
    assert_eq!(retry_release.source_cas_receipt.created, false);
    assert_eq!(
        retry_release.descriptor_digest,
        release.descriptor_digest
    );

    // 7. Test Idempotency Conflict: same idempotency key but modified workspace content
    let mut modified_command = command.clone();
    modified_command.workspace = RhaiWorkspace {
        schema_version: 1,
        entrypoint: "src/main.rhai".to_string(),
        files: vec![RhaiWorkspaceFile {
            path: "src/main.rhai".to_string(),
            kind: RhaiWorkspaceFileKind::Source,
            contents: "fn main() { return 999; }".to_string(),
        }],
    };
    modified_command.bindings = vec![]; // Remove binding to keep valid entrypoints

    let conflict_err = authoring
        .package_release(modified_command)
        .await
        .expect_err("idempotency conflict must be rejected");

    assert!(matches!(
        conflict_err,
        RhaiAuthoringError::IdempotencyConflict { .. }
    ));
}

#[tokio::test]
async fn test_rhai_authoring_validation_rejections() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }

    let blob_store = Arc::new(InMemoryArtifactBlobStore::default());
    let control_plane = ModuleControlPlane::new(database.clone());
    let authoring = control_plane.rhai_authoring(blob_store);

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let alloy_script_id = Uuid::new_v4();
    let review_decision_id = Uuid::new_v4();

    // 1. Missing workspace entrypoint
    let bad_workspace = RhaiWorkspace {
        schema_version: 1,
        entrypoint: "src/main.rhai".to_string(),
        files: vec![RhaiWorkspaceFile {
            path: "src/other.rhai".to_string(),
            kind: RhaiWorkspaceFileKind::Source,
            contents: "fn other() {}".to_string(),
        }],
    };

    let mut command = RhaiAuthoringPackageCommand {
        tenant_id,
        actor_id,
        slug: "bad_entrypoint".to_string(),
        version: "1.0.0".to_string(),
        alloy_script_id,
        alloy_revision: 1,
        review_decision_id,
        review_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        workspace: bad_workspace,
        bindings: vec![],
        permissions: vec![],
        schema_documents: vec![],
        settings_schema_digest: None,
        data_schema_digest: None,
        persistence_contract: None,
        capabilities: vec![],
        trace_id: "trace-bad-1".to_string(),
        idempotency_key: Uuid::new_v4(),
    };

    let err = authoring
        .package_release(command.clone())
        .await
        .expect_err("missing workspace entrypoint must fail");
    assert!(matches!(
        err,
        RhaiAuthoringError::Workspace(rustok_sandbox::RhaiWorkspaceError::MissingEntrypoint(_))
    ));

    // 2. Missing binding entrypoint
    command.slug = "bad_binding".to_string();
    command.workspace = RhaiWorkspace::single_source("fn main() {}");
    command.bindings = vec![ModuleRuntimeBinding {
        id: "missing_hook".to_string(),
        kind: ModuleRuntimeBindingKind::Command,
        entrypoint: "src/non_existent.rhai".to_string(),
        input_schema_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        output_schema_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        permission: "bad_binding.test".to_string(),
        idempotency: ModuleBindingIdempotency::Required,
        limit_profile: "standard".to_string(),
        capabilities: vec![],
        event_topics: vec![],
        schedule: None,
        http: None,
    }];
    command.permissions = vec![ArtifactPermissionDescriptor {
        key: "bad_binding.test".to_string(),
        localizations: vec![rustok_api::ArtifactPermissionLocalization {
            locale: "en".to_string(),
            label: "Test".to_string(),
            description: "Test permission".to_string(),
        }],
    }];

    let err = authoring
        .package_release(command.clone())
        .await
        .expect_err("missing binding entrypoint must fail");
    assert!(matches!(
        err,
        RhaiAuthoringError::MissingBindingEntrypoint { .. }
    ));

    // 3. Undeclared binding permission
    command.bindings[0].entrypoint = "src/main.rhai".to_string();
    command.permissions = vec![]; // Remove permission declaration

    let err = authoring
        .package_release(command.clone())
        .await
        .expect_err("undeclared permission must fail");
    assert!(matches!(
        err,
        RhaiAuthoringError::UndeclaredBindingPermission { .. }
    ));
}

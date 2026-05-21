use rustok_core::ModuleRegistry;
use rustok_server::models::build::Entity as BuildEntity;
use rustok_server::modules::{ManifestDiff, ManifestModuleSpec, ModulesManifest};
use rustok_server::services::build_service::NoopBuildEventPublisher;
use rustok_server::services::platform_composition::{
    PlatformCompositionBuildError, PlatformCompositionBuildService, PlatformCompositionService,
};
use sea_orm::{Database, DatabaseConnection, DbBackend, EntityTrait, Statement};
use std::sync::Arc;

async fn setup_db(include_builds: bool) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("db connect");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE platform_state (
            id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL,
            manifest_json TEXT NOT NULL,
            manifest_hash TEXT NOT NULL,
            active_release_id TEXT NULL,
            updated_by TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    ))
    .await
    .expect("create platform_state");

    if include_builds {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            r#"
            CREATE TABLE builds (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                stage TEXT NOT NULL,
                progress INTEGER NOT NULL,
                profile TEXT NOT NULL,
                manifest_ref TEXT NOT NULL,
                manifest_hash TEXT NOT NULL UNIQUE,
                manifest_revision INTEGER NOT NULL,
                manifest_snapshot TEXT NOT NULL,
                modules_delta TEXT NULL,
                requested_by TEXT NOT NULL,
                reason TEXT NULL,
                release_id TEXT NULL,
                logs_url TEXT NULL,
                error_message TEXT NULL,
                started_at TEXT NULL,
                finished_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        ))
        .await
        .expect("create builds");
    }

    db
}

async fn enqueue_default_manifest(
    db: &DatabaseConnection,
) -> rustok_server::services::platform_composition::PlatformCompositionBuildResult {
    let registry = ModuleRegistry::new();
    let publisher = Arc::new(NoopBuildEventPublisher);
    let manifest = ModulesManifest::default();

    let seeded = PlatformCompositionService::active_snapshot(db)
        .await
        .expect("seed active snapshot");

    PlatformCompositionBuildService::update_manifest_and_request_build(
        db,
        publisher,
        &registry,
        Some(seeded.revision),
        manifest,
        ManifestDiff::default(),
        "test-admin".to_string(),
        "success case".to_string(),
    )
    .await
    .expect("build request should succeed")
}

fn invalid_manifest_with_missing_dependency() -> ModulesManifest {
    let mut invalid_manifest = ModulesManifest::default();
    invalid_manifest.modules.insert(
        "catalog".to_string(),
        ManifestModuleSpec {
            source: "workspace".to_string(),
            crate_name: "rustok-catalog".to_string(),
            depends_on: vec!["missing-dependency".to_string()],
            ..ManifestModuleSpec::default()
        },
    );
    invalid_manifest
}

async fn assert_snapshot_unchanged(
    db: &DatabaseConnection,
    seeded: &rustok_server::services::platform_composition::PlatformCompositionSnapshot,
    context: &str,
) {
    let state_after = PlatformCompositionService::active_snapshot(db)
        .await
        .expect("load state after failed operation");
    assert_eq!(
        state_after.revision, seeded.revision,
        "revision must stay unchanged for {context}"
    );
    assert_eq!(
        state_after.manifest_hash, seeded.manifest_hash,
        "manifest hash must stay unchanged for {context}"
    );
    assert_eq!(
        state_after.manifest, seeded.manifest,
        "manifest payload must stay unchanged for {context}"
    );
}

async fn assert_no_builds_enqueued(db: &DatabaseConnection, context: &str) {
    let builds = BuildEntity::find().all(db).await.expect("list builds");
    assert!(builds.is_empty(), "no builds expected for {context}");
}

#[tokio::test]
async fn stale_revision_does_not_enqueue_build() {
    let db = setup_db(true).await;
    let registry = ModuleRegistry::new();
    let publisher = Arc::new(NoopBuildEventPublisher);
    let manifest = ModulesManifest::default();

    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");

    let err = PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        Some(seeded.revision - 1),
        manifest,
        ManifestDiff::default(),
        "test-admin".to_string(),
        "stale revision case".to_string(),
    )
    .await
    .expect_err("must fail with revision conflict");

    assert!(matches!(
        err,
        PlatformCompositionBuildError::Composition(
            rustok_server::services::platform_composition::PlatformCompositionError::RevisionConflict { .. }
        )
    ));

    let state_after = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("load state after stale revision");
    assert_eq!(state_after.revision, seeded.revision);

    let builds = BuildEntity::find().all(&db).await.expect("list builds");
    assert!(
        builds.is_empty(),
        "no build should be enqueued on stale CAS"
    );
}

#[tokio::test]
async fn build_insert_error_rolls_back_platform_revision() {
    let db = setup_db(false).await;
    let registry = ModuleRegistry::new();
    let publisher = Arc::new(NoopBuildEventPublisher);
    let manifest = ModulesManifest::default();

    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");

    let err = PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        Some(seeded.revision),
        manifest,
        ManifestDiff::default(),
        "test-admin".to_string(),
        "missing builds table".to_string(),
    )
    .await
    .expect_err("build insert must fail without builds table");

    assert!(matches!(err, PlatformCompositionBuildError::Build(_)));

    let state_after = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("load state after rollback");
    assert_eq!(
        state_after.revision, seeded.revision,
        "revision must be rolled back when build enqueue fails"
    );
}

#[tokio::test]
async fn manifest_validation_error_does_not_update_state_or_enqueue_build() {
    let db = setup_db(true).await;
    let registry = ModuleRegistry::new();
    let publisher = Arc::new(NoopBuildEventPublisher);

    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");

    let invalid_manifest = invalid_manifest_with_missing_dependency();

    let err = PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        Some(seeded.revision),
        invalid_manifest,
        ManifestDiff::default(),
        "test-admin".to_string(),
        "invalid manifest should fail validation".to_string(),
    )
    .await
    .expect_err("manifest validation should fail before transaction update");

    assert!(matches!(
        err,
        PlatformCompositionBuildError::Composition(
            rustok_server::services::platform_composition::PlatformCompositionError::Manifest(_)
        )
    ));

    assert_snapshot_unchanged(&db, &seeded, "manifest validation failure (build path)").await;
    assert_no_builds_enqueued(&db, "manifest validation failure (build path)").await;
}

#[tokio::test]
async fn update_manifest_validation_error_does_not_update_platform_state() {
    let db = setup_db(true).await;
    let registry = ModuleRegistry::new();

    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");

    let invalid_manifest = invalid_manifest_with_missing_dependency();

    let err = PlatformCompositionService::update_manifest(
        &db,
        &registry,
        Some(seeded.revision),
        invalid_manifest,
        Some("test-admin".to_string()),
    )
    .await
    .expect_err("manifest validation should fail before platform state update");

    assert!(matches!(
        err,
        rustok_server::services::platform_composition::PlatformCompositionError::Manifest(_)
    ));

    assert_snapshot_unchanged(&db, &seeded, "manifest validation failure (update path)").await;
    assert_no_builds_enqueued(&db, "manifest validation failure (update path)").await;
}

#[tokio::test]
async fn update_manifest_stale_revision_conflict_does_not_update_platform_state() {
    let db = setup_db(true).await;
    let registry = ModuleRegistry::new();

    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");

    let err = PlatformCompositionService::update_manifest(
        &db,
        &registry,
        Some(seeded.revision - 1),
        ModulesManifest::default(),
        Some("test-admin".to_string()),
    )
    .await
    .expect_err("stale revision must fail with conflict");

    assert!(matches!(
        err,
        rustok_server::services::platform_composition::PlatformCompositionError::RevisionConflict { .. }
    ));

    assert_snapshot_unchanged(&db, &seeded, "stale revision conflict (update path)").await;
    assert_no_builds_enqueued(&db, "stale revision conflict (update path)").await;
}

#[tokio::test]
async fn successful_enqueue_sets_manifest_ref_to_platform_state_revision() {
    let db = setup_db(true).await;
    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");
    let result = enqueue_default_manifest(&db).await;

    assert_eq!(result.snapshot.revision, seeded.revision + 1);
    assert_eq!(
        result.build.manifest_ref,
        format!("platform_state:{}", result.snapshot.revision)
    );
    assert_eq!(result.build.manifest_revision, result.snapshot.revision);

    let state_after = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("load state after success");
    assert_eq!(state_after.revision, result.snapshot.revision);
}

#[tokio::test]
async fn successful_enqueue_keeps_hash_parity_between_snapshot_and_build() {
    let db = setup_db(true).await;
    let result = enqueue_default_manifest(&db).await;

    let expected_hash = PlatformCompositionService::manifest_hash(&result.snapshot.manifest);
    assert_eq!(result.snapshot.manifest_hash, expected_hash);
    assert_eq!(result.build.manifest_hash, expected_hash);
}

#[tokio::test]
async fn successful_enqueue_keeps_manifest_snapshot_parity_with_hash() {
    let db = setup_db(true).await;
    let result = enqueue_default_manifest(&db).await;

    let persisted_snapshot: serde_json::Value =
        serde_json::from_str(&result.build.manifest_snapshot)
            .expect("manifest snapshot in build should be valid json");
    let expected_snapshot =
        PlatformCompositionService::manifest_snapshot_json(&result.snapshot.manifest)
            .expect("serialize snapshot from platform state manifest");
    assert_eq!(persisted_snapshot, expected_snapshot);

    let expected_hash = rustok_api::manifest_hash::hash_manifest_snapshot(&persisted_snapshot);
    assert_eq!(result.build.manifest_hash, expected_hash);
    assert_eq!(result.snapshot.manifest_hash, expected_hash);
}

#[tokio::test]
async fn same_manifest_keeps_hash_and_snapshot_stable_across_revisions() {
    let db = setup_db(true).await;

    let first = enqueue_default_manifest(&db).await;
    let second = enqueue_default_manifest(&db).await;

    assert!(
        second.snapshot.revision > first.snapshot.revision,
        "revisions should advance for every successful enqueue"
    );
    assert_ne!(first.build.manifest_ref, second.build.manifest_ref);

    assert_eq!(first.snapshot.manifest_hash, second.snapshot.manifest_hash);
    assert_eq!(first.build.manifest_hash, second.build.manifest_hash);
    assert_eq!(first.build.manifest_snapshot, second.build.manifest_snapshot);
}

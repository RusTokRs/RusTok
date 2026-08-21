use rustok_build::NoopBuildEventPublisher;
use rustok_build::build::Entity as BuildEntity;
use rustok_core::ModuleRegistry;
use rustok_modules::ModuleCompositionError;
use rustok_outbox::SysEventsMigration;
use rustok_server::modules::{ManifestDiff, ManifestModuleSpec, ModulesManifest};
use rustok_server::services::platform_composition::{
    PlatformCompositionBuildCommand, PlatformCompositionBuildError,
    PlatformCompositionBuildService, PlatformCompositionService,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use uuid::Uuid;

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
            updated_by TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    ))
    .await
    .expect("create platform_state");

    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("create owner operation receipt table");

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
                manifest_hash TEXT NOT NULL,
                manifest_revision INTEGER NOT NULL,
                manifest_snapshot TEXT NOT NULL,
                modules_delta TEXT NULL,
                requested_by TEXT NOT NULL,
                reason TEXT NULL,
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

fn build_command(
    expected_revision: i64,
    manifest: ModulesManifest,
    manifest_diff: ManifestDiff,
    reason: &str,
) -> PlatformCompositionBuildCommand {
    PlatformCompositionBuildCommand {
        tenant_id: Uuid::new_v4(),
        actor_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        expected_revision,
        manifest,
        manifest_diff,
        reason: reason.to_string(),
    }
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
        build_command(
            seeded.revision,
            manifest,
            ManifestDiff::default(),
            "success case",
        ),
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
        PlatformCompositionService::manifest_snapshot_json(&state_after.manifest)
            .expect("serialize current manifest for comparison"),
        PlatformCompositionService::manifest_snapshot_json(&seeded.manifest)
            .expect("serialize seeded manifest for comparison"),
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
    let advanced = enqueue_default_manifest(&db).await;

    let err = match PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        build_command(
            seeded.revision,
            manifest,
            ManifestDiff::default(),
            "stale revision case",
        ),
    )
    .await
    {
        Ok(_) => panic!("must fail with revision conflict"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        PlatformCompositionBuildError::Composition(
            rustok_server::services::platform_composition::PlatformCompositionError::Owner(
                ModuleCompositionError::RevisionConflict { .. }
            )
        )
    ));

    let state_after = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("load state after stale revision");
    assert_eq!(state_after.revision, advanced.snapshot.revision);

    let builds = BuildEntity::find().all(&db).await.expect("list builds");
    assert_eq!(
        builds.len(),
        1,
        "the stale command must not enqueue a build after the prior revision advanced"
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

    let err = match PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        build_command(
            seeded.revision,
            manifest,
            ManifestDiff::default(),
            "missing builds table",
        ),
    )
    .await
    {
        Ok(_) => panic!("build insert must fail without builds table"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        PlatformCompositionBuildError::Composition(
            rustok_server::services::platform_composition::PlatformCompositionError::Owner(
                ModuleCompositionError::BuildEnqueue(_)
            )
        )
    ));

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

    let err = match PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        build_command(
            seeded.revision,
            invalid_manifest,
            ManifestDiff::default(),
            "invalid manifest should fail validation",
        ),
    )
    .await
    {
        Ok(_) => panic!("manifest validation should fail before transaction update"),
        Err(err) => err,
    };

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
async fn exact_idempotency_retry_replays_original_build_after_composition_changes() {
    let db = setup_db(true).await;
    let registry = ModuleRegistry::new();
    let publisher = Arc::new(NoopBuildEventPublisher);
    let seeded = PlatformCompositionService::active_snapshot(&db)
        .await
        .expect("seed active snapshot");
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    let first = PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher.clone(),
        &registry,
        PlatformCompositionBuildCommand {
            tenant_id,
            actor_id,
            idempotency_key,
            expected_revision: seeded.revision,
            manifest: ModulesManifest::default(),
            manifest_diff: ManifestDiff::default(),
            reason: "exact retry".to_string(),
        },
    )
    .await
    .expect("first build request succeeds");

    let replay = PlatformCompositionBuildService::update_manifest_and_request_build(
        &db,
        publisher,
        &registry,
        PlatformCompositionBuildCommand {
            tenant_id,
            actor_id,
            idempotency_key,
            expected_revision: seeded.revision,
            manifest: ModulesManifest::default(),
            manifest_diff: ManifestDiff::default(),
            reason: "exact retry".to_string(),
        },
    )
    .await
    .expect("exact retry succeeds after the revision advances");

    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(replay.snapshot.revision, first.snapshot.revision);
    assert_eq!(replay.build.id, first.build.id);
    assert_eq!(
        BuildEntity::find()
            .all(&db)
            .await
            .expect("list persisted builds")
            .len(),
        1,
        "an exact retry must not enqueue another immutable build"
    );
}

#[tokio::test]
async fn successful_enqueue_keeps_canonical_composition_digest_with_its_snapshot() {
    let db = setup_db(true).await;
    let result = enqueue_default_manifest(&db).await;

    let expected_hash = PlatformCompositionService::manifest_hash(&result.snapshot.manifest)
        .expect("serialize manifest for hash comparison");
    assert_eq!(result.snapshot.manifest_hash, expected_hash);
    assert_eq!(
        result.build.manifest_snapshot,
        PlatformCompositionService::manifest_snapshot_json(&result.snapshot.manifest)
            .expect("serialize manifest for snapshot comparison")
    );
}

#[tokio::test]
async fn build_request_identity_is_distinct_from_the_composition_digest() {
    let db = setup_db(true).await;
    let result = enqueue_default_manifest(&db).await;

    let persisted_snapshot = result.build.manifest_snapshot.clone();
    let expected_snapshot =
        PlatformCompositionService::manifest_snapshot_json(&result.snapshot.manifest)
            .expect("serialize snapshot from platform state manifest");
    assert_eq!(persisted_snapshot, expected_snapshot);

    let expected_hash = rustok_api::manifest_hash::hash_manifest_snapshot(&persisted_snapshot);
    assert_eq!(result.snapshot.manifest_hash, expected_hash);
    assert_ne!(
        result.build.manifest_hash, expected_hash,
        "the build identity covers the complete immutable execution request"
    );
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
    assert_eq!(
        first.build.manifest_snapshot,
        second.build.manifest_snapshot
    );
}

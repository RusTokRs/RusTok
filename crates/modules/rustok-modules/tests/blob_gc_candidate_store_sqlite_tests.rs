use chrono::{DateTime, Duration, Utc};
use rustok_core::MigrationSource;
use rustok_modules::{
    ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS, ArtifactAdmissionStore, ArtifactBlobGcCandidateState,
    ArtifactBlobGcCandidateStore, ArtifactBlobGcClaimOutcome, ArtifactBlobGcMarkOutcome,
    ModuleInstallationError, SeaOrmArtifactInstallationStore, StagedArtifactBlob,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:blob_gc_candidate_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("sqlite in-memory connection");

    let manager = SchemaManager::new(&db);
    for migration in rustok_outbox::OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in rustok_modules::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("migration should apply");
    }
    db
}

fn fixed_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
        .expect("fixed time")
        .with_timezone(&Utc)
}

#[tokio::test]
async fn blob_gc_candidate_requires_grace_and_final_unreferenced_claim() {
    let db = setup_test_db().await;
    let store = SeaOrmArtifactInstallationStore::new(db.clone());
    let digest = format!("sha256:{}", "a".repeat(64));
    let first_seen = fixed_time();
    let grace_ends = first_seen + Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);

    let marked = store
        .mark_unreferenced_blob(&digest, first_seen, grace_ends)
        .await
        .expect("mark unreferenced blob");
    match marked {
        ArtifactBlobGcMarkOutcome::Marked(candidate) => {
            assert_eq!(candidate.state, ArtifactBlobGcCandidateState::Marked);
            assert_eq!(candidate.revision, 1);
            assert_eq!(candidate.first_seen_unreferenced_at, first_seen);
            assert_eq!(candidate.grace_ends_at, grace_ends);
        }
        other => panic!("expected marked candidate, got {other:?}"),
    }

    let early_claim = store
        .claim_unreferenced_blob_gc(&digest, first_seen + Duration::hours(1))
        .await
        .expect("early claim is evaluated");
    assert!(matches!(
        early_claim,
        ArtifactBlobGcClaimOutcome::GraceNotElapsed(_)
    ));

    // A reference that commits after the initial mark must revoke the
    // tombstone at final recheck instead of authorizing the external delete.
    let installation_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO module_artifact_installations \
         (installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
          slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
          data_owner_id, settings_instance_id, dependency_graph_revision, \
          dependency_graph_digest, dependency_lock, installed_at) \
         VALUES (?1, 'tenant', ?2, 'local', 'modules/blob_gc', 'sha256:manifest', \
                 'blob_gc', '1.0.0', 'rhai', 'rustok:module/runtime@1', ?3, 'main', '{}', \
                 ?4, ?4, 1, 'sha256:lock', '{}', ?5)"
            .to_string(),
        vec![
            installation_id.to_string().into(),
            Uuid::new_v4().to_string().into(),
            digest.clone().into(),
            Uuid::new_v4().to_string().into(),
            first_seen.to_rfc3339().into(),
        ],
    ))
    .await
    .expect("seed installation reference");
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO module_artifact_admissions \
         (stage_id, installation_id, payload_digest, media_type, size_bytes, \
          verification_evidence, status, revision, committed_at) \
         VALUES (?1, ?2, ?3, 'application/vnd.rustok.rhai', 1, '{}', 'admitted', 1, ?4)"
            .to_string(),
        vec![
            Uuid::new_v4().to_string().into(),
            installation_id.to_string().into(),
            digest.clone().into(),
            first_seen.to_rfc3339().into(),
        ],
    ))
    .await
    .expect("seed admission reference");

    let referenced_claim = store
        .claim_unreferenced_blob_gc(&digest, grace_ends)
        .await
        .expect("referenced final claim is evaluated");
    assert_eq!(
        referenced_claim,
        ArtifactBlobGcClaimOutcome::Referenced { revoked: true }
    );
    assert!(
        store
            .blob_gc_candidates()
            .await
            .expect("candidates after revocation")
            .is_empty()
    );

    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "DELETE FROM module_artifact_admissions WHERE payload_digest = ?1".to_string(),
        vec![digest.clone().into()],
    ))
    .await
    .expect("remove admission reference");

    let second_grace_ends =
        grace_ends + Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);
    assert!(matches!(
        store
            .mark_unreferenced_blob(&digest, grace_ends, second_grace_ends)
            .await
            .expect("remark unreferenced blob"),
        ArtifactBlobGcMarkOutcome::Marked(_)
    ));

    let claimed = match store
        .claim_unreferenced_blob_gc(&digest, second_grace_ends)
        .await
        .expect("claim after grace")
    {
        ArtifactBlobGcClaimOutcome::Claimed(candidate) => candidate,
        other => panic!("expected claimed candidate, got {other:?}"),
    };
    assert_eq!(claimed.state, ArtifactBlobGcCandidateState::Deleting);
    assert_eq!(claimed.revision, 2);
    assert_eq!(claimed.delete_started_at, Some(second_grace_ends));

    let concurrent_claim = store
        .claim_unreferenced_blob_gc(&digest, second_grace_ends + Duration::seconds(1))
        .await
        .expect("concurrent claim is fenced by the lease");
    assert!(matches!(
        concurrent_claim,
        ArtifactBlobGcClaimOutcome::ClaimInProgress(_)
    ));

    assert!(
        !store
            .complete_blob_gc(&digest, claimed.revision - 1)
            .await
            .expect("stale completion is rejected")
    );
    assert!(
        store
            .complete_blob_gc(&digest, claimed.revision)
            .await
            .expect("claimed completion is recorded")
    );
    assert!(
        store
            .blob_gc_candidates()
            .await
            .expect("candidates after completion")
            .is_empty()
    );

    let third_grace_ends =
        second_grace_ends + Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);
    let reserved_stage = StagedArtifactBlob {
        stage_id: Uuid::new_v4(),
        digest: digest.clone(),
        media_type: "application/vnd.rustok.rhai".to_string(),
        size_bytes: 1,
        staging_object_key: None,
    };
    store
        .reserve_blob_publication(&reserved_stage, second_grace_ends)
        .await
        .expect("reserve publication after prior collection completed");
    assert_eq!(
        store
            .mark_unreferenced_blob(&digest, second_grace_ends, third_grace_ends)
            .await
            .expect("live publication reservation blocks marking"),
        ArtifactBlobGcMarkOutcome::Referenced
    );
    assert!(
        store
            .release_blob_publication_reservation(&reserved_stage)
            .await
            .expect("release publication reservation")
    );
    assert!(matches!(
        store
            .mark_unreferenced_blob(&digest, second_grace_ends, third_grace_ends)
            .await
            .expect("mark after reservation release"),
        ArtifactBlobGcMarkOutcome::Marked(_)
    ));
    let claimed = match store
        .claim_unreferenced_blob_gc(&digest, third_grace_ends)
        .await
        .expect("claim after reservation release")
    {
        ArtifactBlobGcClaimOutcome::Claimed(candidate) => candidate,
        other => panic!("expected claimed candidate, got {other:?}"),
    };
    let blocked_stage = StagedArtifactBlob {
        stage_id: Uuid::new_v4(),
        digest: digest.clone(),
        media_type: "application/vnd.rustok.rhai".to_string(),
        size_bytes: 1,
        staging_object_key: None,
    };
    assert!(matches!(
        store
            .reserve_blob_publication(&blocked_stage, third_grace_ends)
            .await,
        Err(ModuleInstallationError::AdmissionRevisionConflict(_))
    ));
    assert!(
        store
            .complete_blob_gc(&digest, claimed.revision)
            .await
            .expect("complete after blocked reservation")
    );
    assert!(
        store
            .blob_gc_candidates()
            .await
            .expect("candidates after final completion")
            .is_empty()
    );
}

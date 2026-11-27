use chrono::Utc;
use rustok_modules::{AdmissionReverificationPolicyRevision, SeaOrmArtifactInstallationStore};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use uuid::Uuid;

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite in-memory database");
    db.execute_unprepared(
        "CREATE TABLE module_artifact_installations (
            installation_id TEXT PRIMARY KEY NOT NULL,
            scope_kind TEXT NOT NULL,
            tenant_id TEXT NULL
        )",
    )
    .await
    .expect("create installation table");
    db.execute_unprepared(
        "CREATE TABLE module_artifact_admissions (
            installation_id TEXT PRIMARY KEY NOT NULL,
            revision INTEGER NOT NULL,
            verification_evidence TEXT NOT NULL,
            status TEXT NOT NULL
        )",
    )
    .await
    .expect("create admission table");
    db
}

async fn insert_admission(db: &DatabaseConnection, installation_id: Uuid, evidence: &str) {
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO module_artifact_installations
         (installation_id, scope_kind, tenant_id)
         VALUES (?1, 'platform', NULL)"
            .to_string(),
        vec![installation_id.to_string().into()],
    ))
    .await
    .expect("insert installation");
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO module_artifact_admissions
         (installation_id, revision, verification_evidence, status)
         VALUES (?1, 1, ?2, 'admitted')"
            .to_string(),
        vec![
            installation_id.to_string().into(),
            evidence.to_string().into(),
        ],
    ))
    .await
    .expect("insert admission");
}

fn evidence_json(trust_revision: u64, capability_revision: u64) -> String {
    serde_json::json!({
        "manifest_digest": format!("sha256:{}", "a".repeat(64)),
        "payload_digest": format!("sha256:{}", "b".repeat(64)),
        "media_type": "application/vnd.rustok.rhai",
        "signer_identity": "pagination-test",
        "trust_policy_revision": trust_revision,
        "capability_policy_revision": capability_revision,
        "signature_verified": true,
        "provenance_verified": true,
        "sbom_verified": true,
        "license_policy_verified": true,
        "vulnerability_policy_verified": true,
        "evidence": [],
        "verified_at": Utc::now().to_rfc3339()
    })
    .to_string()
}

fn policy(trust_revision: u64, capability_revision: u64) -> AdmissionReverificationPolicyRevision {
    AdmissionReverificationPolicyRevision {
        trust_policy_revision: trust_revision,
        capability_policy_revision: capability_revision,
    }
}

#[tokio::test]
async fn reverification_cursor_pages_past_persistent_stale_candidates() {
    const CANDIDATE_PAGE: u32 = 32;

    let db = setup_db().await;
    for value in 1..=70_u128 {
        insert_admission(&db, Uuid::from_u128(value), "not-json").await;
    }

    let store = SeaOrmArtifactInstallationStore::new(db);
    let current_policy = policy(2, 2);

    let first = store
        .list_stale_admission_reverification_candidates_after(&current_policy, None, CANDIDATE_PAGE)
        .await
        .expect("first stale page");
    assert_eq!(
        first
            .candidates
            .iter()
            .map(|candidate| candidate.installation_id)
            .collect::<Vec<_>>(),
        (1..=32).map(Uuid::from_u128).collect::<Vec<_>>()
    );
    assert_eq!(first.next_cursor, Some(Uuid::from_u128(32)));
    assert!(first.has_more);

    // None of the first 32 rows is changed. A later pass must nevertheless
    // move on to the next stale admissions rather than retrying this prefix.
    let second = store
        .list_stale_admission_reverification_candidates_after(
            &current_policy,
            first.next_cursor,
            CANDIDATE_PAGE,
        )
        .await
        .expect("second stale page");
    assert_eq!(
        second
            .candidates
            .iter()
            .map(|candidate| candidate.installation_id)
            .collect::<Vec<_>>(),
        (33..=64).map(Uuid::from_u128).collect::<Vec<_>>()
    );
    assert!(second.has_more);

    let third = store
        .list_stale_admission_reverification_candidates_after(
            &current_policy,
            second.next_cursor,
            CANDIDATE_PAGE,
        )
        .await
        .expect("final stale page");
    assert_eq!(
        third
            .candidates
            .iter()
            .map(|candidate| candidate.installation_id)
            .collect::<Vec<_>>(),
        (65..=70).map(Uuid::from_u128).collect::<Vec<_>>()
    );
    assert!(!third.has_more);

    let end = store
        .list_stale_admission_reverification_candidates_after(
            &current_policy,
            third.next_cursor,
            CANDIDATE_PAGE,
        )
        .await
        .expect("end of stale inventory");
    assert!(end.candidates.is_empty());
    assert!(!end.has_more);
    assert!(
        store
            .list_stale_admission_reverification_candidates_after(
                &current_policy,
                Some(Uuid::nil()),
                CANDIDATE_PAGE,
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reverification_cursor_continues_after_the_bounded_fresh_scan_prefix() {
    const FRESH_ROWS: u128 = 8_200;
    const SCAN_BUDGET_CURSOR: u128 = 8_192;

    let db = setup_db().await;
    let fresh_evidence = evidence_json(1, 1);
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "WITH RECURSIVE seq(value) AS (
             SELECT 1
             UNION ALL SELECT value + 1 FROM seq WHERE value < 8200
         )
         INSERT INTO module_artifact_installations
             (installation_id, scope_kind, tenant_id)
         SELECT printf('00000000-0000-0000-0000-%012x', value), 'platform', NULL
         FROM seq"
            .to_string(),
        Vec::new(),
    ))
    .await
    .expect("insert fresh installations");
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "WITH RECURSIVE seq(value) AS (
             SELECT 1
             UNION ALL SELECT value + 1 FROM seq WHERE value < 8200
         )
         INSERT INTO module_artifact_admissions
             (installation_id, revision, verification_evidence, status)
         SELECT printf('00000000-0000-0000-0000-%012x', value), 1, ?1, 'admitted'
         FROM seq"
            .to_string(),
        vec![fresh_evidence.into()],
    ))
    .await
    .expect("insert fresh admissions");

    let stale_id = Uuid::from_u128(FRESH_ROWS + 100);
    insert_admission(&db, stale_id, &evidence_json(2, 1)).await;

    let store = SeaOrmArtifactInstallationStore::new(db);
    let current_policy = policy(1, 1);
    let first = store
        .list_stale_admission_reverification_candidates_after(&current_policy, None, 32)
        .await
        .expect("bounded fresh-prefix scan");
    assert!(first.candidates.is_empty());
    assert_eq!(first.next_cursor, Some(Uuid::from_u128(SCAN_BUDGET_CURSOR)));
    assert!(first.has_more);

    let second = store
        .list_stale_admission_reverification_candidates_after(
            &current_policy,
            first.next_cursor,
            32,
        )
        .await
        .expect("continue beyond scan budget");
    assert_eq!(second.candidates.len(), 1);
    assert_eq!(second.candidates[0].installation_id, stale_id);
    assert_eq!(second.next_cursor, Some(stale_id));
    assert!(!second.has_more);
}

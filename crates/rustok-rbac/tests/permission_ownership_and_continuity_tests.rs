use rustok_api::{
    ArtifactPermissionLocalization, ArtifactPermissionRegistration,
    ArtifactPermissionRegistrationPort, ArtifactPermissionScope,
    PermissionContinuityEvaluationRequest, ReleasePermissionAdmissionRequest,
    ScopedPermissionProjectionRequest, compute_canonical_authorization_fingerprint,
};
use rustok_core::MigrationSource;
use rustok_rbac::{RbacArtifactPermissionCatalog, RbacModule};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect sqlite memory");

    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("enable foreign keys");

    for stmt in [
        "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL);",
        "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL);",
        "CREATE TABLE permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL);",
        "CREATE TABLE user_roles (user_id TEXT NOT NULL, role_id TEXT NOT NULL);",
        "CREATE TABLE role_permissions (role_id TEXT NOT NULL, permission_id TEXT NOT NULL);",
    ] {
        db.execute_unprepared(stmt)
            .await
            .expect("create parent tables");
    }

    let manager = SchemaManager::new(&db);
    for migration in RbacModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("apply migration");
    }

    db
}

fn sample_permissions(slug: &str) -> Vec<ArtifactPermissionRegistration> {
    vec![
        ArtifactPermissionRegistration {
            key: format!("{slug}.read"),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Read access".to_string(),
                description: "Allows read access".to_string(),
            }],
        },
        ArtifactPermissionRegistration {
            key: format!("{slug}.write"),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Write access".to_string(),
                description: "Allows write access".to_string(),
            }],
        },
    ]
}

#[tokio::test]
async fn test_inert_admission_persists_definitions_without_installation() {
    let db = setup_database().await;
    let catalog = RbacArtifactPermissionCatalog::new(db.clone());

    let release_digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let permissions = sample_permissions("sample");

    catalog
        .admit_release_permissions(ReleasePermissionAdmissionRequest {
            module_slug: "sample".to_string(),
            release_digest: release_digest.clone(),
            permissions: permissions.clone(),
        })
        .await
        .expect("admit release permissions");

    // Verify release definitions exist in DB
    let count_definitions: i64 = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            format!(
                "SELECT COUNT(*) as cnt FROM rbac_artifact_release_permission_definitions WHERE release_digest = '{release_digest}'"
            ),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "cnt")
        .unwrap();
    assert_eq!(count_definitions, 2);

    // Verify NO installation rows exist yet
    let count_installations: i64 = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) as cnt FROM rbac_artifact_permission_installations".to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "cnt")
        .unwrap();
    assert_eq!(count_installations, 0);

    // Verify NO scoped permission rows exist yet
    let count_scoped: i64 = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) as cnt FROM rbac_artifact_permission_definitions".to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "cnt")
        .unwrap();
    assert_eq!(count_scoped, 0);
}

#[tokio::test]
async fn test_multi_tenant_independent_scoped_projections() {
    let db = setup_database().await;
    let catalog = RbacArtifactPermissionCatalog::new(db.clone());

    let release_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string();
    let permissions = sample_permissions("isolated");

    catalog
        .admit_release_permissions(ReleasePermissionAdmissionRequest {
            module_slug: "isolated".to_string(),
            release_digest: release_digest.clone(),
            permissions,
        })
        .await
        .expect("admit");

    // Tenant 1 installation
    let tenant1_id = Uuid::new_v4();
    let install1_id = Uuid::new_v4();
    catalog
        .project_scoped_permissions(ScopedPermissionProjectionRequest {
            scope: ArtifactPermissionScope::Tenant { tenant_id: tenant1_id },
            installation_id: install1_id,
            module_slug: "isolated".to_string(),
            release_digest: release_digest.clone(),
        })
        .await
        .expect("project tenant 1");

    // Tenant 2 installation
    let tenant2_id = Uuid::new_v4();
    let install2_id = Uuid::new_v4();
    catalog
        .project_scoped_permissions(ScopedPermissionProjectionRequest {
            scope: ArtifactPermissionScope::Tenant { tenant_id: tenant2_id },
            installation_id: install2_id,
            module_slug: "isolated".to_string(),
            release_digest: release_digest.clone(),
        })
        .await
        .expect("project tenant 2");

    let count_tenant1: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) as cnt FROM rbac_artifact_permission_definitions WHERE scope_key = ?1 AND installation_id = ?2",
            vec![format!("tenant:{tenant1_id}").into(), install1_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "cnt")
        .unwrap();
    assert_eq!(count_tenant1, 2);

    let count_tenant2: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) as cnt FROM rbac_artifact_permission_definitions WHERE scope_key = ?1 AND installation_id = ?2",
            vec![format!("tenant:{tenant2_id}").into(), install2_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "cnt")
        .unwrap();
    assert_eq!(count_tenant2, 2);
}

#[tokio::test]
async fn test_authorization_fingerprint_invariance_under_display_text_edits() {
    let v1_permissions = sample_permissions("demo");
    let mut v2_permissions = sample_permissions("demo");

    // Change labels and descriptions in v2
    v2_permissions[0].localizations[0].label = "Completely different label".to_string();
    v2_permissions[0].localizations[0].description = "Different descriptive text".to_string();
    v2_permissions[1].localizations[0].label = "Changed write label".to_string();

    let v1_fingerprint = compute_canonical_authorization_fingerprint(&v1_permissions);
    let v2_fingerprint = compute_canonical_authorization_fingerprint(&v2_permissions);

    // Fingerprints MUST be identical because authorization fields did not change!
    assert_eq!(v1_fingerprint, v2_fingerprint);

    // Modify a permission key in v3
    let mut v3_permissions = v1_permissions.clone();
    v3_permissions[0].key = "demo.read.extended".to_string();
    let v3_fingerprint = compute_canonical_authorization_fingerprint(&v3_permissions);

    // Fingerprint MUST change when authorization key changes
    assert_ne!(v1_fingerprint, v3_fingerprint);
}

#[tokio::test]
async fn test_permission_continuity_evaluates_approval_and_diff() {
    let db = setup_database().await;
    let catalog = RbacArtifactPermissionCatalog::new(db);

    let pred = sample_permissions("mod");
    let mut cand = sample_permissions("mod");
    // Add a new permission to candidate
    cand.push(ArtifactPermissionRegistration {
        key: "mod.admin".to_string(),
        localizations: vec![ArtifactPermissionLocalization {
            locale: "en".to_string(),
            label: "Admin".to_string(),
            description: "Admin power".to_string(),
        }],
    });

    let receipt = catalog
        .evaluate_permission_continuity(PermissionContinuityEvaluationRequest {
            scope: ArtifactPermissionScope::Platform,
            predecessor_release_digest: "sha256:pred".to_string(),
            candidate_release_digest: "sha256:cand".to_string(),
            predecessor_permissions: pred,
            candidate_permissions: cand,
            expected_rbac_epoch: 0,
        })
        .await
        .expect("evaluate continuity");

    // Key set modified -> approved MUST be false, requiring explicit operator approval
    assert!(!receipt.approved);
    assert_eq!(receipt.diff.unchanged_keys.len(), 2);
    assert_eq!(receipt.diff.added_keys, vec!["mod.admin"]);
    assert!(receipt.diff.removed_dormant_keys.is_empty());
}

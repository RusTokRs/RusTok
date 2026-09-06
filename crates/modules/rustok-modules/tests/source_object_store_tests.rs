use std::{
    fs,
    path::PathBuf,
};

use rustok_modules::{
    ModuleInstallationScope, SourceObjectError, SourceObjectStore,
    migrations,
};
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("rustok-test-cas-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");
    
    struct Migrator;
    #[async_trait::async_trait]
    impl MigratorTrait for Migrator {
        fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
            migrations::migrations()
        }
    }
    
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");
    db
}

#[tokio::test]
async fn test_source_blob_create_only_and_deduplication() {
    let db = setup_test_db().await;
    let cas_dir = TestTempDir::new();
    let cas_root = cas_dir.path().to_path_buf();
    let store = SourceObjectStore::new(db, cas_root.clone()).expect("store");

    let preparation_1 = Uuid::new_v4();
    let scope_1 = ModuleInstallationScope::Platform;
    let payload = b"{\"rhai_script\": \"let x = 42;\"}";
    let digest = sha256_digest(payload);
    let hex = &digest[7..];

    // 1. Publish blob
    let receipt1 = store
        .publish_source_blob(
            preparation_1,
            &scope_1,
            "application/vnd.rustok.source.workspace.v1+json",
            &digest,
            payload,
            None,
        )
        .await
        .expect("publish");

    assert_eq!(receipt1.preparation_id, preparation_1);
    assert_eq!(receipt1.source_digest, digest);
    assert_eq!(receipt1.byte_length, payload.len() as u64);

    // Verify file exists directly under <cas_root>/<hex> without .tar extension
    let blob_path = cas_root.join(hex);
    assert!(blob_path.exists(), "blob must exist at canonical path");
    let content = fs::read(&blob_path).expect("read");
    assert_eq!(content, payload);

    // 2. Idempotent replay for same preparation returns existing receipt
    let receipt1_replay = store
        .publish_source_blob(
            preparation_1,
            &scope_1,
            "application/vnd.rustok.source.workspace.v1+json",
            &digest,
            payload,
            None,
        )
        .await
        .expect("replay");
    assert_eq!(receipt1.source_receipt_id, receipt1_replay.source_receipt_id);

    // 3. Different preparation sharing same blob converges without overwrite
    let preparation_2 = Uuid::new_v4();
    let receipt2 = store
        .publish_source_blob(
            preparation_2,
            &scope_1,
            "application/vnd.rustok.source.workspace.v1+json",
            &digest,
            payload,
            None,
        )
        .await
        .expect("publish 2");
    assert_eq!(receipt2.preparation_id, preparation_2);
    assert_ne!(receipt1.source_receipt_id, receipt2.source_receipt_id);

    // Read back via store
    let read_back = store.get_source_blob(&digest).await.expect("get_blob");
    assert_eq!(read_back, payload);
}

#[tokio::test]
async fn test_source_blob_digest_mismatch_fails_closed() {
    let db = setup_test_db().await;
    let cas_dir = TestTempDir::new();
    let store = SourceObjectStore::new(db, cas_dir.path().to_path_buf()).expect("store");

    let preparation = Uuid::new_v4();
    let scope = ModuleInstallationScope::Platform;
    let payload = b"legitimate content";
    let fake_digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let result = store
        .publish_source_blob(
            preparation,
            &scope,
            "application/x-tar",
            fake_digest,
            payload,
            None,
        )
        .await;

    match result {
        Err(SourceObjectError::DigestMismatch { expected, actual }) => {
            assert_eq!(expected, fake_digest);
            assert_eq!(actual, sha256_digest(payload));
        }
        other => panic!("expected DigestMismatch, got {other:?}"),
    }

    // Verify no file or receipt was committed
    let hex = &fake_digest[7..];
    assert!(!cas_dir.path().join(hex).exists());
}

#[tokio::test]
async fn test_tenant_isolation_and_rls() {
    let db = setup_test_db().await;
    let cas_dir = TestTempDir::new();
    let store = SourceObjectStore::new(db, cas_dir.path().to_path_buf()).expect("store");

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let prep_a = Uuid::new_v4();

    let payload = b"private tenant code";
    let digest = sha256_digest(payload);

    let receipt_a = store
        .publish_source_blob(
            prep_a,
            &ModuleInstallationScope::Tenant { tenant_id: tenant_a },
            "application/vnd.rustok.source.workspace.v1+json",
            &digest,
            payload,
            None,
        )
        .await
        .expect("publish tenant a");

    // Tenant A can access its receipt
    let read_a = store
        .get_receipt(
            receipt_a.source_receipt_id,
            Some(&ModuleInstallationScope::Tenant { tenant_id: tenant_a }),
        )
        .await
        .expect("read a");
    assert!(read_a.is_some());

    // Tenant B cannot access Tenant A's receipt
    let read_b = store
        .get_receipt(
            receipt_a.source_receipt_id,
            Some(&ModuleInstallationScope::Tenant { tenant_id: tenant_b }),
        )
        .await;

    match read_b {
        Err(SourceObjectError::UnauthorizedTenant { receipt_id, target_tenant }) => {
            assert_eq!(receipt_id, receipt_a.source_receipt_id);
            assert_eq!(target_tenant, tenant_b);
        }
        other => panic!("expected UnauthorizedTenant, got {other:?}"),
    }
}

#[tokio::test]
async fn test_retention_holds_lifecycle() {
    let db = setup_test_db().await;
    let cas_dir = TestTempDir::new();
    let store = SourceObjectStore::new(db, cas_dir.path().to_path_buf()).expect("store");

    let digest = sha256_digest(b"active deployment source");

    assert!(!store.is_held(&digest).await.expect("is_held"));

    let hold_id = store
        .add_retention_hold(&digest, "transition_coordinator", "active predecessor hold", None)
        .await
        .expect("add hold");

    assert!(store.is_held(&digest).await.expect("is_held"));

    store.release_retention_hold(hold_id).await.expect("release hold");

    assert!(!store.is_held(&digest).await.expect("is_held"));
}

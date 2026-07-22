use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use object_store::{ObjectStoreExt, signer::Signer};
use rustok_media::{
    AssetState, BlobState, CreateRenditionInput, ImageBackground, ImageOutputFormat, ImageRecipe,
    MediaError, MediaService, PrepareUploadSessionInput, QuarterTurn, RenditionState, UploadInput,
    UploadState,
    entities::{asset, blob, rendition, upload_session},
    migrations,
};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DbBackend, EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Debug)]
struct TestSigner;

#[async_trait]
impl Signer for TestSigner {
    async fn signed_url(
        &self,
        method: axum::http::Method,
        path: &object_store::path::Path,
        _expires_in: Duration,
    ) -> object_store::Result<url::Url> {
        Ok(format!("https://upload.invalid/{path}?method={method}")
            .parse()
            .expect("test URL should parse"))
    }
}

async fn test_runtime() -> (
    sea_orm::DatabaseConnection,
    StorageRuntime,
    tempfile::TempDir,
) {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite test database should connect");
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .expect("tenant fixture table should be created");
    database
        .execute_unprepared("CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .expect("user fixture table should be created");

    let manager = SchemaManager::new(&database);
    for migration in migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("Media migration should apply to SQLite");
    }

    let directory = tempfile::tempdir().expect("temporary object directory should be created");
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.path().display().to_string(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .expect("local object store should initialize");

    (database, storage, directory)
}

async fn seed_tenant(database: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .expect("tenant fixture should be inserted");
}

fn png_upload(tenant_id: Uuid) -> UploadInput {
    let image = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
        24,
        12,
        image::Rgba([10, 20, 30, 255]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    UploadInput {
        tenant_id,
        uploaded_by: None,
        original_name: "hero.png".to_string(),
        content_type: "image/png".to_string(),
        data: Bytes::from(bytes.into_inner()),
    }
}

fn webp_recipe() -> ImageRecipe {
    ImageRecipe {
        crop: None,
        resize: None,
        rotate: QuarterTurn::None,
        flip_horizontal: false,
        flip_vertical: false,
        output: ImageOutputFormat::Webp,
        quality: 82,
        background: ImageBackground::default(),
        strip_metadata: true,
    }
}

#[tokio::test]
async fn upload_persists_asset_and_immutable_blob_then_deletes_through_tombstones() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage.clone());

    let item = service
        .upload(png_upload(tenant_id))
        .await
        .expect("upload should succeed");
    assert!(
        item.storage_path
            .starts_with(&format!("media/objects/tenants/{tenant_id}/"))
    );

    let stored_asset = asset::Entity::find_by_id(item.id)
        .one(&database)
        .await
        .expect("asset query should succeed")
        .expect("asset should exist");
    assert_eq!(stored_asset.lifecycle_state, AssetState::Active.as_str());
    let stored_blob = blob::Entity::find()
        .filter(blob::Column::AssetId.eq(item.id))
        .one(&database)
        .await
        .expect("blob query should succeed")
        .expect("blob should exist");
    assert_eq!(stored_blob.state, BlobState::Ready.as_str());
    assert_eq!(stored_blob.checksum_sha256.len(), 64);

    service
        .delete(tenant_id, item.id)
        .await
        .expect("delete request should reconcile immediately");

    let deleted_asset = asset::Entity::find_by_id(item.id)
        .one(&database)
        .await
        .expect("deleted asset query should succeed")
        .expect("asset tombstone should remain");
    let deleted_blob = blob::Entity::find_by_id(stored_blob.id)
        .one(&database)
        .await
        .expect("deleted blob query should succeed")
        .expect("blob tombstone should remain");
    assert_eq!(deleted_asset.lifecycle_state, AssetState::Deleted.as_str());
    assert_eq!(deleted_blob.state, BlobState::Deleted.as_str());
    assert!(matches!(
        service.get(tenant_id, item.id).await,
        Err(MediaError::NotFound(_))
    ));
}

#[tokio::test]
async fn reconciliation_marks_missing_blob_and_preserves_database_evidence() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage.clone());
    let item = service
        .upload(png_upload(tenant_id))
        .await
        .expect("upload should succeed");

    storage
        .objects
        .delete(&object_store::path::Path::from(item.storage_path.as_str()))
        .await
        .expect("fixture object should be removed");
    let report = service
        .reconcile_storage(tenant_id, 100)
        .await
        .expect("reconciliation should succeed");
    assert_eq!(report.missing_marked, 1);

    let failed_asset = asset::Entity::find_by_id(item.id)
        .one(&database)
        .await
        .expect("asset query should succeed")
        .expect("failed asset evidence should remain");
    let failed_blob = blob::Entity::find()
        .filter(blob::Column::AssetId.eq(item.id))
        .one(&database)
        .await
        .expect("blob query should succeed")
        .expect("failed blob evidence should remain");
    assert_eq!(failed_asset.lifecycle_state, AssetState::Failed.as_str());
    assert_eq!(failed_blob.state, BlobState::Failed.as_str());
    assert_eq!(failed_blob.reconcile_attempts, 1);
    assert!(failed_blob.last_error.is_some());
}

#[tokio::test]
async fn rendition_is_content_addressed_by_source_and_recipe_and_reuses_ready_result() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage.clone());
    let asset = service
        .upload(png_upload(tenant_id))
        .await
        .expect("upload should succeed");

    let create = || CreateRenditionInput {
        tenant_id,
        asset_id: asset.id,
        purpose: "card-thumbnail".to_string(),
        recipe: webp_recipe(),
    };
    let first = service
        .create_rendition(create())
        .await
        .expect("rendition should be created");
    let second = service
        .create_rendition(create())
        .await
        .expect("ready rendition should be reused");

    assert_eq!(first.id, second.id);
    assert_eq!(first.result_blob_id, second.result_blob_id);
    assert_eq!((first.width, first.height), (24, 12));
    assert_eq!(first.mime_type, "image/webp");
    assert!(
        first
            .storage_path
            .starts_with(&format!("media/objects/tenants/{tenant_id}/"))
    );
    storage
        .objects
        .head(&object_store::path::Path::from(first.storage_path.as_str()))
        .await
        .expect("rendition object should exist");

    assert_eq!(
        rendition::Entity::find()
            .all(&database)
            .await
            .expect("renditions should query")
            .len(),
        1
    );
    let stored = rendition::Entity::find_by_id(first.id)
        .one(&database)
        .await
        .expect("rendition should query")
        .expect("rendition should exist");
    assert_eq!(stored.state, RenditionState::Ready.as_str());
    assert_eq!(
        blob::Entity::find()
            .filter(blob::Column::AssetId.eq(asset.id))
            .all(&database)
            .await
            .expect("blobs should query")
            .len(),
        2
    );
}

#[tokio::test]
async fn presigned_session_finalization_is_idempotent_and_cleans_staging() {
    let (database, storage, _directory) = test_runtime().await;
    let storage = storage.with_signer(Arc::new(TestSigner));
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage.clone());
    let upload = png_upload(tenant_id);
    let prepared = service
        .prepare_upload_session(PrepareUploadSessionInput {
            tenant_id,
            actor_id: None,
            original_name: upload.original_name.clone(),
            content_type: upload.content_type.clone(),
            content_length: Some(upload.data.len() as u64),
            expires_in: Duration::from_secs(300),
        })
        .await
        .expect("upload session should be prepared");
    assert!(prepared.endpoint.contains("method=PUT"));

    let session = upload_session::Entity::find_by_id(prepared.id)
        .one(&database)
        .await
        .expect("session should query")
        .expect("session should exist");
    storage
        .objects
        .put(
            &object_store::path::Path::from(session.staging_key.as_str()),
            upload.data.clone().into(),
        )
        .await
        .expect("staging object should be written");

    let first = service
        .complete_upload_session(tenant_id, prepared.id)
        .await
        .expect("session should finalize");
    let second = service
        .complete_upload_session(tenant_id, prepared.id)
        .await
        .expect("completed session should be idempotent");
    assert_eq!(first.id, second.id);

    let completed = upload_session::Entity::find_by_id(prepared.id)
        .one(&database)
        .await
        .expect("completed session should query")
        .expect("completed session should remain");
    assert_eq!(completed.state, UploadState::Completed.as_str());
    assert!(completed.completed_at.is_some());
    assert!(completed.staging_deleted_at.is_some());
    assert!(
        storage
            .objects
            .head(&object_store::path::Path::from(
                completed.staging_key.as_str()
            ))
            .await
            .is_err()
    );
    assert_eq!(
        asset::Entity::find()
            .filter(asset::Column::UploadSessionId.eq(prepared.id))
            .all(&database)
            .await
            .expect("session assets should query")
            .len(),
        1
    );
}

#[tokio::test]
async fn reconciliation_expires_upload_session_and_removes_staging_object() {
    let (database, storage, _directory) = test_runtime().await;
    let storage = storage.with_signer(Arc::new(TestSigner));
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage.clone());
    let upload = png_upload(tenant_id);
    let prepared = service
        .prepare_upload_session(PrepareUploadSessionInput {
            tenant_id,
            actor_id: None,
            original_name: upload.original_name,
            content_type: upload.content_type,
            content_length: Some(upload.data.len() as u64),
            expires_in: Duration::from_secs(300),
        })
        .await
        .expect("upload session should prepare");
    let session = upload_session::Entity::find_by_id(prepared.id)
        .one(&database)
        .await
        .expect("session should query")
        .expect("session should exist");
    storage
        .objects
        .put(
            &object_store::path::Path::from(session.staging_key.as_str()),
            upload.data.into(),
        )
        .await
        .expect("staging object should write");
    upload_session::Entity::update_many()
        .col_expr(
            upload_session::Column::ExpiresAt,
            sea_orm::sea_query::Expr::value(
                (chrono::Utc::now() - chrono::Duration::minutes(1)).fixed_offset(),
            ),
        )
        .filter(upload_session::Column::Id.eq(prepared.id))
        .exec(&database)
        .await
        .expect("session expiry should update");

    let report = service
        .reconcile_storage(tenant_id, 100)
        .await
        .expect("reconciliation should succeed");
    assert_eq!(report.upload_sessions_expired, 1);
    assert_eq!(report.staging_objects_deleted, 1);
    let expired = upload_session::Entity::find_by_id(prepared.id)
        .one(&database)
        .await
        .expect("expired session should query")
        .expect("expired session should remain");
    assert_eq!(expired.state, UploadState::Expired.as_str());
    assert!(expired.staging_deleted_at.is_some());
}

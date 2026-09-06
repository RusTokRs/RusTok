use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use object_store::{ObjectStoreExt, signer::Signer};
use rustok_api::{
    Action, Permission, PortActor, PortContext, PortErrorKind, Resource, TenantLocale,
};
use rustok_media::{
    ApplyExactMediaTranslationInput, AssetState, BlobState, CreateRenditionInput, ImageBackground,
    ImageOutputFormat, ImageRecipe, MediaError, MediaService, MediaTranslationTargetProvider,
    PrepareUploadSessionInput, QuarterTurn, RenditionState, UploadInput, UploadState,
    UpsertTranslationInput,
    entities::{asset, blob, media_translation, rendition, translation_change, upload_session},
    migrations,
};
use rustok_outbox::{SysEvents, SysEventsMigration};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, ReadTranslationResourceRequest,
    TranslationFieldPatch, TranslationPatchRequest, TranslationTargetCapability,
    TranslationTargetChangesRequest, TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DbBackend, EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
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
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply to SQLite");
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
        .execute_raw(Statement::from_sql_and_values(
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
    service
        .upsert_translation(
            tenant_id,
            item.id,
            UpsertTranslationInput {
                locale: "en".to_string(),
                title: Some("Missing source asset".to_string()),
                alt_text: None,
                caption: None,
            },
        )
        .await
        .expect("translation evidence fixture should be created");
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
    service
        .upsert_translation(
            tenant_id,
            item.id,
            UpsertTranslationInput {
                locale: "en".to_string(),
                title: Some("Missing source asset".to_string()),
                alt_text: None,
                caption: None,
            },
        )
        .await
        .expect("translation evidence fixture should be created");

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
    let unavailable_changes = translation_change::Entity::find()
        .filter(translation_change::Column::TenantId.eq(tenant_id))
        .filter(translation_change::Column::AssetId.eq(item.id))
        .filter(translation_change::Column::Lifecycle.eq("unavailable"))
        .all(&database)
        .await
        .expect("unavailable translation changes should query");
    assert_eq!(unavailable_changes.len(), 1);
    assert_eq!(unavailable_changes[0].operation, "unavailable");
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

#[tokio::test]
async fn exact_translation_apply_checks_source_and_target_revisions_atomically() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database, storage);
    let media = service
        .upload(png_upload(tenant_id))
        .await
        .expect("media fixture should upload");
    let source = service
        .upsert_translation(
            tenant_id,
            media.id,
            UpsertTranslationInput {
                locale: "en".to_string(),
                title: Some("Hero".to_string()),
                alt_text: Some("Hero image".to_string()),
                caption: None,
            },
        )
        .await
        .expect("exact source should be created");
    assert_eq!(source.revision, 1);
    let resource_revision = service
        .translation_resource_revision(tenant_id, media.id)
        .await
        .expect("media resource revision should be available");

    let target_values = || {
        UpsertTranslationInput {
            locale: "fr".to_string(),
            title: Some("Héros".to_string()),
            alt_text: Some("Image du héros".to_string()),
            caption: None,
        }
        .normalize()
        .expect("target values should normalize")
    };
    let applied = service
        .apply_exact_translation(
            tenant_id,
            media.id,
            ApplyExactMediaTranslationInput {
                source_locale: rustok_api::TenantLocale::new("en").unwrap(),
                target: target_values(),
                expected_resource_revision: resource_revision.clone(),
                expected_source_revision: 1,
                expected_target_revision: None,
            },
        )
        .await
        .expect("first exact target should apply");
    assert_eq!(applied.revision, 1);

    let stale_target = service
        .apply_exact_translation(
            tenant_id,
            media.id,
            ApplyExactMediaTranslationInput {
                source_locale: rustok_api::TenantLocale::new("en").unwrap(),
                target: target_values(),
                expected_resource_revision: resource_revision.clone(),
                expected_source_revision: 1,
                expected_target_revision: None,
            },
        )
        .await
        .expect_err("missing target revision must fail after target creation");
    assert!(matches!(
        stale_target,
        MediaError::TranslationRevisionConflict { .. }
    ));

    let source = service
        .upsert_translation(
            tenant_id,
            media.id,
            UpsertTranslationInput {
                locale: "en".to_string(),
                title: Some("Updated hero".to_string()),
                alt_text: Some("Updated hero image".to_string()),
                caption: None,
            },
        )
        .await
        .expect("source edit should advance its revision");
    assert_eq!(source.revision, 2);

    let stale_source = service
        .apply_exact_translation(
            tenant_id,
            media.id,
            ApplyExactMediaTranslationInput {
                source_locale: rustok_api::TenantLocale::new("en").unwrap(),
                target: target_values(),
                expected_resource_revision: resource_revision.clone(),
                expected_source_revision: 1,
                expected_target_revision: Some(1),
            },
        )
        .await
        .expect_err("stale source revision must fail");
    assert!(matches!(
        stale_source,
        MediaError::TranslationRevisionConflict { .. }
    ));

    let reapplied = service
        .apply_exact_translation(
            tenant_id,
            media.id,
            ApplyExactMediaTranslationInput {
                source_locale: rustok_api::TenantLocale::new("en").unwrap(),
                target: target_values(),
                expected_resource_revision: resource_revision,
                expected_source_revision: 2,
                expected_target_revision: Some(1),
            },
        )
        .await
        .expect("current source and target revisions should apply");
    assert_eq!(reapplied.revision, 2);
}

#[tokio::test]
async fn translation_write_rolls_back_when_owner_event_cannot_persist() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = MediaService::new(database.clone(), storage);
    let media = service
        .upload(png_upload(tenant_id))
        .await
        .expect("media fixture should upload");
    database
        .execute_unprepared("DROP TABLE sys_events")
        .await
        .expect("outbox table should be removable for the failure fixture");

    let error = service
        .upsert_translation(
            tenant_id,
            media.id,
            UpsertTranslationInput {
                locale: "en-US".to_string(),
                title: Some("Hero".to_string()),
                alt_text: None,
                caption: None,
            },
        )
        .await
        .expect_err("translation write must fail when owner event persistence fails");
    assert!(matches!(error, MediaError::TranslationEvent(_)));
    assert!(
        media_translation::Entity::find()
            .filter(media_translation::Column::TenantId.eq(tenant_id))
            .all(&database)
            .await
            .expect("translation rows should query")
            .is_empty()
    );
    assert!(
        translation_change::Entity::find()
            .filter(translation_change::Column::TenantId.eq(tenant_id))
            .all(&database)
            .await
            .expect("translation change rows should query")
            .is_empty()
    );
}

#[tokio::test]
async fn translation_target_provider_applies_and_replays_one_exact_locale_patch() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = Arc::new(MediaService::new(database.clone(), storage));
    let media = service
        .upload(png_upload(tenant_id))
        .await
        .expect("media fixture should upload");
    service
        .upsert_translation(
            tenant_id,
            media.id,
            UpsertTranslationInput {
                locale: "en-US".to_string(),
                title: Some("Hero".to_string()),
                alt_text: Some("Hero image".to_string()),
                caption: None,
            },
        )
        .await
        .expect("exact source should be created");
    let provider = MediaTranslationTargetProvider::new(service.clone());
    let read_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en-US",
        "translation-read",
    )
    .with_deadline(Duration::from_secs(5));
    let source_change_page = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await
        .expect("direct Media translation writes should publish change evidence");
    assert_eq!(source_change_page.changes.len(), 1);
    let source_change_cursor = source_change_page
        .next_cursor
        .expect("direct Media translation change must return a checkpoint cursor");

    let page = provider
        .list_resources(
            read_context.clone(),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("Media resources should be discoverable");
    assert_eq!(page.resources.len(), 1);
    let identity = page.resources[0].identity.clone();
    let snapshot = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: identity.clone(),
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("exact Media source should be readable");
    let title = snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == "title")
        .expect("title field should be exposed");
    let patch = TranslationPatchRequest {
        identity,
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: FieldKey::new("title").unwrap(),
            value: "Héros".to_string(),
            expected_source_hash: title.source_hash.clone(),
        }],
        proposal_id: "proposal-media-1".to_string(),
        approval_receipt_id: "approval-media-1".to_string(),
    };
    let validation = provider
        .validate_patch(read_context.clone(), patch.clone())
        .await
        .expect("Media patch validation should complete");
    assert!(validation.accepted);

    let apply_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en-US",
        "translation-apply",
    )
    .with_idempotency_key("media-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let first_receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("Media patch should apply");
    assert_eq!(first_receipt.target_revision.as_str(), "1");
    assert_eq!(
        first_receipt.applied_field_keys,
        vec![FieldKey::new("title").unwrap()]
    );
    let replay_receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same idempotency request should replay");
    assert_eq!(replay_receipt, first_receipt);
    let recovery_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "en-US",
        "translation-apply-recovery",
    )
    .with_claim(Permission::new(Resource::Media, Action::Update).to_string())
    .with_role("manager")
    .with_idempotency_key("media-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let recovered_receipt = provider
        .apply_patch(recovery_context, patch.clone())
        .await
        .expect("authorized recovery actor should reconcile the same owner mutation");
    assert_eq!(recovered_receipt, first_receipt);
    let mut conflicting_patch = patch;
    conflicting_patch.proposal_id = "proposal-media-2".to_string();
    let conflict = provider
        .apply_patch(apply_context, conflicting_patch)
        .await
        .expect_err("same idempotency key must reject a different request");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "outbox.operation_receipt_conflict");
    let events = SysEvents::find()
        .all(&database)
        .await
        .expect("translation target event should query");
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .all(|event| event.event_type == "translation.target.changed")
    );
    let change_page = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: Some(source_change_cursor),
                limit: 10,
            },
        )
        .await
        .expect("Media translation changes should be readable");
    assert_eq!(change_page.changes.len(), 1);
    assert_eq!(change_page.changes[0].identity, page.resources[0].identity);
    assert_eq!(
        change_page.changes[0].resource_revision,
        first_receipt.resource_revision
    );
    let change_cursor = change_page
        .next_cursor
        .expect("non-empty change page must return a checkpoint cursor");
    let after_change = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: Some(change_cursor),
                limit: 10,
            },
        )
        .await
        .expect("Media change cursor should resume after the checkpoint");
    assert!(after_change.changes.is_empty());
    assert!(after_change.next_cursor.is_none());
    let foreign_tenant_page = provider
        .read_changes(
            PortContext::new(
                Uuid::new_v4().to_string(),
                PortActor::system(),
                "en-US",
                "translation-change-tenant-isolation",
            )
            .with_deadline(Duration::from_secs(5)),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await
        .expect("tenant-isolated Media change query should complete");
    assert!(foreign_tenant_page.changes.is_empty());
    assert!(foreign_tenant_page.next_cursor.is_none());

    let updated = provider
        .read_resource(
            read_context,
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("applied target should be readable");
    assert_eq!(
        updated
            .fields
            .iter()
            .find(|field| field.descriptor.key.as_str() == "title")
            .and_then(|field| field.exact_target_value.as_deref()),
        Some("Héros")
    );
}

#[tokio::test]
async fn translation_target_progress_counts_only_exact_locale_values_and_source_eligible_assets() {
    let (database, storage, _directory) = test_runtime().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let service = Arc::new(MediaService::new(database, storage));

    let first = service
        .upload(png_upload(tenant_id))
        .await
        .expect("first media fixture should upload");
    service
        .upsert_translation(
            tenant_id,
            first.id,
            UpsertTranslationInput {
                locale: "en-US".to_string(),
                title: Some("Hero".to_string()),
                alt_text: Some("Hero image".to_string()),
                caption: None,
            },
        )
        .await
        .expect("first exact source should be created");
    service
        .upsert_translation(
            tenant_id,
            first.id,
            UpsertTranslationInput {
                locale: "fr".to_string(),
                title: Some("Héros".to_string()),
                alt_text: None,
                caption: None,
            },
        )
        .await
        .expect("first exact target should be created");

    let second = service
        .upload(png_upload(tenant_id))
        .await
        .expect("second media fixture should upload");
    service
        .upsert_translation(
            tenant_id,
            second.id,
            UpsertTranslationInput {
                locale: "en-US".to_string(),
                title: None,
                alt_text: None,
                caption: Some("Source caption".to_string()),
            },
        )
        .await
        .expect("second exact source should be created");

    let target_only = service
        .upload(png_upload(tenant_id))
        .await
        .expect("target-only media fixture should upload");
    service
        .upsert_translation(
            tenant_id,
            target_only.id,
            UpsertTranslationInput {
                locale: "fr".to_string(),
                title: Some("Target without requested source".to_string()),
                alt_text: Some("Target-only alt text".to_string()),
                caption: Some("Target-only caption".to_string()),
            },
        )
        .await
        .expect("target-only exact locale should be created");

    let provider = MediaTranslationTargetProvider::new(service.clone());
    assert!(
        provider
            .descriptor()
            .capabilities
            .contains(&TranslationTargetCapability::AggregateProgress)
    );
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en-US",
        "translation-progress",
    )
    .with_deadline(Duration::from_secs(5));
    let exact = provider
        .read_progress(
            context.clone(),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("exact Media progress should aggregate");
    assert_eq!(exact.resources, 2);
    assert_eq!(exact.required_units, 0);
    assert_eq!(exact.optional_units, 6);
    assert_eq!(exact.exact_optional_units, 1);
    assert_eq!(exact.complete_resources, 2);
    assert!(exact.owner_change_cursor.is_some());

    let fallback_candidate = provider
        .read_progress(
            context,
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr-CA").unwrap(),
            },
        )
        .await
        .expect("fallback-candidate progress should aggregate");
    assert_eq!(fallback_candidate.resources, 2);
    assert_eq!(fallback_candidate.exact_optional_units, 0);

    let exact_cursor = exact
        .owner_change_cursor
        .clone()
        .expect("non-empty progress should expose the owner cursor");
    service
        .delete(tenant_id, first.id)
        .await
        .expect("deleting a translated asset should succeed");
    let after_delete = provider
        .read_progress(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::system(),
                "en-US",
                "translation-progress-after-delete",
            )
            .with_deadline(Duration::from_secs(5)),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("progress after owner lifecycle change should aggregate");
    assert_eq!(after_delete.resources, 1);
    assert_eq!(after_delete.exact_optional_units, 0);
    assert_ne!(
        after_delete
            .owner_change_cursor
            .as_ref()
            .map(|cursor| cursor.as_str()),
        Some(exact_cursor.as_str())
    );
    let lifecycle_changes = provider
        .read_changes(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::system(),
                "en-US",
                "translation-progress-delete-cursor",
            )
            .with_deadline(Duration::from_secs(5)),
            TranslationTargetChangesRequest {
                after: Some(exact_cursor),
                limit: 10,
            },
        )
        .await
        .expect("owner lifecycle changes should be cursor-readable");
    assert_eq!(lifecycle_changes.changes.len(), 2);
    assert!(lifecycle_changes.changes.iter().all(|change| {
        change.lifecycle == rustok_translation_targets::TranslationResourceLifecycle::Deleted
    }));

    let foreign_tenant = provider
        .read_progress(
            PortContext::new(
                Uuid::new_v4().to_string(),
                PortActor::system(),
                "en-US",
                "translation-progress-tenant-isolation",
            )
            .with_deadline(Duration::from_secs(5)),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en-US").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("foreign tenant progress should aggregate");
    assert_eq!(foreign_tenant.resources, 0);
    assert_eq!(foreign_tenant.exact_optional_units, 0);
    assert!(foreign_tenant.owner_change_cursor.is_none());
}

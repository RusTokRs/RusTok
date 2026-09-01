use std::time::Duration;

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_media::{
    MediaPublicImageReadPort, MediaPublicImageService, MediaService, UploadInput, migrations,
};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (
    sea_orm::DatabaseConnection,
    StorageRuntime,
    tempfile::TempDir,
    Uuid,
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

    let tenant_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .expect("tenant fixture should be inserted");

    let directory = tempfile::tempdir().expect("temporary object directory should be created");
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.path().display().to_string(),
        base_url: String::new(),
        fsync: false,
    })
    .expect("local object store should initialize without a public base URL");

    (database, storage, directory, tenant_id)
}

fn png_bytes() -> Bytes {
    let image = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
        24,
        12,
        image::Rgba([10, 20, 30, 255]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    Bytes::from(bytes.into_inner())
}

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("media-public-image-test"),
        "en",
        "media-public-image-test-correlation",
    )
    .with_deadline(Duration::from_secs(2))
}

#[tokio::test]
async fn storage_relative_image_gets_owner_capability_url_and_immutable_body() {
    let (database, storage, directory, tenant_id) = setup().await;
    let expected = png_bytes();
    let item = MediaService::new(database.clone(), storage.clone())
        .upload(UploadInput {
            tenant_id,
            uploaded_by: None,
            original_name: "profile.png".to_string(),
            content_type: "image/png".to_string(),
            data: expected.clone(),
        })
        .await
        .expect("image upload should succeed");
    assert_eq!(item.public_url, item.storage_path);

    let service = MediaPublicImageService::new(database.clone(), storage);
    let public_asset = MediaPublicImageReadPort::get_public_image_asset(
        &service,
        context(tenant_id),
        item.id,
        Some("Profile image".to_string()),
    )
    .await
    .expect("owner public image descriptor should resolve");
    let descriptor = public_asset
        .descriptor
        .expect("storage-relative image should receive a capability URL");
    let expected_prefix = format!("/api/media/public/images/{}/", item.id);
    assert!(descriptor.url.starts_with(&expected_prefix));
    assert_ne!(descriptor.url, item.storage_path);
    assert_eq!(descriptor.mime_type.as_deref(), Some("image/png"));
    assert_eq!((descriptor.width, descriptor.height), (Some(24), Some(12)));

    let checksum = descriptor
        .url
        .strip_prefix(&expected_prefix)
        .expect("capability URL should end in checksum");
    assert_eq!(checksum.len(), 64);
    let body = service
        .read_public_image(tenant_id, item.id, checksum)
        .await
        .expect("capability should read immutable owner bytes");
    assert_eq!(body.bytes, expected);
    assert_eq!(body.mime_type, "image/png");
    assert_eq!(body.etag(), format!("\"sha256-{checksum}\""));

    let wrong_checksum = "0".repeat(64);
    let wrong = service
        .read_public_image(tenant_id, item.id, &wrong_checksum)
        .await
        .expect_err("wrong checksum must not expose the object");
    assert_eq!(wrong.kind, PortErrorKind::NotFound);
    let cross_tenant = service
        .read_public_image(Uuid::new_v4(), item.id, checksum)
        .await
        .expect_err("cross-tenant capability must not expose the object");
    assert_eq!(cross_tenant.kind, PortErrorKind::NotFound);

    let direct_storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.path().display().to_string(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .expect("direct-public runtime should initialize");
    let direct = MediaPublicImageReadPort::get_public_image_asset(
        &MediaPublicImageService::new(database, direct_storage),
        context(tenant_id),
        item.id,
        None,
    )
    .await
    .expect("direct-public descriptor should resolve")
    .descriptor
    .expect("image descriptor should remain available");
    assert!(direct.url.starts_with("/media/"));
    assert!(!direct.url.contains("/api/media/public/images/"));
}

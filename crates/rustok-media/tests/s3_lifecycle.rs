#![cfg(feature = "s3")]

use std::time::Duration;

use bytes::Bytes;
use rustok_media::{
    CreateRenditionInput, ImageBackground, ImageOutputFormat, ImageRecipe, MediaService,
    PrepareUploadSessionInput, QuarterTurn, UploadInput, migrations,
};
use rustok_storage::{
    LocalStorageConfig, S3StorageConfig, StorageConfig, StorageDriver, StorageRuntime,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

fn png() -> Bytes {
    let image = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
        32,
        16,
        image::Rgba([30, 60, 90, 255]),
    ));
    let mut output = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    Bytes::from(output.into_inner())
}

async fn database(tenant_id: Uuid) -> sea_orm::DatabaseConnection {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite should connect");
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .expect("tenant fixture table should create");
    database
        .execute_unprepared("CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .expect("user fixture table should create");
    let manager = SchemaManager::new(&database);
    for migration in migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("Media migration should apply");
    }
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .expect("tenant should seed");
    database
}

async fn runtime() -> Option<StorageRuntime> {
    let endpoint = std::env::var("RUSTOK_TEST_S3_ENDPOINT").ok()?;
    Some(
        StorageRuntime::from_config(&StorageConfig {
            driver: StorageDriver::S3,
            local: LocalStorageConfig::default(),
            s3: S3StorageConfig {
                bucket: std::env::var("RUSTOK_TEST_S3_BUCKET")
                    .unwrap_or_else(|_| "rustok-media-test".to_string()),
                region: Some(
                    std::env::var("RUSTOK_TEST_S3_REGION")
                        .unwrap_or_else(|_| "us-east-1".to_string()),
                ),
                endpoint_url: Some(endpoint),
                access_key_id: Some(
                    std::env::var("RUSTOK_TEST_S3_ACCESS_KEY")
                        .unwrap_or_else(|_| "rustok-test".to_string()),
                ),
                secret_access_key: Some(
                    std::env::var("RUSTOK_TEST_S3_SECRET_KEY")
                        .unwrap_or_else(|_| "rustok-test-secret".to_string()),
                ),
                session_token: None,
                public_base_url: None,
                allow_http: true,
                virtual_hosted_style_request: false,
            },
        })
        .await
        .expect("S3-compatible runtime should initialize"),
    )
}

#[tokio::test]
async fn media_lifecycle_and_presigned_upload_conform_to_s3_compatible_storage() {
    let Some(storage) = runtime().await else {
        eprintln!("skipping Media S3 lifecycle: RUSTOK_TEST_S3_ENDPOINT is not set");
        return;
    };
    let tenant_id = Uuid::new_v4();
    let database = database(tenant_id).await;
    let service = MediaService::new(database, storage);
    let source = png();

    let asset = service
        .upload(UploadInput {
            tenant_id,
            uploaded_by: None,
            original_name: "source.png".to_string(),
            content_type: "image/png".to_string(),
            data: source.clone(),
        })
        .await
        .expect("direct S3 upload should succeed");
    service
        .create_rendition(CreateRenditionInput {
            tenant_id,
            asset_id: asset.id,
            purpose: "s3-proof".to_string(),
            recipe: ImageRecipe {
                crop: None,
                resize: None,
                rotate: QuarterTurn::None,
                flip_horizontal: false,
                flip_vertical: false,
                output: ImageOutputFormat::Webp,
                quality: 80,
                background: ImageBackground::default(),
                strip_metadata: true,
            },
        })
        .await
        .expect("S3 rendition should succeed");

    let prepared = service
        .prepare_upload_session(PrepareUploadSessionInput {
            tenant_id,
            actor_id: None,
            original_name: "presigned.png".to_string(),
            content_type: "image/png".to_string(),
            content_length: Some(source.len() as u64),
            expires_in: Duration::from_secs(300),
        })
        .await
        .expect("presigned S3 session should prepare");
    let response = reqwest::Client::new()
        .put(prepared.endpoint)
        .body(source)
        .send()
        .await
        .expect("signed PUT request should complete");
    assert!(
        response.status().is_success(),
        "signed PUT failed: {response:?}"
    );
    let presigned_asset = service
        .complete_upload_session(tenant_id, prepared.id)
        .await
        .expect("presigned S3 session should finalize");

    service
        .delete(tenant_id, asset.id)
        .await
        .expect("direct S3 asset should delete");
    service
        .delete(tenant_id, presigned_asset.id)
        .await
        .expect("presigned S3 asset should delete");
}

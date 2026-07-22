use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_media::{
    MediaAssetReadPort, MediaAssetWritePort, MediaReconciliationRequest, MediaService,
    MediaUploadRequest, MediaUploadTransport, UploadInput, UpsertTranslationInput, migrations,
};
use rustok_media_transport::{
    GrpcMediaProvider, MediaGrpcService, proto::media_service_server::MediaServiceServer,
};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Endpoint, Server};
use uuid::Uuid;

async fn test_service() -> (Arc<MediaService>, DatabaseConnection, tempfile::TempDir) {
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

    (
        Arc::new(MediaService::new(database.clone(), storage)),
        database,
        directory,
    )
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) {
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .expect("tenant fixture should be inserted");
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("media-conformance"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(5))
}

fn write_context(tenant_id: Uuid, operation: &str) -> PortContext {
    read_context(tenant_id).with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

fn png_upload(tenant_id: Uuid, name: &str) -> UploadInput {
    let image = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
        8,
        4,
        image::Rgba([20, 40, 60, 255]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    UploadInput {
        tenant_id,
        uploaded_by: None,
        original_name: name.to_string(),
        content_type: "image/png".to_string(),
        data: Bytes::from(bytes.into_inner()),
    }
}

async fn exercise_provider(
    read: &dyn MediaAssetReadPort,
    write: &dyn MediaAssetWritePort,
    tenant_id: Uuid,
    asset_id: Uuid,
) {
    let item = read
        .get_asset(read_context(tenant_id), asset_id)
        .await
        .expect("get_asset should preserve the owner DTO");
    assert_eq!(item.id, asset_id);
    assert_eq!(item.tenant_id, tenant_id);
    assert_eq!(item.mime_type, "image/png");

    let (items, total) = read
        .list_assets(read_context(tenant_id), 100, 0)
        .await
        .expect("list_assets should succeed");
    assert!(total >= 1);
    assert!(items.iter().any(|candidate| candidate.id == asset_id));

    let descriptor = read
        .get_image_descriptor(
            read_context(tenant_id),
            asset_id,
            Some("  Product hero  ".to_string()),
        )
        .await
        .expect("get_image_descriptor should succeed")
        .expect("PNG should expose an image descriptor");
    assert_eq!(descriptor.alt.as_deref(), Some("Product hero"));
    assert_eq!(
        (descriptor.width, descriptor.height),
        (item.width, item.height)
    );
    assert_eq!(descriptor.url, item.public_url);

    assert!(
        read.get_translations(read_context(tenant_id), asset_id)
            .await
            .expect("initial translations should load")
            .is_empty()
    );
    let translation = write
        .upsert_translation(
            write_context(tenant_id, "translation"),
            asset_id,
            UpsertTranslationInput {
                locale: "EN_us".to_string(),
                title: Some(" Hero ".to_string()),
                alt_text: Some(" Accessible hero ".to_string()),
                caption: None,
            },
        )
        .await
        .expect("upsert_translation should preserve normalization");
    assert_eq!(translation.locale, "en-us");
    assert_eq!(translation.title.as_deref(), Some("Hero"));
    assert_eq!(
        read.get_translations(read_context(tenant_id), asset_id)
            .await
            .expect("translations should load")
            .len(),
        1
    );

    let upload_target = write
        .prepare_upload(
            write_context(tenant_id, "prepare"),
            MediaUploadRequest {
                original_name: "next.png".to_string(),
                content_type: "image/png".to_string(),
                content_length: Some(128),
            },
        )
        .await
        .expect("Local provider should return its owner streaming endpoint");
    assert_eq!(
        upload_target.transport,
        MediaUploadTransport::OwnerStreamingRest
    );
    assert!(upload_target.session_id.is_none());

    let missing_session = write
        .complete_upload(write_context(tenant_id, "complete"), Uuid::new_v4())
        .await
        .expect_err("unknown upload session should retain typed not-found semantics");
    assert_eq!(missing_session.kind, PortErrorKind::NotFound);
    assert_eq!(missing_session.code, "media.not_found");

    write
        .reconcile_storage(
            write_context(tenant_id, "reconcile"),
            MediaReconciliationRequest { limit: 100 },
        )
        .await
        .expect("reconcile_storage should preserve its report contract");

    let missing_deadline = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("media-conformance"),
        "en",
        Uuid::new_v4().to_string(),
    );
    let policy_error = read
        .get_asset(missing_deadline, asset_id)
        .await
        .expect_err("deadline policy should cross the provider boundary");
    assert_eq!(policy_error.kind, PortErrorKind::Timeout);
    assert_eq!(policy_error.code, "port.deadline_required");

    write
        .delete_asset(write_context(tenant_id, "delete"), asset_id)
        .await
        .expect("delete_asset should complete through reconciliation");
    let deleted = read
        .get_asset(read_context(tenant_id), asset_id)
        .await
        .expect_err("deleted asset should retain typed not-found semantics");
    assert_eq!(deleted.kind, PortErrorKind::NotFound);
    assert_eq!(deleted.code, "media.not_found");
}

#[tokio::test]
async fn embedded_and_loopback_grpc_providers_pass_the_same_port_suite() {
    let (service, database, _directory) = test_service().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;

    let embedded_asset = service
        .upload(png_upload(tenant_id, "embedded.png"))
        .await
        .expect("embedded asset should upload");
    exercise_provider(
        service.as_ref(),
        service.as_ref(),
        tenant_id,
        embedded_asset.id,
    )
    .await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener should bind");
    let address = listener
        .local_addr()
        .expect("listener address should exist");
    let incoming = TcpListenerStream::new(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_service = MediaServiceServer::new(MediaGrpcService::new(service.clone()));
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(server_service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("loopback gRPC server should run");
    });
    let remote = GrpcMediaProvider::connect(
        Endpoint::from_shared(format!("http://{address}"))
            .expect("loopback endpoint should parse")
            .connect_timeout(Duration::from_secs(5)),
    )
    .await
    .expect("loopback gRPC client should connect");

    let remote_asset = service
        .upload(png_upload(tenant_id, "remote.png"))
        .await
        .expect("remote asset should upload through the Media-owned binary path");
    exercise_provider(&remote, &remote, tenant_id, remote_asset.id).await;

    let _ = shutdown_tx.send(());
    server.await.expect("loopback server task should stop");
}

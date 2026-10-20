//! Real Media owner + Product consumer/maintenance/queue integration. This uses
//! isolated owner databases and real storage, not a fake reference port. Product
//! image rows/removal intents are fixture-seeded; it does not certify mounted
//! Product commands, receipt transactions, PostgreSQL, or process crash recovery.
use super::*;
use rustok_api::{Permission, PortActor, PortErrorKind};
use rustok_media::{MediaService, UploadInput};
use rustok_product::{
    CatalogService, ProductCatalogCommandRuntime, ProductMediaMaintenancePort,
    entities::{product_image, product_media_release},
};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    Schema, Set, Statement, TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, prelude::*};
use std::time::Duration;

struct Fixture {
    media: Arc<MediaService>,
    product_db: DatabaseConnection,
    tenant: Uuid,
    other_tenant: Uuid,
    parent: Uuid,
    _storage: tempfile::TempDir,
}
impl Fixture {
    async fn new() -> Self {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        db.execute_unprepared("CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        let manager = SchemaManager::new(&db);
        rustok_outbox::SysEventsMigration
            .up(&manager)
            .await
            .unwrap();
        for migration in rustok_media::migrations::migrations() {
            migration.up(&manager).await.unwrap();
        }
        let tenant = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        for id in [tenant, other_tenant] {
            db.execute_raw(Statement::from_sql_and_values(
                db.get_database_backend(),
                "INSERT INTO tenants (id) VALUES (?)",
                [id.into()],
            ))
            .await
            .unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: directory.path().display().to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .unwrap();
        let media = Arc::new(MediaService::new(db, storage));
        // Deliberately a separate database: no shared Product/Media transaction.
        let product_db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&product_db);
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("products"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .to_owned(),
            )
            .await
            .unwrap();
        manager
            .create_table(
                Schema::new(product_db.get_database_backend())
                    .create_table_from_entity(product_image::Entity),
            )
            .await
            .unwrap();
        rustok_product::migrations::migrations()
            .into_iter()
            .find(|migration| migration.name() == "m20261018_000037_create_product_media_releases")
            .expect("retention migration")
            .up(&manager)
            .await
            .unwrap();
        let parent = Uuid::new_v4();
        product_db
            .execute_raw(Statement::from_sql_and_values(
                product_db.get_database_backend(),
                "INSERT INTO products (id, tenant_id) VALUES (?, ?)",
                [parent.into(), tenant.into()],
            ))
            .await
            .unwrap();
        Self {
            media,
            product_db,
            tenant,
            other_tenant,
            parent,
            _storage: directory,
        }
    }
    async fn upload(&self) -> Uuid {
        self.media
            .upload(UploadInput {
                tenant_id: self.tenant,
                uploaded_by: None,
                owner_module: None,
                original_name: "product.png".into(),
                content_type: "image/png".into(),
                data: bytes::Bytes::from_static(PNG),
            })
            .await
            .unwrap()
            .id
    }
    async fn image(&self, asset: Uuid, position: i32) -> product_image::Model {
        product_image::ActiveModel {
            id: Set(Uuid::new_v4()),
            product_id: Set(self.parent),
            media_id: Set(asset),
            position: Set(position),
        }
        .insert(&self.product_db)
        .await
        .unwrap()
    }
    async fn queue(&self, image: &product_image::Model, context: &PortContext, remove: bool) {
        let tx = self.product_db.begin().await.unwrap();
        product_media_release::ActiveModel {
            image_id: Set(image.id),
            tenant_id: Set(self.tenant),
            media_id: Set(image.media_id),
            context: Set(serde_json::to_value(context).unwrap()),
            attempts: Set(0),
            next_attempt_at: Set(chrono::Utc::now()),
            last_error_code: Set(None),
        }
        .insert(&tx)
        .await
        .unwrap();
        if remove {
            product_image::Entity::delete_by_id(image.id)
                .exec(&tx)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
    }
    async fn make_due(&self) {
        product_media_release::Entity::update_many()
            .col_expr(
                product_media_release::Column::NextAttemptAt,
                sea_orm::sea_query::Expr::value(chrono::Utc::now()),
            )
            .exec(&self.product_db)
            .await
            .unwrap();
    }
}
fn context(tenant: Uuid, actor: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(actor.to_string()),
        "en",
        "product-media-owner-integration",
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(2))
    .with_claim(Permission::PRODUCTS_MANAGE.to_string())
    .with_claim(Permission::PRODUCTS_READ.to_string())
}
async fn exercise(fixture: &Fixture, pair: MediaAssetPorts) {
    let provider = Arc::new(ProductMediaReferenceProvider::new(pair.clone()));
    let service = CatalogService::new(fixture.product_db.clone(), mock_transactional_event_bus())
        .with_media_reference_port(provider.clone());
    let runtime = ProductCatalogCommandRuntime::in_process(
        fixture.product_db.clone(),
        mock_transactional_event_bus(),
    )
    .with_media_reference_port(provider.clone())
    .unwrap();
    let asset = fixture.upload().await;
    let actor = Uuid::new_v4();
    let original = context(fixture.tenant, actor, "legacy-backfill");
    let mut denied = original.clone();
    denied.claims.clear();
    assert_eq!(
        ProductMediaMaintenancePort::audit(&service, denied, None, None, 1)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden
    );
    let first = fixture.image(asset, 0).await;
    let second = fixture.image(asset, 1).await;
    let report = service
        .backfill_media_references(original.clone(), None, 100)
        .await
        .unwrap();
    assert_eq!(report.retained, 2);
    assert!(!report.legacy_images_remaining);
    assert!(
        service
            .audit_media_references(original.clone(), None, None, 100)
            .await
            .unwrap()
            .issues
            .is_empty()
    );
    assert_eq!(
        provider
            .lookup(original.clone(), vec![first.id, second.id])
            .await
            .unwrap()
            .len(),
        2
    );
    let foreign_image = Uuid::new_v4();
    let foreign = provider
        .retain(
            context(fixture.other_tenant, actor, "foreign-asset"),
            asset,
            foreign_image,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        foreign.kind,
        PortErrorKind::NotFound | PortErrorKind::Forbidden
    ));
    assert!(
        provider
            .lookup(original.clone(), vec![foreign_image])
            .await
            .unwrap()
            .is_empty()
    );
    let delete = pair
        .write()
        .delete_asset(context(fixture.tenant, actor, "delete-held"), asset)
        .await
        .unwrap_err();
    assert_eq!(delete.kind, PortErrorKind::Conflict);
    assert_eq!(delete.code, "media.asset_referenced");

    // A completed retain receipt remains historical after its hold is released.
    provider
        .release(
            context(fixture.tenant, actor, "release-first"),
            asset,
            first.id,
        )
        .await
        .unwrap();
    let stale = provider
        .retain(original.clone(), asset, first.id)
        .await
        .unwrap_err();
    assert_eq!(stale.code, "product.media_hold_unconfirmed");
    assert!(
        provider
            .lookup(original.clone(), vec![first.id])
            .await
            .unwrap()
            .is_empty()
    );
    provider
        .retain(
            context(fixture.tenant, actor, "explicit-repair-first"),
            asset,
            first.id,
        )
        .await
        .unwrap();

    // A committed deletion has work even if its release was completed but its
    // acknowledgement was lost. A racing explicit backfill may then recreate an
    // orphan; the historical release receipt must not acknowledge a live hold.
    let removal = context(fixture.tenant, actor, "remove-second");
    fixture.queue(&second, &removal, true).await;
    provider
        .release(removal.clone(), asset, second.id)
        .await
        .unwrap();
    provider
        .retain(
            context(fixture.tenant, actor, "racing-backfill"),
            asset,
            second.id,
        )
        .await
        .unwrap();
    assert_eq!(runtime.process_media_releases(100).await.unwrap().failed, 1);
    assert_eq!(
        product_media_release::Entity::find()
            .count(&fixture.product_db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        service
            .list_media_releases(original.clone(), None, 100)
            .await
            .unwrap()
            .jobs[0]
            .attempts,
        1
    );
    // Fixture-only operator cleanup after independently proving the row absent.
    provider
        .release(
            context(fixture.tenant, actor, "confirmed-orphan-cleanup"),
            asset,
            second.id,
        )
        .await
        .unwrap();
    fixture.make_due().await;
    assert_eq!(
        runtime.process_media_releases(100).await.unwrap().released,
        1
    );
    assert_eq!(
        product_media_release::Entity::find()
            .count(&fixture.product_db)
            .await
            .unwrap(),
        0
    );

    // Even a corrupt/manual queue row cannot release a still-live Product image.
    fixture
        .queue(
            &first,
            &context(fixture.tenant, actor, "invalid-live-removal"),
            false,
        )
        .await;
    assert_eq!(runtime.process_media_releases(100).await.unwrap().failed, 1);
    assert_eq!(
        provider
            .lookup(original.clone(), vec![first.id])
            .await
            .unwrap()
            .len(),
        1
    );
    product_image::Entity::delete_by_id(first.id)
        .exec(&fixture.product_db)
        .await
        .unwrap();
    fixture.make_due().await;
    assert_eq!(
        runtime.process_media_releases(100).await.unwrap().released,
        1
    );
    assert_eq!(
        product_media_release::Entity::find()
            .count(&fixture.product_db)
            .await
            .unwrap(),
        0
    );
    pair.write()
        .delete_asset(context(fixture.tenant, actor, "delete-unheld"), asset)
        .await
        .unwrap();
    assert!(
        provider
            .lookup(original, vec![first.id, second.id])
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn native_owner_fences_gc_and_refuses_historical_hold_acknowledgements() {
    let fixture = Fixture::new().await;
    tokio::time::timeout(
        Duration::from_secs(30),
        exercise(&fixture, MediaAssetPorts::new(fixture.media.clone())),
    )
    .await
    .expect("native owner scenario exceeded its test deadline");
}

#[cfg(any(feature = "media-grpc", feature = "mod-profiles"))]
mod grpc {
    use super::*;
    struct Loopback {
        shutdown: Option<tokio::sync::oneshot::Sender<()>>,
        task: Option<tokio::task::JoinHandle<()>>,
    }
    impl Drop for Loopback {
        fn drop(&mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(task) = self.task.take() {
                task.abort();
            }
        }
    }
    impl Loopback {
        async fn start(fixture: &Fixture) -> (Self, MediaAssetPorts) {
            use rustok_media_transport::{
                GrpcMediaProvider, MediaGrpcOperation, MediaGrpcService, TrustedMediaAuthority,
                proto::media_service_server::MediaServiceServer,
            };
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let authority = TrustedMediaAuthority::new(
                fixture.tenant.to_string(),
                PortActor::service("product-media-integration"),
            )
            .allow_operations([
                MediaGrpcOperation::LookupAssetReferences,
                MediaGrpcOperation::ListAssetReferences,
                MediaGrpcOperation::RetainAssetReference,
                MediaGrpcOperation::ReleaseAssetReference,
                MediaGrpcOperation::DeleteAsset,
            ]);
            let service = MediaServiceServer::with_interceptor(
                MediaGrpcService::new(fixture.media.clone()),
                move |mut request: tonic::Request<()>| {
                    request.extensions_mut().insert(authority.clone());
                    Ok(request)
                },
            );
            let (shutdown, receive) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                tonic::transport::Server::builder()
                    .add_service(service)
                    .serve_with_incoming_shutdown(
                        tokio_stream::wrappers::TcpListenerStream::new(listener),
                        async {
                            let _ = receive.await;
                        },
                    )
                    .await
                    .unwrap();
            });
            let running = Self {
                shutdown: Some(shutdown),
                task: Some(task),
            };
            let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
                .unwrap()
                .connect_timeout(Duration::from_secs(5));
            let provider = GrpcMediaProvider::connect(endpoint).await.unwrap();
            (running, MediaAssetPorts::new(Arc::new(provider)))
        }
        async fn stop(mut self) {
            let _ = self.shutdown.take().unwrap().send(());
            self.task.take().unwrap().await.unwrap();
        }
    }
    #[tokio::test]
    async fn loopback_owner_fences_gc_and_refuses_historical_hold_acknowledgements() {
        let fixture = Fixture::new().await;
        let (server, pair) = Loopback::start(&fixture).await;
        tokio::time::timeout(Duration::from_secs(30), exercise(&fixture, pair))
            .await
            .expect("loopback owner scenario exceeded its test deadline");
        server.stop().await;
    }
}

// Small real PNG, generated from scanlines with standard PNG CRC/zlib encoding.
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 8, 0, 0, 0, 4, 8, 6, 0,
    0, 0, 179, 205, 126, 240, 0, 0, 0, 18, 73, 68, 65, 84, 120, 156, 99, 16, 209, 176, 249, 143,
    15, 51, 208, 94, 1, 0, 233, 153, 46, 225, 1, 68, 160, 172, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
    96, 130,
];

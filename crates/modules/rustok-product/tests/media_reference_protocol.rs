//! Real SQLite/PostgreSQL local transaction/FK/retry checks, with a consumer-port
//! fake. Does not prove native Media GC or live gRPC; those remain separate gates.
use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortContext, PortError};
use rustok_product::{
    CatalogService, ProductMediaReference, ProductMediaReferencePage, ProductMediaReferencePort,
    entities::{product_image, product_image_media_coverage, product_media_release},
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, Schema, Set, TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, prelude::*};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

#[derive(Default)]
struct References {
    holds: Mutex<BTreeMap<(Uuid, Uuid), Uuid>>,
    fail_release: AtomicBool,
}
#[async_trait]
impl ProductMediaReferencePort for References {
    async fn retain(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        self.holds.lock().unwrap().insert(
            (Uuid::parse_str(&context.tenant_id).unwrap(), image_id),
            media_id,
        );
        Ok(())
    }
    async fn release(
        &self,
        context: PortContext,
        _media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        if self.fail_release.load(Ordering::SeqCst) {
            return Err(PortError::unavailable("test.release_failed", "unavailable"));
        }
        self.holds
            .lock()
            .unwrap()
            .remove(&(Uuid::parse_str(&context.tenant_id).unwrap(), image_id));
        Ok(())
    }
    async fn lookup(
        &self,
        context: PortContext,
        ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
        Ok(self
            .holds
            .lock()
            .unwrap()
            .iter()
            .filter_map(|((tenant, image), media)| {
                (*tenant == tenant_id && ids.contains(image)).then_some(ProductMediaReference {
                    tenant_id,
                    image_id: *image,
                    media_id: *media,
                })
            })
            .collect())
    }
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
        let mut references: Vec<_> = self
            .holds
            .lock()
            .unwrap()
            .iter()
            .filter_map(|((tenant, image), media)| {
                (*tenant == tenant_id && after.is_none_or(|cursor| *image > cursor)).then_some(
                    ProductMediaReference {
                        tenant_id,
                        image_id: *image,
                        media_id: *media,
                    },
                )
            })
            .take(limit as usize + 1)
            .collect();
        let has_more = references.len() > limit as usize;
        references.truncate(limit as usize);
        Ok(ProductMediaReferencePage {
            next_image_id: references.last().map(|row| row.image_id),
            references,
            has_more,
        })
    }
}
fn context(tenant: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "en",
        "media-protocol",
    )
    .with_idempotency_key("media-protocol")
    .with_deadline(std::time::Duration::from_secs(2))
    .with_claim(Permission::PRODUCTS_MANAGE.to_string())
    .with_claim(Permission::PRODUCTS_READ.to_string())
}
fn job(image: &product_image::Model, context: &PortContext) -> product_media_release::ActiveModel {
    product_media_release::ActiveModel {
        image_id: Set(image.id),
        tenant_id: Set(Uuid::parse_str(&context.tenant_id).unwrap()),
        media_id: Set(image.media_id),
        context: Set(serde_json::to_value(context).unwrap()),
        attempts: Set(0),
        next_attempt_at: Set(chrono::Utc::now()),
        last_error_code: Set(None),
    }
}
async fn rollback_migration(
    db: &DatabaseConnection,
    migration: &dyn MigrationTrait,
) -> Result<(), sea_orm::DbErr> {
    let txn = db.begin().await?;
    let result = migration.down(&SchemaManager::new(&txn)).await;
    if result.is_ok() {
        txn.commit().await?;
    } else {
        txn.rollback().await?;
    }
    result
}
async fn create_protocol_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    manager
        .create_table(
            Table::create()
                .table(Alias::new("products"))
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                .to_owned(),
        )
        .await
        .unwrap();
    manager
        .create_table(
            Schema::new(db.get_database_backend()).create_table_from_entity(product_image::Entity),
        )
        .await
        .unwrap();
    let migrations = rustok_product::migrations::migrations();
    let migration = migrations
        .iter()
        .find(|m| m.name() == "m20261018_000037_create_product_media_releases")
        .expect("migration m20261018_000037_create_product_media_releases not found");
    migration.up(&manager).await.unwrap();
}
async fn exercise(db: DatabaseConnection) {
    create_protocol_schema(&db).await;
    let manager = SchemaManager::new(&db);
    let migrations = rustok_product::migrations::migrations();
    let migration = migrations
        .iter()
        .find(|m| m.name() == "m20261018_000037_create_product_media_releases")
        .expect("migration m20261018_000037_create_product_media_releases not found");
    assert_eq!(
        migration.name(),
        "m20261018_000037_create_product_media_releases"
    );
    assert_eq!(migration.use_transaction(), Some(true));
    let tenant = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let ctx = context(tenant);
    let insert = Query::insert()
        .into_table(Alias::new("products"))
        .columns([Alias::new("id"), Alias::new("tenant_id")])
        .values_panic([parent.into(), tenant.into()])
        .to_owned();
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    let image = product_image::ActiveModel {
        id: Set(Uuid::new_v4()),
        product_id: Set(parent),
        media_id: Set(Uuid::new_v4()),
        position: Set(0),
    }
    .insert(&db)
    .await
    .unwrap();
    let references = Arc::new(References::default());
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus())
        .with_media_reference_port(references.clone());
    let initial = service
        .audit_media_references(ctx.clone(), None, None, 100)
        .await
        .unwrap();
    assert_eq!(initial.issues[0].kind, "missing_media_hold");
    let filled = service
        .backfill_media_references(ctx.clone(), None, 100)
        .await
        .unwrap();
    assert_eq!(filled.retained, 1);
    assert!(!filled.legacy_images_remaining);
    // Repeated backfill must tolerate an existing provisioning marker.
    assert_eq!(
        service
            .backfill_media_references(ctx.clone(), None, 100)
            .await
            .unwrap()
            .retained,
        1
    );
    assert!(
        rollback_migration(&db, migration.as_ref()).await.is_err(),
        "covered live relations block rollback"
    );
    let txn = db.begin().await.unwrap();
    job(&image, &ctx).insert(&txn).await.unwrap();
    product_image::Entity::delete_by_id(image.id)
        .exec(&txn)
        .await
        .unwrap();
    txn.rollback().await.unwrap();
    assert_eq!(
        product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert!(
        product_image_media_coverage::Entity::find_by_id(image.id)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    let txn = db.begin().await.unwrap();
    job(&image, &ctx).insert(&txn).await.unwrap();
    product_image::Entity::delete_by_id(image.id)
        .exec(&txn)
        .await
        .unwrap();
    txn.commit().await.unwrap();
    assert!(
        product_image_media_coverage::Entity::find_by_id(image.id)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        references
            .holds
            .lock()
            .unwrap()
            .contains_key(&(tenant, image.id))
    );
    assert!(
        rollback_migration(&db, migration.as_ref()).await.is_err(),
        "pending durable work blocks rollback"
    );
    references.fail_release.store(true, Ordering::SeqCst);
    assert_eq!(service.process_media_releases(1).await.unwrap().failed, 1);
    let status = service
        .list_media_releases(ctx.clone(), None, 100)
        .await
        .unwrap();
    assert_eq!(status.jobs.len(), 1);
    assert_eq!(status.jobs[0].attempts, 1);
    assert!(status.jobs[0].last_error_code.is_some());
    assert!(
        service
            .list_media_releases(context(Uuid::new_v4()), None, 100)
            .await
            .unwrap()
            .jobs
            .is_empty()
    );
    references.fail_release.store(false, Ordering::SeqCst);
    product_media_release::Entity::update_many()
        .col_expr(
            product_media_release::Column::NextAttemptAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now()),
        )
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(service.process_media_releases(1).await.unwrap().released, 1);
    assert_eq!(
        product_media_release::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    let orphan = Uuid::new_v4();
    references
        .holds
        .lock()
        .unwrap()
        .insert((tenant, orphan), image.media_id);
    let audit = service
        .audit_media_references(ctx.clone(), None, None, 1)
        .await
        .unwrap();
    assert_eq!(audit.issues[0].kind, "orphan_media_hold");
    assert!(
        references
            .holds
            .lock()
            .unwrap()
            .contains_key(&(tenant, orphan))
    );
    assert!(
        service
            .backfill_media_references(ctx.clone(), None, 101)
            .await
            .is_err()
    );
    assert!(
        service
            .audit_media_references(ctx, None, None, 0)
            .await
            .is_err()
    );
    rollback_migration(&db, migration.as_ref()).await.unwrap();
    migration.up(&manager).await.unwrap();
}
#[tokio::test]
async fn sqlite_media_reference_transaction_and_backfill_protocol() {
    exercise(rustok_test_utils::db::setup_test_db().await).await;
}
#[tokio::test]
#[ignore = "requires RUSTOK_MIGRATION_SMOKE_ADMIN_URL PostgreSQL admin access"]
async fn postgres_media_reference_transaction_and_backfill_protocol() {
    use rustok_test_utils::{
        assert_postgres_url, connect_postgres, create_postgres_database,
        drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
    };
    let url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .expect("PostgreSQL admin URL is required");
    assert_postgres_url(&url);
    let admin = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name("product_media_protocol");
    create_postgres_database(&admin, &name).await.unwrap();
    let db = connect_postgres(&postgres_database_url(&url, &name))
        .await
        .unwrap();
    let test_db = db.clone();
    let result = tokio::spawn(async move {
        exercise(test_db.clone()).await;
        exercise_batch_race(test_db.clone(), false).await;
        exercise_batch_race(test_db, true).await;
    })
    .await;
    db.close().await.unwrap();
    drop_postgres_database_if_exists(&admin, &name)
        .await
        .unwrap();
    result.unwrap();
}

/// Deterministically interleave a successful acknowledgement by another replica
/// with this replica's failed network attempt, using the actual local queue DB.
struct AcknowledgedElsewhere {
    db: DatabaseConnection,
    first: Uuid,
    advance_retry: bool,
    inner: References,
}
#[async_trait]
impl ProductMediaReferencePort for AcknowledgedElsewhere {
    async fn retain(
        &self,
        context: PortContext,
        media: Uuid,
        image: Uuid,
    ) -> Result<(), PortError> {
        self.inner.retain(context, media, image).await
    }
    async fn release(
        &self,
        context: PortContext,
        media: Uuid,
        image: Uuid,
    ) -> Result<(), PortError> {
        if image == self.first {
            if self.advance_retry {
                product_media_release::Entity::update_many()
                    .col_expr(
                        product_media_release::Column::Attempts,
                        sea_orm::sea_query::Expr::value(7),
                    )
                    .col_expr(
                        product_media_release::Column::NextAttemptAt,
                        sea_orm::sea_query::Expr::value(
                            chrono::Utc::now() + chrono::Duration::hours(1),
                        ),
                    )
                    .col_expr(
                        product_media_release::Column::LastErrorCode,
                        sea_orm::sea_query::Expr::value("newer_attempt"),
                    )
                    .filter(product_media_release::Column::ImageId.eq(image))
                    .exec(&self.db)
                    .await
                    .unwrap();
            } else {
                product_media_release::Entity::delete_by_id(image)
                    .exec(&self.db)
                    .await
                    .unwrap();
            }
            return Err(PortError::unavailable(
                "test.concurrent_ack",
                "another replica acknowledged this work",
            ));
        }
        self.inner.release(context, media, image).await
    }
    async fn lookup(
        &self,
        context: PortContext,
        ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError> {
        if ids.contains(&self.first) {
            return Err(PortError::unavailable(
                "test.lookup_unavailable",
                "network outage",
            ));
        }
        self.inner.lookup(context, ids).await
    }
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError> {
        self.inner.list(context, after, limit).await
    }
}
async fn exercise_batch_race(db: DatabaseConnection, advance_retry: bool) {
    let ctx = context(Uuid::new_v4());
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let due = chrono::Utc::now();
    for id in [first, second] {
        let image = product_image::Model {
            id,
            product_id: Uuid::new_v4(),
            media_id: Uuid::new_v4(),
            position: 0,
        };
        let mut work = job(&image, &ctx);
        work.next_attempt_at = Set(due);
        work.insert(&db).await.unwrap();
    }
    let port = Arc::new(AcknowledgedElsewhere {
        db: db.clone(),
        first,
        advance_retry,
        inner: References::default(),
    });
    let report = rustok_product::process_product_media_releases(&db, port, 2)
        .await
        .unwrap();
    assert_eq!(report.examined, 2);
    assert_eq!(report.failed, 1);
    assert_eq!(report.released, 1);
    let pending = product_media_release::Entity::find()
        .all(&db)
        .await
        .unwrap();
    if advance_retry {
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].image_id, first);
        assert_eq!(pending[0].attempts, 7);
        assert!(pending[0].next_attempt_at > due + chrono::Duration::minutes(59));
        assert_eq!(pending[0].last_error_code.as_deref(), Some("newer_attempt"));
        product_media_release::Entity::delete_by_id(first)
            .exec(&db)
            .await
            .unwrap();
    } else {
        assert!(pending.is_empty());
    }
}
#[tokio::test]
async fn concurrently_acknowledged_release_does_not_abort_or_resurrect_the_batch() {
    let db = rustok_test_utils::db::setup_test_db().await;
    create_protocol_schema(&db).await;
    exercise_batch_race(db, false).await;
}
#[tokio::test]
async fn stale_failure_cannot_overwrite_a_newer_retry_schedule() {
    let db = rustok_test_utils::db::setup_test_db().await;
    create_protocol_schema(&db).await;
    exercise_batch_race(db, true).await;
}

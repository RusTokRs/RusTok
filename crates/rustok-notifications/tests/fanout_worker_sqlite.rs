use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_notifications::api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudienceCandidate,
    NotificationAudienceCursor, NotificationAudiencePage, NotificationOpenAuthorization,
    NotificationPriority, NotificationProviderError, NotificationProviderResult,
    NotificationSemanticDescriptor, NotificationSourceEventRef, NotificationSourceProvider,
    NotificationSourceRegistry, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
    ResolveNotificationAudienceRequest,
};
use rustok_notifications::entities::{delivery_attempt, fanout_item, notification};
use rustok_notifications::model::FanoutItemStatus;
use rustok_notifications::{
    MAX_NOTIFICATION_FANOUT_BATCH_SIZE, MAX_NOTIFICATION_FANOUT_PAGE_SIZE,
    NotificationFanoutService, NotificationFanoutWorker, NotificationsModule,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "worker-source";
const EVENT_TYPE: &str = "worker.event";
const NOTIFICATION_TYPE: &str = "worker.notification";
const SECOND_PAGE: &str = "second-page";

#[derive(Clone)]
struct WorkerSourceProvider {
    first_recipient: Uuid,
    second_recipient: Uuid,
    target_id: Uuid,
}

#[async_trait]
impl NotificationSourceProvider for WorkerSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Worker source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![event_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        let template_data = NotificationTemplateData::try_new(BTreeMap::from([(
            "source_event_id".to_string(),
            request.event.event_id().to_string(),
        )]))
        .map_err(|_| NotificationProviderError::InvalidEvent)?;
        Ok(Some(NotificationSemanticDescriptor {
            notification_type: notification_type(),
            template_key: NotificationTemplateKey::new(NOTIFICATION_TYPE)
                .expect("worker template key must stay valid"),
            target: NotificationTargetRef {
                owner: source_slug(),
                kind: NotificationTargetKind::new("worker.target")
                    .expect("worker target kind must stay valid"),
                id: self.target_id,
            },
            actor_id: None,
            priority: NotificationPriority::Normal,
            template_data,
        }))
    }

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        if request.bounded_limit() != 1 {
            return Err(NotificationProviderError::Rejected);
        }
        match request
            .cursor
            .as_ref()
            .map(NotificationAudienceCursor::as_str)
        {
            None => NotificationAudiencePage::try_new(
                vec![NotificationAudienceCandidate {
                    recipient_id: self.first_recipient,
                }],
                Some(
                    NotificationAudienceCursor::new(SECOND_PAGE)
                        .expect("worker cursor must stay valid"),
                ),
            ),
            Some(SECOND_PAGE) => NotificationAudiencePage::try_new(
                vec![NotificationAudienceCandidate {
                    recipient_id: self.second_recipient,
                }],
                None,
            ),
            Some(_) => return Err(NotificationProviderError::InvalidEvent),
        }
        .map_err(|_| NotificationProviderError::Internal { retryable: false })
    }

    async fn authorize_target_open(
        &self,
        _request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        Ok(NotificationOpenAuthorization::Unavailable)
    }
}

#[tokio::test]
async fn bounded_worker_materializes_sources_and_pages_without_final_delivery() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let first_recipient = Uuid::new_v4();
    let second_recipient = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, first_recipient).await;
    insert_user(&db, tenant_id, second_recipient).await;

    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(WorkerSourceProvider {
            first_recipient,
            second_recipient,
            target_id: Uuid::new_v4(),
        })
        .expect("worker source should register");
    let registry = Arc::new(registry);
    let service = NotificationFanoutService::new(db.clone(), registry.clone());
    for revision in 1..=2 {
        service
            .enqueue_source_event(source_event(tenant_id, Uuid::new_v4(), revision))
            .await
            .expect("source event should enter durable inbox");
    }

    let worker = NotificationFanoutWorker::new(db.clone(), registry, "fanout-worker", 1, 1)
        .expect("bounded fanout worker should compose");
    let first_work = worker
        .claimable_source_inbox_work()
        .await
        .expect("tenant-scoped source work should select");
    assert_eq!(first_work.len(), 1);
    assert_eq!(first_work[0].tenant_id, tenant_id);

    let first = worker.process_next_batch().await.expect("first poll");
    assert_eq!(first.source_selected, 1);
    assert_eq!(first.source_completed, 1);
    assert_eq!(first.jobs_selected, 1);
    assert_eq!(first.pages_processed, 1);
    assert_eq!(first.jobs_completed, 0);
    assert!(first.failures.is_empty());

    let second = worker.process_next_batch().await.expect("second poll");
    assert_eq!(second.source_selected, 1);
    assert_eq!(second.source_completed, 1);
    assert_eq!(second.jobs_selected, 1);
    assert_eq!(second.jobs_completed, 1);
    assert!(second.failures.is_empty());

    let third = worker.process_next_batch().await.expect("third poll");
    assert_eq!(third.source_selected, 0);
    assert_eq!(third.jobs_selected, 1);
    assert_eq!(third.pages_processed, 1);
    assert_eq!(third.jobs_completed, 0);

    let fourth = worker.process_next_batch().await.expect("fourth poll");
    assert_eq!(fourth.source_selected, 0);
    assert_eq!(fourth.jobs_selected, 1);
    assert_eq!(fourth.jobs_completed, 1);
    assert!(worker
        .claimable_source_inbox_work()
        .await
        .expect("source selection")
        .is_empty());
    assert!(worker
        .claimable_fanout_job_work()
        .await
        .expect("job selection")
        .is_empty());

    let items = fanout_item::Entity::find()
        .filter(fanout_item::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("candidate rows should load");
    assert_eq!(items.len(), 4);
    assert!(items.iter().all(|item| {
        item.status == FanoutItemStatus::Pending && item.notification_id.is_none()
    }));
    assert_eq!(
        notification::Entity::find()
            .filter(notification::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("notification count"),
        0
    );
    assert_eq!(
        delivery_attempt::Entity::find()
            .filter(delivery_attempt::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("delivery count"),
        0
    );

    assert!(NotificationFanoutWorker::new(
        db.clone(),
        Arc::new(NotificationSourceRegistry::default()),
        "fanout-worker",
        MAX_NOTIFICATION_FANOUT_BATCH_SIZE + 1,
        1,
    )
    .is_err());
    assert!(NotificationFanoutWorker::new(
        db,
        Arc::new(NotificationSourceRegistry::default()),
        "fanout-worker",
        1,
        MAX_NOTIFICATION_FANOUT_PAGE_SIZE + 1,
    )
    .is_err());
}

fn source_event(tenant_id: Uuid, event_id: Uuid, revision: u64) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(tenant_id, event_id, source_slug(), event_type(), revision)
        .expect("worker source event must stay valid")
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("worker source slug must stay valid")
}

fn event_type() -> NotificationTypeKey {
    NotificationTypeKey::new(EVENT_TYPE).expect("worker event type must stay valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE).expect("worker notification type must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_fanout_worker_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("fanout worker sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    )
    .await
    .expect("platform identity fixture should apply");
    let manager = SchemaManager::new(&db);
    for migration in NotificationsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("notification migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
        .await
        .expect("tenant fixture should persist");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_unprepared(&format!(
        "INSERT INTO users (id, tenant_id) VALUES ('{user_id}', '{tenant_id}')"
    ))
    .await
    .expect("user fixture should persist");
}

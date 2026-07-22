use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_notifications::{
    NotificationFanoutService, NotificationsModule,
    entities::{fanout_item, notification, source_inbox},
    model::{FanoutItemStatus, NotificationSourceInboxStatus},
};
use rustok_notifications::api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest,
    NotificationAudienceCandidate, NotificationAudienceCursor, NotificationAudiencePage,
    NotificationOpenAuthorization, NotificationPriority, NotificationProviderError,
    NotificationProviderResult, NotificationSemanticDescriptor, NotificationSourceEventRef,
    NotificationSourceProvider, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRef, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey, ResolveNotificationAudienceRequest,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "test-source";
const EVENT_TYPE: &str = "test.event";
const NOTIFICATION_TYPE: &str = "test.notification";
const PAGE_TWO: &str = "page-2";

#[derive(Clone)]
struct FakeSourceProvider {
    first_recipient: Uuid,
    second_recipient: Uuid,
    target_id: Uuid,
}

#[async_trait]
impl NotificationSourceProvider for FakeSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Test source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![event_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        if request.event.source() != &source_slug()
            || request.event.event_type() != &event_type()
        {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let template_data = NotificationTemplateData::try_new(BTreeMap::from([(
            "source_event_id".to_string(),
            request.event.event_id().to_string(),
        )]))
        .map_err(|_| NotificationProviderError::InvalidEvent)?;
        Ok(Some(NotificationSemanticDescriptor {
            notification_type: notification_type(),
            template_key: NotificationTemplateKey::new(NOTIFICATION_TYPE)
                .expect("test template key must stay valid"),
            target: NotificationTargetRef {
                owner: source_slug(),
                kind: NotificationTargetKind::new("test.target")
                    .expect("test target kind must stay valid"),
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
        if request.bounded_limit() == 0 {
            return Err(NotificationProviderError::Rejected);
        }
        match request.cursor.as_ref().map(NotificationAudienceCursor::as_str) {
            None => NotificationAudiencePage::try_new(
                vec![NotificationAudienceCandidate {
                    recipient_id: self.first_recipient,
                }],
                Some(
                    NotificationAudienceCursor::new(PAGE_TWO)
                        .expect("test cursor must stay valid"),
                ),
            ),
            Some(PAGE_TWO) => NotificationAudiencePage::try_new(
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
async fn source_inbox_and_bounded_candidate_fanout_are_idempotent() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let first_recipient = Uuid::new_v4();
    let second_recipient = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, first_recipient).await;
    insert_user(&db, tenant_id, second_recipient).await;

    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(FakeSourceProvider {
            first_recipient,
            second_recipient,
            target_id: Uuid::new_v4(),
        })
        .expect("fake source should register");
    let service = NotificationFanoutService::new(db.clone(), Arc::new(registry));

    let event_id = Uuid::new_v4();
    let event = source_event(tenant_id, event_id, 1);
    let accepted = service
        .enqueue_source_event(event.clone())
        .await
        .expect("source event should be accepted");
    assert_eq!(accepted.status, NotificationSourceInboxStatus::Pending);
    assert!(!accepted.replayed);

    let replayed = service
        .enqueue_source_event(event.clone())
        .await
        .expect("identical source event should replay");
    assert_eq!(replayed.inbox_id, accepted.inbox_id);
    assert!(replayed.replayed);

    let conflict = service
        .enqueue_source_event(source_event(tenant_id, event_id, 2))
        .await
        .expect_err("changed revision for one event id must conflict");
    assert_eq!(
        conflict.stable_code(),
        "NOTIFICATION_SOURCE_IDENTITY_CONFLICT"
    );

    let materialized = service
        .materialize_source_event(accepted.inbox_id, "source-worker")
        .await
        .expect("source descriptor should materialize");
    assert_eq!(materialized.status, NotificationSourceInboxStatus::Completed);
    let job_id = materialized
        .fanout_job_id
        .expect("completed source event must link a fan-out job");

    let materialized_replay = service
        .materialize_source_event(accepted.inbox_id, "source-worker-replay")
        .await
        .expect("terminal source materialization should replay");
    assert_eq!(materialized_replay.fanout_job_id, Some(job_id));
    assert!(materialized_replay.replayed);

    let first_page = service
        .process_fanout_page(job_id, "fanout-worker-1", 1)
        .await
        .expect("first audience page should persist");
    assert_eq!(first_page.candidates, 1);
    assert_eq!(first_page.inserted_items, 1);
    assert_eq!(first_page.next_cursor.as_deref(), Some(PAGE_TWO));
    assert!(!first_page.completed);

    let second_page = service
        .process_fanout_page(job_id, "fanout-worker-2", 1)
        .await
        .expect("second audience page should persist");
    assert_eq!(second_page.candidates, 1);
    assert_eq!(second_page.inserted_items, 1);
    assert!(second_page.next_cursor.is_none());
    assert!(second_page.completed);

    let terminal_replay = service
        .process_fanout_page(job_id, "fanout-worker-replay", 1)
        .await
        .expect("completed fan-out should replay without provider work");
    assert_eq!(terminal_replay.candidates, 0);
    assert_eq!(terminal_replay.inserted_items, 0);
    assert!(terminal_replay.completed);

    let inbox = source_inbox::Entity::find_by_id(accepted.inbox_id)
        .one(&db)
        .await
        .expect("source inbox read should succeed")
        .expect("source inbox row should exist");
    assert_eq!(inbox.fanout_job_id, Some(job_id));
    assert_eq!(inbox.status, NotificationSourceInboxStatus::Completed);

    let items = fanout_item::Entity::find()
        .filter(fanout_item::Column::TenantId.eq(tenant_id))
        .filter(fanout_item::Column::FanoutJobId.eq(job_id))
        .all(&db)
        .await
        .expect("fan-out item read should succeed");
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .all(|item| item.status == FanoutItemStatus::Pending && item.notification_id.is_none()));

    let notification_count = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .count(&db)
        .await
        .expect("notification count should succeed");
    assert_eq!(
        notification_count, 0,
        "candidate fan-out must not bypass preference/privacy policy"
    );
}

fn source_event(tenant_id: Uuid, event_id: Uuid, revision: u64) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(
        tenant_id,
        event_id,
        source_slug(),
        event_type(),
        revision,
    )
    .expect("test source event must stay valid")
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("test source slug must stay valid")
}

fn event_type() -> NotificationTypeKey {
    NotificationTypeKey::new(EVENT_TYPE).expect("test event type must stay valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE)
        .expect("test notification type must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_fanout_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification fan-out sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL
        );
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
    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{tenant_id}')"
    ))
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

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
use rustok_notifications::{
    NotificationFanoutService, NotificationsModule,
    entities::{fanout_item, fanout_job},
    model::NotificationJobStatus,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "sparse-test-source";
const EVENT_TYPE: &str = "sparse.test.event";
const NOTIFICATION_TYPE: &str = "sparse.test.notification";
const PAGE_TWO: &str = "page-2";

#[derive(Clone)]
struct SparseSourceProvider {
    recipient_id: Uuid,
    target_id: Uuid,
    stall_after_sparse_page: bool,
}

#[async_trait]
impl NotificationSourceProvider for SparseSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Sparse test source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![event_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        if request.event.source() != &source_slug() || request.event.event_type() != &event_type() {
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
                .expect("sparse test template key must stay valid"),
            target: NotificationTargetRef {
                owner: source_slug(),
                kind: NotificationTargetKind::new("sparse.test.target")
                    .expect("sparse test target kind must stay valid"),
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
        match request
            .cursor
            .as_ref()
            .map(NotificationAudienceCursor::as_str)
        {
            None => NotificationAudiencePage::try_new(
                Vec::new(),
                Some(
                    NotificationAudienceCursor::new(PAGE_TWO)
                        .expect("sparse test cursor must stay valid"),
                ),
            ),
            Some(PAGE_TWO) if self.stall_after_sparse_page => NotificationAudiencePage::try_new(
                Vec::new(),
                Some(
                    NotificationAudienceCursor::new(PAGE_TWO)
                        .expect("stalled sparse cursor must stay valid"),
                ),
            ),
            Some(PAGE_TWO) => NotificationAudiencePage::try_new(
                vec![NotificationAudienceCandidate {
                    recipient_id: self.recipient_id,
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
async fn sparse_audience_page_advances_without_creating_candidates() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    let service = service(db.clone(), recipient_id, false);
    let job_id = materialize_job(&service, tenant_id).await;

    let sparse = service
        .process_fanout_page(job_id, "sparse-worker-1", 1)
        .await
        .expect("empty sparse page with an advancing cursor should persist");
    assert_eq!(sparse.candidates, 0);
    assert_eq!(sparse.inserted_items, 0);
    assert_eq!(sparse.next_cursor.as_deref(), Some(PAGE_TWO));
    assert!(!sparse.completed);
    assert_eq!(candidate_count(&db, tenant_id, job_id).await, 0);

    let terminal = service
        .process_fanout_page(job_id, "sparse-worker-2", 1)
        .await
        .expect("the page after a sparse cursor should complete normally");
    assert_eq!(terminal.candidates, 1);
    assert_eq!(terminal.inserted_items, 1);
    assert!(terminal.next_cursor.is_none());
    assert!(terminal.completed);
    assert_eq!(candidate_count(&db, tenant_id, job_id).await, 1);
}

#[tokio::test]
async fn sparse_audience_page_still_rejects_a_stalled_cursor() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    let service = service(db.clone(), recipient_id, true);
    let job_id = materialize_job(&service, tenant_id).await;

    let sparse = service
        .process_fanout_page(job_id, "stall-worker-1", 1)
        .await
        .expect("the initial sparse page should advance");
    assert_eq!(sparse.next_cursor.as_deref(), Some(PAGE_TWO));

    let error = service
        .process_fanout_page(job_id, "stall-worker-2", 1)
        .await
        .expect_err("a repeated sparse cursor must fail closed");
    assert_eq!(error.stable_code(), "NOTIFICATION_FANOUT_CURSOR_STALLED");

    let job = fanout_job::Entity::find_by_id(job_id)
        .one(&db)
        .await
        .expect("fan-out job read should succeed")
        .expect("fan-out job should exist");
    assert_eq!(job.status, NotificationJobStatus::DeadLetter);
    assert_eq!(job.audience_cursor.as_deref(), Some(PAGE_TWO));
    assert_eq!(
        job.last_error_code.as_deref(),
        Some("NOTIFICATION_FANOUT_CURSOR_STALLED")
    );
    assert_eq!(candidate_count(&db, tenant_id, job_id).await, 0);
}

fn service(
    db: DatabaseConnection,
    recipient_id: Uuid,
    stall_after_sparse_page: bool,
) -> NotificationFanoutService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(SparseSourceProvider {
            recipient_id,
            target_id: Uuid::new_v4(),
            stall_after_sparse_page,
        })
        .expect("sparse source should register");
    NotificationFanoutService::new(db, Arc::new(registry))
}

async fn materialize_job(service: &NotificationFanoutService, tenant_id: Uuid) -> Uuid {
    let accepted = service
        .enqueue_source_event(source_event(tenant_id))
        .await
        .expect("sparse source event should enqueue");
    service
        .materialize_source_event(accepted.inbox_id, "sparse-materialize-worker")
        .await
        .expect("sparse source descriptor should materialize")
        .fanout_job_id
        .expect("materialized sparse source should link a fan-out job")
}

async fn candidate_count(db: &DatabaseConnection, tenant_id: Uuid, job_id: Uuid) -> u64 {
    fanout_item::Entity::find()
        .filter(fanout_item::Column::TenantId.eq(tenant_id))
        .filter(fanout_item::Column::FanoutJobId.eq(job_id))
        .count(db)
        .await
        .expect("fan-out item count should succeed")
}

fn source_event(tenant_id: Uuid) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(
        tenant_id,
        Uuid::new_v4(),
        source_slug(),
        event_type(),
        1,
    )
    .expect("sparse source event must stay valid")
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("sparse source slug must stay valid")
}

fn event_type() -> NotificationTypeKey {
    NotificationTypeKey::new(EVENT_TYPE).expect("sparse event type must stay valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE).expect("sparse notification type must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_sparse_fanout_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("sparse fan-out sqlite database should connect");
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

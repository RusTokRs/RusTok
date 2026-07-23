use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_notifications::api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudiencePage,
    NotificationOpenAuthorization, NotificationPriority, NotificationProviderError,
    NotificationProviderResult, NotificationSemanticDescriptor, NotificationSourceProvider,
    NotificationSourceRegistry, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTargetRoute, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey, ResolveNotificationAudienceRequest,
};
use rustok_notifications::entities::{fanout_item, fanout_job, notification};
use rustok_notifications::model::{FanoutItemStatus, NotificationJobStatus};
use rustok_notifications::{
    MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE, NotificationCandidatePolicyDeferral,
    NotificationCandidateWorker, NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyError, NotificationRecipientPolicyRequest, NotificationsModule,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "worker-test-source";
const NOTIFICATION_TYPE: &str = "worker.test.notification";

#[derive(Clone)]
struct AllowPolicy;

#[async_trait]
impl NotificationRecipientPolicy for AllowPolicy {
    async fn evaluate(
        &self,
        _request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        Ok(NotificationRecipientPolicyDecision::Allow)
    }
}

#[derive(Clone)]
struct WorkerSourceProvider;

#[async_trait]
impl NotificationSourceProvider for WorkerSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Candidate worker test source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![notification_type()]
    }

    async fn describe_event(
        &self,
        _request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        Err(NotificationProviderError::Rejected)
    }

    async fn resolve_audience(
        &self,
        _request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        Err(NotificationProviderError::Rejected)
    }

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        Ok(NotificationOpenAuthorization::Allowed {
            route: NotificationTargetRoute::new(format!(
                "/modules/worker-test?target={}",
                request.target.id
            ))
            .expect("worker test route must stay valid"),
        })
    }
}

#[tokio::test]
async fn worker_selection_is_bounded_and_uses_candidate_lease_path() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;

    let mut item_ids = Vec::new();
    for _ in 0..40 {
        let recipient_id = Uuid::new_v4();
        insert_user(&db, tenant_id, recipient_id).await;
        item_ids.push(seed_candidate(&db, tenant_id, recipient_id).await);
    }

    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(WorkerSourceProvider)
        .expect("worker source provider should register");
    let worker = NotificationCandidateWorker::new(
        db.clone(),
        Arc::new(registry),
        Arc::new(AllowPolicy),
        "candidate-worker-sqlite",
        32,
    )
    .expect("bounded worker should compose");

    let selected = worker
        .claimable_candidate_work()
        .await
        .expect("claimable candidate selection should succeed");
    assert_eq!(selected.len(), 32);
    assert!(selected.iter().all(|work| work.tenant_id == tenant_id));
    assert!(selected.iter().all(|work| item_ids.contains(&work.item_id)));

    let processed = worker
        .process_candidate(selected[0].item_id)
        .await
        .expect("worker must process through the canonical candidate service");
    assert_eq!(processed.status, FanoutItemStatus::Processed);
    assert!(processed.notification_id.is_some());

    let row = fanout_item::Entity::find_by_id(selected[0].item_id)
        .one(&db)
        .await
        .expect("candidate row should be readable")
        .expect("candidate row should exist");
    assert_eq!(row.status, FanoutItemStatus::Processed);
    assert_eq!(
        notification::Entity::find()
            .count(&db)
            .await
            .expect("notification count should succeed"),
        1
    );

    let oversized = NotificationCandidateWorker::new(
        db,
        Arc::new(NotificationSourceRegistry::default()),
        Arc::new(AllowPolicy),
        "candidate-worker-oversized",
        MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE + 1,
    )
    .expect_err("worker batches above the hard limit must be rejected");
    assert_eq!(oversized.stable_code(), "NOTIFICATION_VALIDATION_ERROR");
}

#[tokio::test]
async fn tenant_policy_deferral_removes_candidate_from_bounded_head() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;

    for _ in 0..2 {
        let recipient_id = Uuid::new_v4();
        insert_user(&db, tenant_id, recipient_id).await;
        seed_candidate(&db, tenant_id, recipient_id).await;
    }

    let worker = NotificationCandidateWorker::new(
        db.clone(),
        Arc::new(NotificationSourceRegistry::default()),
        Arc::new(AllowPolicy),
        "candidate-policy-deferral",
        1,
    )
    .expect("candidate deferral worker should compose");

    let first_page = worker
        .claimable_candidate_work()
        .await
        .expect("first bounded candidate page should load");
    assert_eq!(first_page.len(), 1);
    let deferred = first_page[0];

    worker
        .defer_candidate(
            deferred,
            NotificationCandidatePolicyDeferral::TenantDisabled,
        )
        .await
        .expect("disabled tenant candidate should receive durable backoff");

    let deferred_row = fanout_item::Entity::find_by_id(deferred.item_id)
        .one(&db)
        .await
        .expect("deferred candidate should load")
        .expect("deferred candidate should exist");
    assert_eq!(deferred_row.status, FanoutItemStatus::RetryableError);
    assert_eq!(deferred_row.attempt_count, 1);
    assert!(deferred_row.next_attempt_at.is_some());
    assert!(deferred_row.lease_owner.is_none());
    assert!(deferred_row.lease_expires_at.is_none());
    assert_eq!(
        deferred_row.last_error_code.as_deref(),
        Some("NOTIFICATION_TENANT_CAPABILITY_DISABLED")
    );

    let next_page = worker
        .claimable_candidate_work()
        .await
        .expect("later enabled work should reach bounded head");
    assert_eq!(next_page.len(), 1);
    assert_ne!(next_page[0].item_id, deferred.item_id);
    assert_eq!(
        notification::Entity::find()
            .count(&db)
            .await
            .expect("tenant deferral must not create notifications"),
        0
    );
}

async fn seed_candidate(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    recipient_id: Uuid,
) -> Uuid {
    let now = Utc::now().fixed_offset();
    let target_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let descriptor = NotificationSemanticDescriptor {
        notification_type: notification_type(),
        template_key: NotificationTemplateKey::new(NOTIFICATION_TYPE)
            .expect("worker test template key must stay valid"),
        target: NotificationTargetRef {
            owner: source_slug(),
            kind: NotificationTargetKind::new("worker.test.target")
                .expect("worker test target kind must stay valid"),
            id: target_id,
        },
        actor_id: None,
        priority: NotificationPriority::Normal,
        template_data: NotificationTemplateData::try_new(BTreeMap::from([(
            "target_id".to_string(),
            target_id.to_string(),
        )]))
        .expect("worker test template data must stay bounded"),
    };
    fanout_job::ActiveModel {
        id: Set(job_id),
        tenant_id: Set(tenant_id),
        source_slug: Set(SOURCE.to_string()),
        source_event_id: Set(Uuid::new_v4()),
        source_revision: Set(1),
        notification_type: Set(NOTIFICATION_TYPE.to_string()),
        descriptor_json: Set(serde_json::to_value(descriptor).expect("descriptor should serialize")),
        audience_cursor: Set(None),
        status: Set(NotificationJobStatus::Completed),
        attempt_count: Set(0),
        next_attempt_at: Set(None),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        last_error_message: Set(None),
        completed_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("worker fan-out job fixture should persist");

    let item_id = Uuid::new_v4();
    fanout_item::ActiveModel {
        id: Set(item_id),
        tenant_id: Set(tenant_id),
        fanout_job_id: Set(job_id),
        recipient_id: Set(recipient_id),
        status: Set(FanoutItemStatus::Pending),
        notification_id: Set(None),
        idempotency_key: Set(format!("worker:{job_id}:{recipient_id}")),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        processed_at: Set(None),
    }
    .insert(db)
    .await
    .expect("worker candidate fixture should persist");
    item_id
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("worker source slug must stay valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE)
        .expect("worker notification type must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_candidate_worker_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("candidate worker sqlite database should connect");
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

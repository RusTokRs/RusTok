use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_notifications::api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest,
    NotificationAudiencePage, NotificationOpenAuthorization, NotificationPriority,
    NotificationProviderError, NotificationProviderResult, NotificationSemanticDescriptor,
    NotificationSourceProvider, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRef, NotificationTargetRoute,
    NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
    ResolveNotificationAudienceRequest,
};
use rustok_notifications::entities::{
    delivery_attempt, fanout_item, fanout_job, notification, preference,
};
use rustok_notifications::model::{
    DigestMode, FanoutItemStatus, NotificationDeliveryMode, NotificationJobStatus,
};
use rustok_notifications::{
    NotificationCandidateService, NotificationRecipientPolicy,
    NotificationRecipientPolicyDecision, NotificationRecipientPolicyError,
    NotificationRecipientPolicyRequest, NotificationRecipientSuppression, NotificationsModule,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "test-source";
const NOTIFICATION_TYPE: &str = "test.notification";

#[derive(Clone)]
struct StaticPolicy {
    result: Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError>,
}

#[async_trait]
impl NotificationRecipientPolicy for StaticPolicy {
    async fn evaluate(
        &self,
        _request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        self.result
    }
}

#[derive(Clone)]
struct TestSourceProvider {
    unavailable_target: Uuid,
}

#[async_trait]
impl NotificationSourceProvider for TestSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Candidate test source"
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
        if request.target.id == self.unavailable_target {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        Ok(NotificationOpenAuthorization::Allowed {
            route: NotificationTargetRoute::new(format!(
                "/modules/test?target={}",
                request.target.id
            ))
            .expect("test target route must remain valid"),
        })
    }
}

#[tokio::test]
async fn candidates_require_preference_privacy_and_source_authorization() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;

    let allowed_recipient = Uuid::new_v4();
    let preference_recipient = Uuid::new_v4();
    let blocked_recipient = Uuid::new_v4();
    let unavailable_recipient = Uuid::new_v4();
    let retryable_recipient = Uuid::new_v4();
    for recipient in [
        allowed_recipient,
        preference_recipient,
        blocked_recipient,
        unavailable_recipient,
        retryable_recipient,
    ] {
        insert_user(&db, tenant_id, recipient).await;
    }

    let unavailable_target = Uuid::new_v4();
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(TestSourceProvider { unavailable_target })
        .expect("test source provider should register");
    let registry = Arc::new(registry);

    let allowed_item = seed_candidate(&db, tenant_id, allowed_recipient, Uuid::new_v4()).await;
    let preference_item =
        seed_candidate(&db, tenant_id, preference_recipient, Uuid::new_v4()).await;
    let blocked_item = seed_candidate(&db, tenant_id, blocked_recipient, Uuid::new_v4()).await;
    let unavailable_item =
        seed_candidate(&db, tenant_id, unavailable_recipient, unavailable_target).await;
    let retryable_item =
        seed_candidate(&db, tenant_id, retryable_recipient, Uuid::new_v4()).await;

    seed_preference(
        &db,
        tenant_id,
        preference_recipient,
        "*",
        "*",
        NotificationDeliveryMode::Instant,
        true,
    )
    .await;
    seed_preference(
        &db,
        tenant_id,
        preference_recipient,
        SOURCE,
        NOTIFICATION_TYPE,
        NotificationDeliveryMode::Off,
        true,
    )
    .await;

    let allow_service = NotificationCandidateService::new(
        db.clone(),
        registry.clone(),
        Arc::new(StaticPolicy {
            result: Ok(NotificationRecipientPolicyDecision::Allow),
        }),
    );
    let allowed = allow_service
        .process_candidate(allowed_item, "candidate-allow")
        .await
        .expect("allowed candidate should create a notification");
    assert_eq!(allowed.status, FanoutItemStatus::Processed);
    let notification_id = allowed
        .notification_id
        .expect("processed candidate must link a notification");

    let replay = allow_service
        .process_candidate(allowed_item, "candidate-replay")
        .await
        .expect("processed candidate should replay");
    assert!(replay.replayed);
    assert_eq!(replay.notification_id, Some(notification_id));

    let preference_skip = allow_service
        .process_candidate(preference_item, "candidate-preference")
        .await
        .expect("exact disabled preference should skip the candidate");
    assert_eq!(preference_skip.status, FanoutItemStatus::Skipped);
    assert!(preference_skip.notification_id.is_none());

    let blocked_service = NotificationCandidateService::new(
        db.clone(),
        registry.clone(),
        Arc::new(StaticPolicy {
            result: Ok(NotificationRecipientPolicyDecision::Suppress {
                reason: NotificationRecipientSuppression::Blocked,
            }),
        }),
    );
    let blocked = blocked_service
        .process_candidate(blocked_item, "candidate-blocked")
        .await
        .expect("blocked recipient should be suppressed");
    assert_eq!(blocked.status, FanoutItemStatus::Skipped);

    let unavailable = allow_service
        .process_candidate(unavailable_item, "candidate-unavailable")
        .await
        .expect("unavailable source target should be suppressed");
    assert_eq!(unavailable.status, FanoutItemStatus::Skipped);

    let retryable_service = NotificationCandidateService::new(
        db.clone(),
        registry,
        Arc::new(StaticPolicy {
            result: Err(NotificationRecipientPolicyError::retryable()),
        }),
    );
    let retryable_error = retryable_service
        .process_candidate(retryable_item, "candidate-retryable")
        .await
        .expect_err("retryable privacy failure must not create a notification");
    assert_eq!(
        retryable_error.stable_code(),
        "NOTIFICATION_RECIPIENT_POLICY_FAILURE"
    );

    let retryable_row = fanout_item::Entity::find_by_id(retryable_item)
        .one(&db)
        .await
        .expect("retryable candidate read should succeed")
        .expect("retryable candidate should exist");
    assert_eq!(retryable_row.status, FanoutItemStatus::RetryableError);
    assert!(retryable_row.notification_id.is_none());
    assert!(retryable_row.processed_at.is_none());

    let notification_rows = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("notification rows should be readable");
    assert_eq!(notification_rows.len(), 1);
    assert_eq!(notification_rows[0].id, notification_id);
    assert_eq!(notification_rows[0].recipient_id, allowed_recipient);

    let delivery_count = delivery_attempt::Entity::find()
        .filter(delivery_attempt::Column::TenantId.eq(tenant_id))
        .count(&db)
        .await
        .expect("delivery attempt count should succeed");
    assert_eq!(
        delivery_count, 0,
        "candidate finalization must not enqueue channel deliveries"
    );
}

async fn seed_candidate(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    recipient_id: Uuid,
    target_id: Uuid,
) -> Uuid {
    let now = Utc::now().fixed_offset();
    let job_id = Uuid::new_v4();
    let descriptor = NotificationSemanticDescriptor {
        notification_type: notification_type(),
        template_key: NotificationTemplateKey::new(NOTIFICATION_TYPE)
            .expect("test template key must stay valid"),
        target: NotificationTargetRef {
            owner: source_slug(),
            kind: NotificationTargetKind::new("test.target")
                .expect("test target kind must stay valid"),
            id: target_id,
        },
        actor_id: None,
        priority: NotificationPriority::Normal,
        template_data: NotificationTemplateData::try_new(BTreeMap::from([(
            "target_id".to_string(),
            target_id.to_string(),
        )]))
        .expect("test template data must stay bounded"),
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
    .expect("fan-out job fixture should persist");

    let item_id = Uuid::new_v4();
    fanout_item::ActiveModel {
        id: Set(item_id),
        tenant_id: Set(tenant_id),
        fanout_job_id: Set(job_id),
        recipient_id: Set(recipient_id),
        status: Set(FanoutItemStatus::Pending),
        notification_id: Set(None),
        idempotency_key: Set(format!("fanout:{job_id}:{recipient_id}")),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        processed_at: Set(None),
    }
    .insert(db)
    .await
    .expect("fan-out candidate fixture should persist");
    item_id
}

#[allow(clippy::too_many_arguments)]
async fn seed_preference(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    source_scope: &str,
    type_scope: &str,
    delivery_mode: NotificationDeliveryMode,
    in_app_enabled: bool,
) {
    let now = Utc::now().fixed_offset();
    preference::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        source_scope: Set(source_scope.to_string()),
        type_scope: Set(type_scope.to_string()),
        delivery_mode: Set(delivery_mode),
        in_app_enabled: Set(in_app_enabled),
        email_enabled: Set(false),
        push_enabled: Set(false),
        sms_enabled: Set(false),
        digest_mode: Set(DigestMode::Daily),
        timezone: Set("UTC".to_string()),
        quiet_start_minute: Set(None),
        quiet_end_minute: Set(None),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("notification preference fixture should persist");
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("test source slug must stay valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE)
        .expect("test notification type must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_candidate_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification candidate sqlite database should connect");
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

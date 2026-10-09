use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_api::{
    PortActor, PortContext, PortError, SharedStaticModuleSettingsReader,
    SharedStaticModuleSettingsTransactionReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot, StaticModuleSettingsTransactionReader,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    FORUM_MODERATION_REPORT_CAPABILITY, ForumError, ForumModerationReportCommand,
    ForumModerationReportPort, ForumModerationReportService, ForumModule, ReplyService,
    SharedForumModerationReportPort, TopicService,
};
use rustok_moderation_api::{ModerationReasonCode, ModerationScopeRef, ModerationSubjectKind};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

/// Keeps the stored or default settings and disables the posting cooldowns, so that the
/// seeding topic and reply are not rejected by the rate limits.
fn without_posting_cooldown(mut settings: serde_json::Value) -> serde_json::Value {
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    if let Some(object) = settings.as_object_mut() {
        object.insert("rate_limit_new_topic_seconds".to_string(), serde_json::json!(0));
        object.insert("rate_limit_new_reply_seconds".to_string(), serde_json::json!(0));
    }
    settings
}

struct TestForumSettingsReader;

#[async_trait]
impl StaticModuleSettingsReader for TestForumSettingsReader {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: without_posting_cooldown(serde_json::json!({ "use_reactions": false })),
        }))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for TestForumSettingsReader {
    async fn settings_in_tx(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: without_posting_cooldown(serde_json::json!({ "use_reactions": false })),
        }))
    }
}

fn test_settings_providers() -> rustok_forum::ForumSettingsProviders {
    let reader = Arc::new(TestForumSettingsReader);
    rustok_forum::ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_moderation_report_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5).min_connections(1).sqlx_logging(false);
    let db = Database::connect(opts)
        .await
        .expect("failed to connect forum sqlite database");

    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .await
    .expect("tenant modules table fixture should apply");
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus, Uuid::new_v4())
}

fn write_context(tenant_id: Uuid, security: &SecurityContext) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(security.user_id.unwrap_or_else(Uuid::nil).to_string()),
        "en",
        "test-moderation-report",
    )
}

/// Records every submission and returns a fixed report id.
struct RecordingReportIntake {
    report_id: Uuid,
    submissions: Mutex<Vec<(PortContext, ForumModerationReportCommand)>>,
}

#[async_trait]
impl ForumModerationReportPort for RecordingReportIntake {
    async fn submit_forum_report(
        &self,
        context: PortContext,
        command: ForumModerationReportCommand,
    ) -> Result<Uuid, PortError> {
        self.submissions
            .lock()
            .expect("recording intake lock should not be poisoned")
            .push((context, command));
        Ok(self.report_id)
    }
}

/// Seeds one topic by `author` and one approved reply by `replier`.
async fn seed_topic_and_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    author: &SecurityContext,
    replier: &SecurityContext,
) -> (Uuid, Uuid) {
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers())
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Report me".to_string(),
                slug: Some("report-me".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    let reply = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers())
        .create(
            tenant_id,
            replier.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    (topic.id, reply.id)
}

#[tokio::test]
async fn topic_report_hands_current_revision_and_reporter_to_moderation_intake() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let reporter_id = Uuid::new_v4();
    let reporter = SecurityContext::new(UserRole::Customer, Some(reporter_id));
    let (topic_id, _reply_id) =
        seed_topic_and_reply(&db, &event_bus, tenant_id, &author, &reporter).await;

    let report_id = Uuid::new_v4();
    let recording = Arc::new(RecordingReportIntake {
        report_id,
        submissions: Mutex::new(Vec::new()),
    });
    let intake: SharedForumModerationReportPort = recording.clone();
    let service = ForumModerationReportService::new(db.clone(), None, Some(intake));

    let returned = service
        .report_topic(
            tenant_id,
            topic_id,
            reporter.clone(),
            write_context(tenant_id, &reporter),
            ModerationReasonCode::Spam,
        )
        .await
        .expect("reporter should be able to report a visible topic");
    assert_eq!(returned, report_id);

    let submissions = recording
        .submissions
        .lock()
        .expect("recording intake lock should not be poisoned");
    assert_eq!(submissions.len(), 1);
    let (context, command) = &submissions[0];
    assert_eq!(command.scope, ModerationScopeRef::platform());
    assert_eq!(command.subject.module, "forum");
    assert_eq!(command.subject.kind, ModerationSubjectKind::ForumTopic);
    assert_eq!(command.subject.id, topic_id);
    assert!(command.subject.revision >= 1);
    assert_eq!(command.reporter_id, reporter_id);
    assert_eq!(command.reason_code, ModerationReasonCode::Spam);
    assert_eq!(
        context.idempotency_key.as_deref(),
        Some(
            format!(
                "forum-report:{reporter_id}:forum_topic:{topic_id}:{}:spam",
                command.subject.revision
            )
            .as_str()
        ),
    );
}

#[tokio::test]
async fn reply_report_hands_forum_post_subject_to_moderation_intake() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let replier = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let reporter_id = Uuid::new_v4();
    let reporter = SecurityContext::new(UserRole::Customer, Some(reporter_id));
    let (_topic_id, reply_id) =
        seed_topic_and_reply(&db, &event_bus, tenant_id, &author, &replier).await;

    let recording = Arc::new(RecordingReportIntake {
        report_id: Uuid::new_v4(),
        submissions: Mutex::new(Vec::new()),
    });
    let intake: SharedForumModerationReportPort = recording.clone();
    let service = ForumModerationReportService::new(db.clone(), None, Some(intake));

    service
        .report_reply(
            tenant_id,
            reply_id,
            reporter.clone(),
            write_context(tenant_id, &reporter),
            ModerationReasonCode::Harassment,
        )
        .await
        .expect("reporter should be able to report a visible approved reply");

    let submissions = recording
        .submissions
        .lock()
        .expect("recording intake lock should not be poisoned");
    assert_eq!(submissions.len(), 1);
    let (_context, command) = &submissions[0];
    assert_eq!(command.subject.kind, ModerationSubjectKind::ForumPost);
    assert_eq!(command.subject.id, reply_id);
    assert_eq!(command.reporter_id, reporter_id);
    assert_eq!(command.reason_code, ModerationReasonCode::Harassment);
}

#[tokio::test]
async fn author_cannot_report_own_topic_and_intake_is_not_called() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let replier = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let (topic_id, _reply_id) =
        seed_topic_and_reply(&db, &event_bus, tenant_id, &author, &replier).await;

    let recording = Arc::new(RecordingReportIntake {
        report_id: Uuid::new_v4(),
        submissions: Mutex::new(Vec::new()),
    });
    let intake: SharedForumModerationReportPort = recording.clone();
    let service = ForumModerationReportService::new(db.clone(), None, Some(intake));

    let error = service
        .report_topic(
            tenant_id,
            topic_id,
            author.clone(),
            write_context(tenant_id, &author),
            ModerationReasonCode::Spam,
        )
        .await
        .expect_err("authors must not report their own topic");
    assert!(matches!(error, ForumError::Validation(_)));
    assert!(
        recording
            .submissions
            .lock()
            .expect("recording intake lock should not be poisoned")
            .is_empty()
    );
}

#[tokio::test]
async fn unknown_topic_is_reported_as_not_found_without_intake_call() {
    let (db, _event_bus, tenant_id) = setup().await;
    let reporter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let unknown_topic = Uuid::new_v4();

    let recording = Arc::new(RecordingReportIntake {
        report_id: Uuid::new_v4(),
        submissions: Mutex::new(Vec::new()),
    });
    let intake: SharedForumModerationReportPort = recording.clone();
    let service = ForumModerationReportService::new(db.clone(), None, Some(intake));

    let error = service
        .report_topic(
            tenant_id,
            unknown_topic,
            reporter.clone(),
            write_context(tenant_id, &reporter),
            ModerationReasonCode::Spam,
        )
        .await
        .expect_err("an unknown topic must not be reported");
    assert!(matches!(error, ForumError::TopicNotFound(id) if id == unknown_topic));
    assert!(
        recording
            .submissions
            .lock()
            .expect("recording intake lock should not be poisoned")
            .is_empty()
    );
}

#[tokio::test]
async fn missing_moderation_intake_fails_closed_with_capability_failure() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let reporter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let (topic_id, _reply_id) =
        seed_topic_and_reply(&db, &event_bus, tenant_id, &author, &reporter).await;

    let service = ForumModerationReportService::new(db.clone(), None, None);
    let error = service
        .report_topic(
            tenant_id,
            topic_id,
            reporter.clone(),
            write_context(tenant_id, &reporter),
            ModerationReasonCode::Spam,
        )
        .await
        .expect_err("reports must not be dropped silently when no intake is composed");
    assert!(matches!(
        &error,
        ForumError::CapabilityFailure { capability, source_code, .. }
            if *capability == FORUM_MODERATION_REPORT_CAPABILITY
                && source_code == "forum.moderation_report.unavailable"
    ));
}

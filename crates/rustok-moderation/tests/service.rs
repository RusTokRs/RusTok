use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_moderation::{
    AssignModerationCaseCommand, ModerationCasePriority, ModerationError, ModerationReasonCode,
    ModerationReporterKind, ModerationScopeRef, ModerationService, ModerationSubjectKind,
    ModerationSubjectRef, OpenModerationCaseCommand, SubmitModerationReportCommand,
};
use sea_orm::Database;
use sea_orm_migration::{MigrationTrait, MigratorTrait};
use serde_json::json;
use uuid::Uuid;

struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_moderation::migrations::migrations()
    }
}

async fn service() -> ModerationService {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    TestMigrator::up(&db, None).await.unwrap();
    ModerationService::new(db)
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(2))
}

fn report_command(actor_id: Uuid, subject_id: Uuid) -> SubmitModerationReportCommand {
    SubmitModerationReportCommand {
        scope: ModerationScopeRef::platform(),
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: subject_id,
            revision: 3,
        },
        reporter_kind: ModerationReporterKind::User,
        reporter_id: Some(actor_id),
        reason_code: ModerationReasonCode::Spam,
        description_reference: Some("media://moderation/report-note".to_string()),
        metadata: json!({"source": "test"}),
    }
}

fn case_command(subject_id: Uuid, report_id: Uuid, revision: i64) -> OpenModerationCaseCommand {
    OpenModerationCaseCommand {
        scope: ModerationScopeRef::platform(),
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: subject_id,
            revision,
        },
        queue_key: "content".to_string(),
        priority: ModerationCasePriority::Normal,
        policy_id: None,
        policy_version: 1,
        report_ids: vec![report_id],
        metadata: json!({}),
    }
}

#[tokio::test]
async fn repeated_report_command_replays_the_completed_receipt() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let context = write_context(tenant_id, actor_id, "report-replay");
    let command = report_command(actor_id, subject_id);

    let first = service
        .submit_report_replay_safe(context.clone(), command.clone())
        .await
        .unwrap();
    let replay = service
        .submit_report_replay_safe(context, command)
        .await
        .unwrap();

    assert_eq!(first.id, replay.id);
}

#[tokio::test]
async fn reused_idempotency_key_with_another_request_is_rejected() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let context = write_context(tenant_id, actor_id, "report-conflict");
    let mut changed = report_command(actor_id, Uuid::new_v4());

    service
        .submit_report_replay_safe(context.clone(), changed.clone())
        .await
        .unwrap();
    changed.reason_code = ModerationReasonCode::Fraud;
    let error = service
        .submit_report_replay_safe(context, changed)
        .await
        .unwrap_err();

    assert!(matches!(error, ModerationError::IdempotencyConflict));
}

#[tokio::test]
async fn different_commands_for_the_same_active_subject_share_one_case() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let first_report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "report-one"),
            report_command(actor_id, subject_id),
        )
        .await
        .unwrap();
    let second_report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "report-two"),
            report_command(actor_id, subject_id),
        )
        .await
        .unwrap();

    let first = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "case-one"),
            case_command(subject_id, first_report.id, 3),
        )
        .await
        .unwrap();
    let second = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "case-two"),
            case_command(subject_id, second_report.id, 3),
        )
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn source_revision_creates_a_distinct_active_case_identity() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let first_report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "revision-report-one"),
            report_command(actor_id, subject_id),
        )
        .await
        .unwrap();
    let mut changed_report_command = report_command(actor_id, subject_id);
    changed_report_command.subject.revision = 4;
    let second_report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "revision-report-two"),
            changed_report_command,
        )
        .await
        .unwrap();

    let first = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "revision-case-one"),
            case_command(subject_id, first_report.id, 3),
        )
        .await
        .unwrap();
    let second = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "revision-case-two"),
            case_command(subject_id, second_report.id, 4),
        )
        .await
        .unwrap();

    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn stale_case_revision_does_not_overwrite_assignment() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "cas-report"),
            report_command(actor_id, subject_id),
        )
        .await
        .unwrap();
    let case = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "cas-case"),
            case_command(subject_id, report.id, 3),
        )
        .await
        .unwrap();

    service
        .assign_case_replay_safe(
            write_context(tenant_id, actor_id, "cas-assign-one"),
            AssignModerationCaseCommand {
                case_id: case.id,
                expected_revision: case.revision,
                moderator_id: Uuid::new_v4(),
            },
        )
        .await
        .unwrap();
    let error = service
        .assign_case_replay_safe(
            write_context(tenant_id, actor_id, "cas-assign-two"),
            AssignModerationCaseCommand {
                case_id: case.id,
                expected_revision: case.revision,
                moderator_id: Uuid::new_v4(),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ModerationError::RevisionConflict));
}

#[tokio::test]
async fn owner_reads_are_tenant_scoped() {
    let service = service().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "tenant-report"),
            report_command(actor_id, Uuid::new_v4()),
        )
        .await
        .unwrap();

    assert!(
        service
            .get_report(Uuid::new_v4(), report.id)
            .await
            .unwrap()
            .is_none()
    );
}

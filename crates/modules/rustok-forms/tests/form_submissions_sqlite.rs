use std::error::Error;
use std::sync::Arc;

use rustok_api::{Permission, UserRole};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_forms::dto::{FormSubmissionState, ListFormSubmissionsFilter};
use rustok_forms::{FormsModule, FormsService};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::{Map, Value, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn test_fields() -> Map<String, Value> {
    json!({ "email": "lead@example.com", "message": "hello" })
        .as_object()
        .cloned()
        .expect("object literal")
}

#[tokio::test]
async fn honeypot_capture_is_stored_as_spam_without_notification_event() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = FormsService::new(db.clone(), event_bus, None);

    let mut fields = test_fields();
    fields.insert("website".to_string(), json!("http://spam.example"));
    let response = service
        .submit(
            tenant_id,
            "contact",
            None,
            Some("en"),
            fields,
            "ip-hash-input",
            None,
        )
        .await?;
    assert!(response.accepted, "honeypot answers exactly like success");

    let stored = service
        .list(
            tenant_id,
            read_security(),
            ListFormSubmissionsFilter {
                form_id: Some("contact".to_string()),
                state: Some(FormSubmissionState::Spam),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].state, FormSubmissionState::Spam);
    assert!(
        !stored[0].payload.contains_key("website"),
        "honeypot field must not be stored in the payload"
    );
    assert_eq!(
        sys_event_count(&db, "forms.submission.received").await?,
        0,
        "spam captures must not emit contract events"
    );
    Ok(())
}

#[tokio::test]
async fn accepted_submission_persists_payload_and_emits_contract_event() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = FormsService::new(db.clone(), event_bus, None);

    let response = service
        .submit(
            tenant_id,
            "contact",
            None,
            Some("en"),
            test_fields(),
            "ip-hash-input",
            None,
        )
        .await?;
    assert!(response.accepted);

    let stored = service
        .list(
            tenant_id,
            read_security(),
            ListFormSubmissionsFilter {
                form_id: Some("contact".to_string()),
                state: Some(FormSubmissionState::New),
                page: 1,
                per_page: 20,
            },
        )
        .await?;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].payload["email"], json!("lead@example.com"));
    assert_eq!(stored[0].locale, "en");
    assert_eq!(
        sys_event_count(&db, "forms.submission.received").await?,
        1,
        "accepted submissions emit forms.submission.received in the same transaction"
    );
    Ok(())
}

#[tokio::test]
async fn rate_limit_rejects_sixth_submission_from_same_identity() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = FormsService::new(db.clone(), event_bus, None);

    for _ in 0..5 {
        service
            .submit(
                tenant_id,
                "contact",
                None,
                None,
                test_fields(),
                "same-identity",
                None,
            )
            .await?;
    }
    let failure = service
        .submit(
            tenant_id,
            "contact",
            None,
            None,
            test_fields(),
            "same-identity",
            None,
        )
        .await
        .expect_err("sixth submission must be rejected");
    assert_eq!(
        failure.error_code().as_deref(),
        Some("FORM_SUBMIT_RATE_LIMITED")
    );

    service
        .submit(
            tenant_id,
            "contact",
            None,
            None,
            test_fields(),
            "other-identity",
            None,
        )
        .await
        .expect("a different identity is not rate limited");
    Ok(())
}

#[tokio::test]
async fn payload_bounds_are_enforced() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = FormsService::new(db.clone(), event_bus, None);

    let mut too_many = Map::new();
    for index in 0..51 {
        too_many.insert(format!("field-{index}"), json!("x"));
    }
    let failure = service
        .submit(tenant_id, "contact", None, None, too_many, "ip", None)
        .await
        .expect_err("payload with too many fields is rejected");
    assert_eq!(
        failure.error_code().as_deref(),
        Some("FORM_SUBMIT_PAYLOAD_INVALID")
    );

    let long_value = Map::from_iter([("message".to_string(), json!("x".repeat(4097)))]);
    let failure = service
        .submit(tenant_id, "contact", None, None, long_value, "ip", None)
        .await
        .expect_err("oversized field value is rejected");
    assert_eq!(
        failure.error_code().as_deref(),
        Some("FORM_SUBMIT_PAYLOAD_INVALID")
    );
    Ok(())
}

#[tokio::test]
async fn triage_state_machine_enforces_allowed_transitions() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let handler_id = Uuid::new_v4();
    let db = setup_db().await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = FormsService::new(db.clone(), event_bus, None);
    let security = manage_security(handler_id);

    let response = service
        .submit(tenant_id, "contact", None, None, test_fields(), "ip", None)
        .await?;

    let read = service
        .set_state(
            tenant_id,
            security.clone(),
            response.id,
            FormSubmissionState::Read,
        )
        .await?;
    assert_eq!(read.state, FormSubmissionState::Read);

    let handled = service
        .set_state(
            tenant_id,
            security.clone(),
            response.id,
            FormSubmissionState::Handled,
        )
        .await?;
    assert_eq!(handled.state, FormSubmissionState::Handled);
    assert_eq!(handled.handled_by, Some(handler_id));
    assert!(handled.handled_at.is_some());

    let failure = service
        .set_state(
            tenant_id,
            security.clone(),
            response.id,
            FormSubmissionState::Handled,
        )
        .await
        .expect_err("handled -> handled is not allowed");
    assert_eq!(
        failure.error_code().as_deref(),
        Some("FORM_SUBMISSION_STATE_INVALID")
    );

    let reopened = service
        .set_state(tenant_id, security, response.id, FormSubmissionState::Read)
        .await?;
    assert_eq!(reopened.state, FormSubmissionState::Read);
    Ok(())
}

fn read_security() -> SecurityContext {
    SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::FORMS_READ, Permission::FORMS_LIST],
    )
}

fn manage_security(handler_id: Uuid) -> SecurityContext {
    SecurityContext::from_permissions(
        UserRole::Manager,
        Some(handler_id),
        [Permission::FORMS_READ, Permission::FORMS_MANAGE],
    )
}

async fn sys_event_count(db: &DatabaseConnection, event_type: &str) -> TestResult<u64> {
    let count = SysEvents::find()
        .filter(rustok_outbox::entity::Column::EventType.eq(event_type))
        .count(db)
        .await?;
    Ok(count)
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:forms_submissions_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in FormsModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

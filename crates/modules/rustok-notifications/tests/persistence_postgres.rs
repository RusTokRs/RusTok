use rustok_core::MigrationSource;
use rustok_notifications::NotificationsModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "NOTIFICATIONS_TEST_DATABASE_URL";

#[tokio::test]
async fn notification_persistence_enforces_postgres_invariants()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(database_url) = postgres_database_url() else {
        eprintln!("{DATABASE_ENV} is not set; skipping PostgreSQL notification persistence test");
        return Ok(());
    };

    let control = connect(&database_url).await?;
    let schema = format!("rustok_notifications_{}", Uuid::new_v4().simple());
    control
        .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema}""#))
        .await?;

    let result = async {
        let db = connect(&database_url).await?;
        db.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#))
            .await?;
        db.execute_unprepared(
            r#"
            CREATE TABLE tenants (
                id UUID PRIMARY KEY
            );
            CREATE TABLE users (
                id UUID PRIMARY KEY,
                tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE
            );
            "#,
        )
        .await?;

        let manager = SchemaManager::new(&db);
        for migration in NotificationsModule.migrations() {
            migration.up(&manager).await?;
        }

        for table in [
            "notifications",
            "notification_delivery_attempts",
            "notification_fanout_jobs",
            "notification_fanout_items",
            "notification_preferences",
            "notification_digest_jobs",
            "notification_digest_items",
            "notification_push_subscriptions",
        ] {
            assert!(manager.has_table(table).await?);
        }

        let tenant = Uuid::new_v4();
        let foreign_tenant = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let foreign_user = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO tenants (id) VALUES ('{tenant}'), ('{foreign_tenant}'); \
             INSERT INTO users (id, tenant_id) VALUES \
             ('{recipient}', '{tenant}'), ('{actor}', '{tenant}'), ('{foreign_user}', '{foreign_tenant}');"
        ))
        .await?;

        let notification_id = Uuid::new_v4();
        let source_event = Uuid::new_v4();
        db.execute_unprepared(&notification_insert(
            notification_id,
            tenant,
            recipient,
            Some(actor),
            source_event,
            "unread",
            None,
            None,
            "{}",
            "pg-notification-one",
        ))
        .await?;

        assert!(db
            .execute_unprepared(&notification_insert(
                Uuid::new_v4(),
                tenant,
                recipient,
                Some(actor),
                source_event,
                "unread",
                None,
                None,
                "{}",
                "pg-notification-two",
            ))
            .await
            .is_err());

        assert!(db
            .execute_unprepared(&notification_insert(
                Uuid::new_v4(),
                tenant,
                foreign_user,
                None,
                Uuid::new_v4(),
                "unread",
                None,
                None,
                "{}",
                "pg-cross-tenant-recipient",
            ))
            .await
            .is_err());

        assert!(db
            .execute_unprepared(&notification_insert(
                Uuid::new_v4(),
                tenant,
                recipient,
                Some(foreign_user),
                Uuid::new_v4(),
                "unread",
                None,
                None,
                "{}",
                "pg-cross-tenant-actor",
            ))
            .await
            .is_err());

        assert!(db
            .execute_unprepared(&notification_insert(
                Uuid::new_v4(),
                tenant,
                recipient,
                None,
                Uuid::new_v4(),
                "read",
                None,
                Some("2026-07-21T12:00:00Z"),
                "{}",
                "pg-read-without-seen",
            ))
            .await
            .is_err());

        let oversized = serde_json::json!({"value": "x".repeat(8300)}).to_string();
        assert!(db
            .execute_unprepared(&notification_insert(
                Uuid::new_v4(),
                tenant,
                recipient,
                None,
                Uuid::new_v4(),
                "unread",
                None,
                None,
                &oversized,
                "pg-oversized",
            ))
            .await
            .is_err());

        assert!(db
            .execute_unprepared(&format!(
                "INSERT INTO notification_delivery_attempts \
                 (id, tenant_id, notification_id, recipient_id, channel, status, idempotency_key, attempt_count) \
                 VALUES ('{}', '{}', '{}', '{}', 'email', 'leased', 'pg-delivery', 0)",
                Uuid::new_v4(), tenant, notification_id, recipient
            ))
            .await
            .is_err());

        assert!(db
            .execute_unprepared(&format!(
                "INSERT INTO notification_push_subscriptions \
                 (id, tenant_id, user_id, platform, endpoint_hash, encrypted_endpoint, key_version, status, failure_count) \
                 VALUES ('{}', '{}', '{}', 'web', 'raw-endpoint', 'plaintext', 'v1', 'active', 0)",
                Uuid::new_v4(), tenant, recipient
            ))
            .await
            .is_err());

        Ok::<(), sea_orm::DbErr>(())
    }
    .await;

    let cleanup = control
        .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
        .await;
    result?;
    cleanup?;
    Ok(())
}

fn postgres_database_url() -> Option<String> {
    std::env::var(DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options).await
}

#[allow(clippy::too_many_arguments)]
fn notification_insert(
    id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    actor_id: Option<Uuid>,
    source_event_id: Uuid,
    state: &str,
    seen_at: Option<&str>,
    read_at: Option<&str>,
    payload: &str,
    idempotency_key: &str,
) -> String {
    let actor = actor_id
        .map(|id| format!("'{id}'"))
        .unwrap_or_else(|| "NULL".to_string());
    let seen = seen_at
        .map(|value| format!("'{value}'::timestamptz"))
        .unwrap_or_else(|| "NULL".to_string());
    let read = read_at
        .map(|value| format!("'{value}'::timestamptz"))
        .unwrap_or_else(|| "NULL".to_string());
    let payload = payload.replace('\'', "''");
    format!(
        "INSERT INTO notifications \
         (id, tenant_id, recipient_id, source_slug, source_event_id, source_revision, notification_type, \
          template_key, target_owner, target_kind, target_id, actor_id, priority, state, template_data_json, \
          idempotency_key, seen_at, read_at) \
         VALUES ('{id}', '{tenant_id}', '{recipient_id}', 'forum', '{source_event_id}', 1, \
          'forum.topic.created', 'forum.topic.created', 'forum', 'forum.topic', '{}', {actor}, \
          'normal', '{state}', '{payload}'::jsonb, '{idempotency_key}', {seen}, {read})",
        Uuid::new_v4()
    )
}

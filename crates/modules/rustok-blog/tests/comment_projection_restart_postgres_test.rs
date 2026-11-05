mod support;

use std::{env, error::Error, io, process::Command, sync::Arc};

use rustok_blog::services::BlogCommentProjectionHandler;
use rustok_core::events::EventHandler;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const PROCESS_WORKER_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_WORKER";
const PROCESS_DATABASE_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_DATABASE_URL";
const PROCESS_SCHEMA_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_SCHEMA";
const PROCESS_TENANT_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_TENANT_ID";
const PROCESS_POST_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_POST_ID";
const PROCESS_ACTOR_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_ACTOR_ID";
const PROCESS_COMMENT_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_COMMENT_ID";
const PROCESS_EVENT_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_EVENT_ID";

struct PostgresBlogProjectionRestartTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresBlogProjectionRestartTestDb {
    async fn setup() -> TestResult<Self> {
        let database_url = postgres_database_url()?;

        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_blog_projection_restart_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        if let Err(error) = create_projection_tables(&db).await {
            if let Err(cleanup_error) = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await
            {
                return Err(io::Error::other(format!(
                    "Blog fixture migration failed: {error}; schema cleanup failed: {cleanup_error}"
                ))
                .into());
            }
            return Err(error.into());
        }

        Ok(Self {
            control,
            db,
            database_url,
            schema_name,
        })
    }

    async fn restarted_connection(&self) -> TestResult<DatabaseConnection> {
        let db = connect(&self.database_url).await?;
        set_search_path(&db, &self.schema_name).await?;
        Ok(db)
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn restarted_handler_reuses_delivery_ledger_without_reapplying_counter() -> TestResult<()> {
    let test_db = PostgresBlogProjectionRestartTestDb::setup().await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let envelope = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentStatusChanged {
            comment_id: Uuid::new_v4(),
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
            old_status: "pending".to_string(),
            new_status: "approved".to_string(),
        },
    );
    insert_post(&test_db.db, tenant_id, post_id, actor_id).await?;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let first_handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus.clone());
    first_handler.handle(&envelope).await?;
    drop(first_handler);

    let restarted_db = test_db.restarted_connection().await?;
    let event_bus =
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(restarted_db.clone())));
    let restarted_handler = BlogCommentProjectionHandler::new(restarted_db.clone(), event_bus);
    restarted_handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&restarted_db, tenant_id, post_id).await?,
        (1, 1)
    );
    assert_eq!(count_delivery(&restarted_db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&restarted_db).await?, 1);

    drop(restarted_handler);
    drop(restarted_db);
    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn restarted_process_reuses_delivery_ledger_without_reapplying_counter() -> TestResult<()> {
    let test_db = PostgresBlogProjectionRestartTestDb::setup().await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    let envelope = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentStatusChanged {
            comment_id,
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
            old_status: "pending".to_string(),
            new_status: "approved".to_string(),
        },
    );
    insert_post(&test_db.db, tenant_id, post_id, actor_id).await?;

    run_projection_worker(
        &test_db,
        tenant_id,
        post_id,
        actor_id,
        comment_id,
        envelope.id,
    )?;
    run_projection_worker(
        &test_db,
        tenant_id,
        post_id,
        actor_id,
        comment_id,
        envelope.id,
    )?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "private child-process entrypoint; requires the parent fixture context"]
async fn process_restart_worker_applies_envelope_from_env() -> TestResult<()> {
    if env::var_os(PROCESS_WORKER_ENV).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the restart worker must be launched by its parent PostgreSQL scenario",
        )
        .into());
    }

    let database_url = required_env(PROCESS_DATABASE_ENV)?;
    let schema_name = required_env(PROCESS_SCHEMA_ENV)?;
    let tenant_id = required_uuid(PROCESS_TENANT_ENV)?;
    let post_id = required_uuid(PROCESS_POST_ENV)?;
    let actor_id = required_uuid(PROCESS_ACTOR_ENV)?;
    let comment_id = required_uuid(PROCESS_COMMENT_ENV)?;
    let event_id = required_uuid(PROCESS_EVENT_ENV)?;

    let db = connect(&database_url).await?;
    set_search_path(&db, &schema_name).await?;
    let mut envelope = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentStatusChanged {
            comment_id,
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
            old_status: "pending".to_string(),
            new_status: "approved".to_string(),
        },
    );
    envelope.id = event_id;
    envelope.correlation_id = event_id;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let handler = BlogCommentProjectionHandler::new(db, event_bus);
    handler.handle(&envelope).await?;
    Ok(())
}

fn run_projection_worker(
    test_db: &PostgresBlogProjectionRestartTestDb,
    tenant_id: Uuid,
    post_id: Uuid,
    actor_id: Uuid,
    comment_id: Uuid,
    event_id: Uuid,
) -> TestResult<()> {
    let status = Command::new(env::current_exe()?)
        .arg("--ignored")
        .arg("--exact")
        .arg("process_restart_worker_applies_envelope_from_env")
        .arg("--nocapture")
        .env(PROCESS_WORKER_ENV, "1")
        .env(PROCESS_DATABASE_ENV, &test_db.database_url)
        .env(PROCESS_SCHEMA_ENV, &test_db.schema_name)
        .env(PROCESS_TENANT_ENV, tenant_id.to_string())
        .env(PROCESS_POST_ENV, post_id.to_string())
        .env(PROCESS_ACTOR_ENV, actor_id.to_string())
        .env(PROCESS_COMMENT_ENV, comment_id.to_string())
        .env(PROCESS_EVENT_ENV, event_id.to_string())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Blog projection restart worker exited with status {status}"
        ))
        .into())
    }
}

fn required_env(name: &str) -> TestResult<String> {
    env::var(name).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("missing or invalid {name}: {error}"),
        )
        .into()
    })
}

fn required_uuid(name: &str) -> TestResult<Uuid> {
    let value = required_env(name)?;
    Uuid::parse_str(&value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid UUID in {name}: {error}"),
        )
        .into()
    })
}

async fn create_projection_tables(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    support::apply_blog_schema(db).await
}

async fn insert_post(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO blog_posts (
            id, tenant_id, author_id, status, slug, metadata, published_at,
            comment_count, version
        ) VALUES ($1, $2, $3, 'published', 'projection-restart-test', '{}'::jsonb, NOW(), 0, 1)
        "#,
        vec![post_id.into(), tenant_id.into(), author_id.into()],
    ))
    .await?;
    Ok(())
}

async fn load_post_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
) -> Result<(i32, i32), sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT comment_count, version
            FROM blog_posts
            WHERE tenant_id = $1 AND id = $2
            "#,
            vec![tenant_id.into(), post_id.into()],
        ))
        .await?
        .expect("Blog post state should exist");
    Ok((
        row.try_get("", "comment_count")?,
        row.try_get("", "version")?,
    ))
}

async fn count_delivery(db: &DatabaseConnection, event_id: Uuid) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM blog_comment_projection_deliveries WHERE event_id = $1",
            vec![event_id.into()],
        ))
        .await?
        .expect("delivery count query should return one row");
    row.try_get("", "count")
}

async fn count_outbox_events(db: &DatabaseConnection) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM sys_events".to_string(),
        ))
        .await?
        .expect("outbox count query should return one row");
    row.try_get("", "count")
}

fn postgres_database_url() -> TestResult<String> {
    support::postgres_database_url()
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
}

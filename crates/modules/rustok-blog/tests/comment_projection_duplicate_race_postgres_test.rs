use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_blog::services::BlogCommentProjectionHandler;
use rustok_core::events::{EventEnvelope, EventHandler};
use rustok_events::DomainEvent;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use tokio::sync::Barrier;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const WORKER_A_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_a";
const WORKER_B_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_b";

struct PostgresBlogProjectionDuplicateRaceTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresBlogProjectionDuplicateRaceTestDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{BLOG_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog duplicate projection race test"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "rustok_blog_duplicate_race_control").await?;
        let schema_name = format!("rustok_blog_duplicate_race_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url, "rustok_blog_duplicate_race_assertions").await?;
        set_search_path(&db, &schema_name).await?;

        if let Err(error) = create_projection_tables(&db).await {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            database_url,
            schema_name,
        }))
    }

    async fn isolated_connection(&self, application_name: &str) -> TestResult<DatabaseConnection> {
        let db = connect(&self.database_url, application_name).await?;
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
async fn concurrent_duplicate_envelope_commits_once_and_replays_cleanly() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionDuplicateRaceTestDb::setup().await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id).await?;

    let envelope = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentCreated {
            comment_id: Uuid::new_v4(),
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
        },
    );

    // Hold the post row before starting either handler. Both workers can finish
    // their delivery-ledger lookup, then block on the same optimistic UPDATE.
    let lock_db = test_db
        .isolated_connection("rustok_blog_duplicate_race_lock_holder")
        .await?;
    let lock_txn = lock_db.begin().await?;
    let locked = lock_txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM blog_posts WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
            vec![tenant_id.into(), post_id.into()],
        ))
        .await?;
    assert!(locked.is_some(), "the Blog post row must be locked");

    let worker_a = test_db
        .isolated_connection(WORKER_A_APPLICATION_NAME)
        .await?;
    let worker_b = test_db
        .isolated_connection(WORKER_B_APPLICATION_NAME)
        .await?;
    let start = Arc::new(Barrier::new(2));

    let task_a = spawn_projection(worker_a, Arc::clone(&start), envelope.clone());
    let task_b = spawn_projection(worker_b, Arc::clone(&start), envelope.clone());

    wait_for_both_workers_to_block(&test_db.control).await?;
    lock_txn.commit().await?;

    let outcomes = [task_a.await?, task_b.await?];
    let success_count = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    let failure_count = outcomes.iter().filter(|outcome| outcome.is_err()).count();
    assert_eq!(
        success_count, 1,
        "exactly one duplicate delivery must commit"
    );
    assert_eq!(
        failure_count, 1,
        "the losing duplicate transaction must fail and roll back"
    );

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    let replay_db = test_db
        .isolated_connection("rustok_blog_duplicate_race_replay")
        .await?;
    BlogCommentProjectionHandler::new(replay_db)
        .handle(&envelope)
        .await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

fn spawn_projection(
    db: DatabaseConnection,
    start: Arc<Barrier>,
    envelope: EventEnvelope,
) -> tokio::task::JoinHandle<rustok_core::Result<()>> {
    tokio::spawn(async move {
        let handler = BlogCommentProjectionHandler::new(db);
        start.wait().await;
        handler.handle(&envelope).await
    })
}

async fn wait_for_both_workers_to_block(control: &DatabaseConnection) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let row = control
                .query_one_raw(Statement::from_string(
                    DbBackend::Postgres,
                    format!(
                        "SELECT COUNT(*)::bigint AS count \
                         FROM pg_stat_activity \
                         WHERE application_name IN ('{WORKER_A_APPLICATION_NAME}', '{WORKER_B_APPLICATION_NAME}') \
                           AND wait_event_type = 'Lock'"
                    ),
                ))
                .await?
                .expect("pg_stat_activity count query should return one row");
            let blocked: i64 = row.try_get("", "count")?;
            if blocked == 2 {
                return Ok::<(), sea_orm::DbErr>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "both duplicate projection workers did not block on the post row",
        )
    })??;
    Ok(())
}

async fn create_projection_tables(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE blog_posts (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            author_id UUID NOT NULL,
            category_id UUID NULL,
            status TEXT NOT NULL,
            slug TEXT NOT NULL,
            metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
            featured_image_url TEXT NULL,
            published_at TIMESTAMPTZ NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            archived_at TIMESTAMPTZ NULL,
            comment_count INTEGER NOT NULL DEFAULT 0,
            view_count INTEGER NOT NULL DEFAULT 0,
            version INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE blog_comment_projection_deliveries (
            event_id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            comment_id UUID NOT NULL,
            post_id UUID NOT NULL,
            delta INTEGER NOT NULL,
            processed_at TIMESTAMPTZ NOT NULL
        );

        CREATE TABLE sys_events (
            id UUID PRIMARY KEY,
            event_type TEXT NOT NULL,
            schema_version SMALLINT NOT NULL,
            payload JSONB NOT NULL,
            status VARCHAR(32) NOT NULL,
            retry_count INTEGER NOT NULL,
            next_attempt_at TIMESTAMPTZ NULL,
            last_error TEXT NULL,
            claimed_by TEXT NULL,
            claimed_at TIMESTAMPTZ NULL,
            created_at TIMESTAMPTZ NOT NULL,
            dispatched_at TIMESTAMPTZ NULL
        );
        "#,
    )
    .await?;
    Ok(())
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
            id, tenant_id, author_id, status, slug, metadata,
            comment_count, view_count, version
        ) VALUES ($1, $2, $3, 'published', 'duplicate-race-test', '{}'::jsonb, 0, 0, 1)
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
            "SELECT comment_count, version FROM blog_posts WHERE tenant_id = $1 AND id = $2",
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

fn postgres_database_url() -> Option<String> {
    std::env::var(BLOG_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str, application_name: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('application_name', $1, false)",
        vec![application_name.into()],
    ))
    .await?;
    Ok(db)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
}

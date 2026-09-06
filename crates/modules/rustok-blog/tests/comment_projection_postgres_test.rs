use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_blog::{BlogModule, services::BlogCommentProjectionHandler};
use rustok_core::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleRuntimeExtensions, RusToKModule,
    events::{DispatcherConfig, EventBus, EventDispatcher, EventHandler},
};
use rustok_events::{DomainEvent, EventEnvelope};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use tokio::sync::Barrier;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const CONCURRENT_PROJECTION_DELIVERIES: usize = 4;
const EXPECTED_RETRY_LIMIT_ATTEMPTS: i64 = 8;

struct PostgresBlogProjectionTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresBlogProjectionTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{BLOG_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog comment projection test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_blog_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
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

    async fn isolated_connection(&self) -> TestResult<DatabaseConnection> {
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
async fn duplicate_delivery_updates_counter_and_outbox_once() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("duplicate").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let envelope = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone());
    handler.handle(&envelope).await?;
    handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn event_dispatcher_routes_registered_handler_and_commits_projection() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("dispatcher").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let extensions = ModuleRuntimeExtensions::default();
    let context = ModuleEventListenerContext {
        db: test_db.db.clone(),
        extensions: &extensions,
    };
    let mut registry = ModuleEventListenerRegistry::new();
    BlogModule.register_event_listeners(&mut registry, &context);

    let bus = EventBus::new();
    let mut dispatcher = EventDispatcher::with_config(
        bus,
        DispatcherConfig {
            fail_fast: true,
            max_concurrent: 1,
            retry_count: 0,
            retry_delay_ms: 0,
            max_queue_depth: 128,
        },
    );
    for handler in registry.into_handlers() {
        dispatcher.register_boxed(handler);
    }
    assert_eq!(dispatcher.handler_count(), 1);

    let envelope = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    let running = dispatcher.start();
    running.bus().publish_envelope(envelope.clone())?;
    wait_for_dispatch_commit(&test_db.db, envelope.id).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    running.stop();
    test_db.cleanup().await
}

#[tokio::test]
async fn concurrent_created_events_converge_without_lost_updates() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("concurrent_created").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let envelopes = (0..CONCURRENT_PROJECTION_DELIVERIES)
        .map(|_| comment_created_envelope(tenant_id, actor_id, Uuid::new_v4(), post_id))
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(envelopes.len()));
    let mut tasks = Vec::with_capacity(envelopes.len());

    for envelope in &envelopes {
        let db = test_db.isolated_connection().await?;
        let barrier = Arc::clone(&barrier);
        let envelope = envelope.clone();
        tasks.push(tokio::spawn(async move {
            let handler = BlogCommentProjectionHandler::new(db);
            barrier.wait().await;
            handler.handle(&envelope).await
        }));
    }

    for task in tasks {
        task.await??;
    }

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (
            CONCURRENT_PROJECTION_DELIVERIES as i32,
            CONCURRENT_PROJECTION_DELIVERIES as i32 + 1,
        )
    );
    for envelope in &envelopes {
        assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    }
    assert_eq!(
        count_all_deliveries(&test_db.db).await?,
        CONCURRENT_PROJECTION_DELIVERIES as i64
    );
    assert_eq!(
        count_outbox_events(&test_db.db).await?,
        CONCURRENT_PROJECTION_DELIVERIES as i64
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("retry_limit").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;
    install_retry_limit_probe(&test_db.db).await?;

    let envelope = comment_created_envelope(tenant_id, actor_id, Uuid::new_v4(), post_id);
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone());

    let error = handler
        .handle(&envelope)
        .await
        .expect_err("eight zero-row updates must reach the optimistic retry limit");
    assert!(error.to_string().contains("after 8 concurrent attempts"));
    assert_eq!(
        load_retry_attempt_count(&test_db.db).await?,
        EXPECTED_RETRY_LIMIT_ATTEMPTS
    );
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 0);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    remove_retry_limit_probe(&test_db.db).await?;
    handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn delete_before_create_stays_non_negative_and_replays_in_order() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("out_of_order").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let handler = BlogCommentProjectionHandler::new(test_db.db.clone());
    let deleted = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentDeleted {
            comment_id: Uuid::new_v4(),
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
        },
    );
    handler.handle(&deleted).await?;
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 2)
    );

    let created = comment_created_envelope(tenant_id, actor_id, Uuid::new_v4(), post_id);
    handler.handle(&created).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 3)
    );
    assert_eq!(count_delivery(&test_db.db, deleted.id).await?, 1);
    assert_eq!(count_delivery(&test_db.db, created.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 2);

    test_db.cleanup().await
}

#[tokio::test]
async fn missing_post_replay_commits_only_after_source_appears() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("missing_post").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let envelope = comment_created_envelope(tenant_id, actor_id, Uuid::new_v4(), post_id);
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone());

    let error = handler
        .handle(&envelope)
        .await
        .expect_err("missing Blog post must keep the delivery retryable");
    assert!(error.to_string().contains("was not found"));
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 0);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;
    handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn outbox_failure_rolls_back_counter_and_delivery_before_retry() -> TestResult<()> {
    let Some(test_db) = PostgresBlogProjectionTestDb::setup("outbox_rollback").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let envelope = comment_created_envelope(tenant_id, actor_id, Uuid::new_v4(), post_id);
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone());

    test_db
        .db
        .execute_unprepared("DROP TABLE sys_events")
        .await?;
    handler
        .handle(&envelope)
        .await
        .expect_err("missing outbox table must fail the projection transaction");

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 0);

    create_outbox_table(&test_db.db).await?;
    handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

fn comment_created_envelope(
    tenant_id: Uuid,
    actor_id: Uuid,
    comment_id: Uuid,
    post_id: Uuid,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentCreated {
            comment_id,
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
        },
    )
}

async fn wait_for_dispatch_commit(db: &DatabaseConnection, event_id: Uuid) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if count_delivery(db, event_id).await? == 1 {
                return Ok::<(), sea_orm::DbErr>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("event dispatcher did not commit delivery {event_id}"),
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
        "#,
    )
    .await?;
    create_outbox_table(db).await
}

async fn create_outbox_table(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
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
        )
        "#,
    )
    .await?;
    Ok(())
}

async fn install_retry_limit_probe(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE SEQUENCE blog_projection_retry_attempts START WITH 1;

        CREATE FUNCTION force_blog_projection_retry_limit()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            PERFORM nextval('blog_projection_retry_attempts');
            RETURN NULL;
        END;
        $$;

        CREATE TRIGGER force_blog_projection_retry_limit
        BEFORE UPDATE OF comment_count, version ON blog_posts
        FOR EACH ROW
        EXECUTE FUNCTION force_blog_projection_retry_limit();
        "#,
    )
    .await?;
    Ok(())
}

async fn remove_retry_limit_probe(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        DROP TRIGGER force_blog_projection_retry_limit ON blog_posts;
        DROP FUNCTION force_blog_projection_retry_limit();
        "#,
    )
    .await?;
    Ok(())
}

async fn load_retry_attempt_count(db: &DatabaseConnection) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT last_value::bigint AS count FROM blog_projection_retry_attempts".to_string(),
        ))
        .await?
        .expect("retry-attempt sequence should return one row");
    row.try_get("", "count")
}

async fn insert_post(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
    comment_count: i32,
    version: i32,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO blog_posts (
            id, tenant_id, author_id, status, slug, metadata,
            comment_count, view_count, version
        ) VALUES ($1, $2, $3, 'published', 'projection-test', '{}'::jsonb, $4, 0, $5)
        "#,
        vec![
            post_id.into(),
            tenant_id.into(),
            author_id.into(),
            comment_count.into(),
            version.into(),
        ],
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

async fn count_all_deliveries(db: &DatabaseConnection) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM blog_comment_projection_deliveries".to_string(),
        ))
        .await?
        .expect("delivery total query should return one row");
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

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}

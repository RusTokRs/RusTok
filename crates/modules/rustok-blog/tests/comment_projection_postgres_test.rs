mod support;

use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_blog::{BlogModule, services::BlogCommentProjectionHandler};
use rustok_core::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleRuntimeExtensions, RusToKModule,
    events::{DispatcherConfig, EventBus, EventDispatcher, EventHandler},
};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use tokio::sync::Barrier;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const CONCURRENT_PROJECTION_DELIVERIES: usize = 4;

struct PostgresBlogProjectionTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresBlogProjectionTestDb {
    async fn setup(prefix: &str) -> TestResult<Self> {
        let database_url = postgres_database_url()?;

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
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn duplicate_delivery_updates_counter_and_outbox_once() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("duplicate").await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let envelope = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus);
    handler.handle(&envelope).await?;
    handler.handle(&envelope).await?;

    // A created comment is moderation-pending: it is delivered once but does
    // not enter the public counter and requests no reindex.
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    // The approval is the counted transition; its duplicate changes nothing.
    let approved = EventEnvelope::new(
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
    handler.handle(&approved).await?;
    handler.handle(&approved).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 1)
    );
    assert_eq!(count_delivery(&test_db.db, approved.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn event_dispatcher_routes_registered_handler_and_commits_projection() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("dispatcher").await?;

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
    let projection = registry
        .into_handlers()
        .into_iter()
        .find(|handler| handler.name() == "blog_comment_projection")
        .expect("module registration must supply the canonical projection handler");
    dispatcher.register_boxed(projection);
    assert_eq!(dispatcher.handler_count(), 1);

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
    let running = dispatcher.start();
    running.bus().publish_envelope(envelope.clone())?;
    wait_for_dispatch_commit(&test_db.db, envelope.id).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    running.stop().await.expect("dispatcher stop");
    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn concurrent_created_events_converge_without_lost_updates() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("concurrent_created").await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    // Approvals of distinct comments are the counted transitions; racing them
    // must converge to one increment per comment without lost updates.
    let envelopes = (0..CONCURRENT_PROJECTION_DELIVERIES)
        .map(|_| {
            EventEnvelope::new(
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
            )
        })
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(envelopes.len()));
    let mut tasks = Vec::with_capacity(envelopes.len());

    for envelope in &envelopes {
        let db = test_db.isolated_connection().await?;
        let barrier = Arc::clone(&barrier);
        let envelope = envelope.clone();
        tasks.push(tokio::spawn(async move {
            let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
            let handler = BlogCommentProjectionHandler::new(db, event_bus);
            barrier.wait().await;
            handler.handle(&envelope).await
        }));
    }

    for task in tasks {
        task.await??;
    }

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (CONCURRENT_PROJECTION_DELIVERIES as i32, 1)
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
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn approval_counts_once_and_update_events_preserve_the_count() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("lifecycle_cursor").await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus);
    let created = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    handler.handle(&created).await?;

    let updated = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentUpdated {
            comment_id,
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
        },
    );
    handler.handle(&updated).await?;

    let status_changed = EventEnvelope::new(
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
    handler.handle(&status_changed).await?;

    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 1)
    );
    assert_eq!(count_all_deliveries(&test_db.db).await?, 3);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);
    assert_eq!(
        latest_delivery_event_id(&test_db.db, tenant_id, post_id).await?,
        status_changed.id
    );

    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn delete_before_create_stays_non_negative_and_replays_in_order() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("out_of_order").await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus);
    let deleted = EventEnvelope::new(
        tenant_id,
        Some(actor_id),
        DomainEvent::CommentDeleted {
            comment_id,
            target_type: "blog_post".to_string(),
            target_id: post_id,
            author_id: actor_id,
        },
    );
    handler.handle(&deleted).await?;
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 1)
    );

    let created = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    handler.handle(&created).await?;

    // The replayed create is newer than the delete, so it is recorded, but a
    // pending comment never enters the public counter.
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (0, 1)
    );
    assert_eq!(count_delivery(&test_db.db, deleted.id).await?, 1);
    assert_eq!(count_delivery(&test_db.db, created.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn missing_post_delivery_is_terminal_without_cross_tenant_effects() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("missing_post").await?;

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
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus);

    // Comments can deliver a previously committed event after Blog deletion.
    // A terminal missing target must acknowledge the obsolete event; retrying
    // it indefinitely cannot recreate the owner and would poison the consumer.
    handler.handle(&envelope).await?;
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 0);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    let other_tenant_id = Uuid::new_v4();
    insert_post(&test_db.db, other_tenant_id, post_id, actor_id, 3, 1).await?;
    handler.handle(&envelope).await?;

    assert_eq!(
        load_post_state(&test_db.db, other_tenant_id, post_id).await?,
        (3, 1)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 0);
    assert_eq!(count_outbox_events(&test_db.db).await?, 0);

    test_db.cleanup().await
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL"]
async fn outbox_failure_rolls_back_counter_and_delivery_before_retry() -> TestResult<()> {
    let test_db = PostgresBlogProjectionTestDb::setup("outbox_rollback").await?;

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id, 0, 1).await?;

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
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(test_db.db.clone())));
    let handler = BlogCommentProjectionHandler::new(test_db.db.clone(), event_bus);

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
        (1, 1)
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
    support::apply_blog_schema(db).await
}

async fn create_outbox_table(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    sea_orm_migration::MigrationTrait::up(
        &rustok_outbox::SysEventsMigration,
        &sea_orm_migration::SchemaManager::new(db),
    )
    .await
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
            id, tenant_id, author_id, status, slug, metadata, published_at,
            comment_count, version
        ) VALUES ($1, $2, $3, 'published', 'projection-test', '{}'::jsonb, NOW(), $4, $5)
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

async fn latest_delivery_event_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
) -> Result<Uuid, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT event_id
            FROM blog_comment_projection_deliveries
            WHERE tenant_id = $1 AND post_id = $2
            ORDER BY event_id DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), post_id.into()],
        ))
        .await?
        .expect("latest delivery query should return one row");
    row.try_get("", "event_id")
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

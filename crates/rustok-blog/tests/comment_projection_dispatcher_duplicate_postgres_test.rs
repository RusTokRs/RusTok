use std::{
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_blog::BlogModule;
use rustok_core::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleRuntimeExtensions, RusToKModule,
    events::{DispatcherConfig, EventBus, EventDispatcher, EventHandler, HandlerResult},
};
use rustok_events::{DomainEvent, EventEnvelope};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const DISPATCHER_DUPLICATE_DELIVERIES: usize = 2;

struct PostgresBlogDispatcherDuplicateTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresBlogDispatcherDuplicateTestDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{BLOG_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog dispatcher duplicate delivery test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_blog_dispatcher_duplicate_{}",
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
            schema_name,
        }))
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

struct ObservedProjectionHandler {
    inner: Arc<dyn EventHandler>,
    completed: Arc<AtomicUsize>,
    failed: Arc<AtomicUsize>,
}

impl ObservedProjectionHandler {
    fn new(
        inner: Arc<dyn EventHandler>,
        completed: Arc<AtomicUsize>,
        failed: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner,
            completed,
            failed,
        }
    }
}

#[async_trait]
impl EventHandler for ObservedProjectionHandler {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        self.inner.handles(event)
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let result = self.inner.handle(envelope).await;
        if result.is_err() {
            self.failed.fetch_add(1, Ordering::SeqCst);
        }
        self.completed.fetch_add(1, Ordering::SeqCst);
        result
    }

    async fn on_error(&self, envelope: &EventEnvelope, error: &rustok_core::Error) {
        self.inner.on_error(envelope, error).await;
    }
}

#[tokio::test]
async fn event_dispatcher_replays_duplicate_envelope_without_double_commit() -> TestResult<()> {
    let Some(test_db) = PostgresBlogDispatcherDuplicateTestDb::setup().await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let comment_id = Uuid::new_v4();
    insert_post(&test_db.db, tenant_id, post_id, actor_id).await?;

    let extensions = ModuleRuntimeExtensions::default();
    let context = ModuleEventListenerContext {
        db: test_db.db.clone(),
        extensions: &extensions,
    };
    let mut registry = ModuleEventListenerRegistry::new();
    BlogModule.register_event_listeners(&mut registry, &context);

    let mut handlers = registry.into_handlers();
    assert_eq!(handlers.len(), 1);
    let projection = handlers
        .pop()
        .expect("Blog must register the comment projection handler");
    assert_eq!(projection.name(), "blog_comment_projection");

    let completed = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let observed =
        ObservedProjectionHandler::new(projection, Arc::clone(&completed), Arc::clone(&failed));
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
    dispatcher.register(observed);
    assert_eq!(dispatcher.handler_count(), 1);

    let envelope = comment_created_envelope(tenant_id, actor_id, comment_id, post_id);
    let running = dispatcher.start();
    for _ in 0..DISPATCHER_DUPLICATE_DELIVERIES {
        running.bus().publish_envelope(envelope.clone())?;
    }
    wait_for_completed_dispatches(&completed).await?;

    assert_eq!(
        completed.load(Ordering::SeqCst),
        DISPATCHER_DUPLICATE_DELIVERIES
    );
    assert_eq!(failed.load(Ordering::SeqCst), 0);
    assert_eq!(
        load_post_state(&test_db.db, tenant_id, post_id).await?,
        (1, 2)
    );
    assert_eq!(count_delivery(&test_db.db, envelope.id).await?, 1);
    assert_eq!(count_outbox_events(&test_db.db).await?, 1);

    running.stop();
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

async fn wait_for_completed_dispatches(completed: &AtomicUsize) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if completed.load(Ordering::SeqCst) >= DISPATCHER_DUPLICATE_DELIVERIES {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "event dispatcher did not complete both duplicate deliveries",
        )
    })?;
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
        ) VALUES ($1, $2, $3, 'published', 'dispatcher-duplicate-test', '{}'::jsonb, 0, 0, 1)
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

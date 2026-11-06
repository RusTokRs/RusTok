mod support;

use std::{sync::Arc, time::Duration};

use rustok_api::{Permission, RichTextDocument};
use rustok_blog::{BlogError, BlogPublicError, CreatePostInput, PostService};
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_test_utils::db::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

fn service(db: DatabaseConnection) -> PostService {
    PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db))),
    )
}

fn input(slug: &str) -> CreatePostInput {
    CreatePostInput {
        command_id: Uuid::new_v4(),
        locale: "en".into(),
        title: "Replayable post".into(),
        content: RichTextDocument::single_paragraph("Original body"),
        excerpt: None,
        slug: Some(slug.into()),
        publish: false,
        tags: vec![],
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        metadata: None,
        scheduled_at: None,
    }
}

async fn cardinalities(db: &DatabaseConnection) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT (SELECT COUNT(*) FROM blog_posts)::bigint AS posts, \
             (SELECT COUNT(*) FROM sys_events)::bigint AS events",
        ))
        .await
        .unwrap()
        .unwrap();
    (
        row.try_get("", "posts").unwrap(),
        row.try_get("", "events").unwrap(),
    )
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn concurrent_creation_retries_commit_one_identity_and_one_event() {
    let control_url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&control_url).await.unwrap();
    let name = unique_postgres_database_name("blog_create_replay");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&control_url, &name);
    let result = tokio::spawn(async move {
        let db = connect_postgres(&url).await.unwrap();
        support::apply_blog_schema(&db).await.unwrap();
        let tenant = Uuid::new_v4();
        let actor = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
        let command = input("creation-replay");

        // A real database lock pauses the first owner after durable admission
        // and before its first row write. No sleep is used as concurrency proof.
        let lock = db.begin().await.unwrap();
        lock.execute_unprepared("LOCK TABLE blog_posts IN SHARE MODE")
            .await
            .unwrap();
        let application = format!("blog_create_{}", Uuid::new_v4().simple());
        let mut options = ConnectOptions::new(url.clone());
        options
            .max_connections(1)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(5))
            .acquire_timeout(Duration::from_secs(5))
            .sqlx_logging(false);
        let worker = Database::connect(options).await.unwrap();
        worker
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('application_name', $1, false)",
                [application.clone().into()],
            ))
            .await
            .unwrap();
        let worker_actor = actor.clone();
        let worker_command = command.clone();
        let first = tokio::spawn(async move {
            service(worker)
                .create_post(tenant, worker_actor, worker_command)
                .await
        });
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let waiting = db
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COUNT(*)::bigint AS count FROM pg_stat_activity \
                         WHERE application_name = $1 AND wait_event_type = 'Lock' \
                         AND query ILIKE '%INSERT%blog_posts%'",
                        [application.clone().into()],
                    ))
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get::<i64>("", "count")
                    .unwrap();
                if waiting == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("creation must be blocked at its real owner INSERT");
        let duplicate = tokio::time::timeout(
            Duration::from_secs(5),
            service(db.clone()).create_post(tenant, actor.clone(), command.clone()),
        )
        .await
        .expect("duplicate must not wait for the blocked owner write")
        .expect_err("an active receipt must not admit another creation");
        assert!(!matches!(&duplicate, BlogError::CreationRejected(_)));
        assert_eq!(BlogPublicError::from(duplicate).status, 503);
        assert_eq!(cardinalities(&db).await, (0, 0));
        lock.commit().await.unwrap();
        let id = tokio::time::timeout(Duration::from_secs(10), first)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(cardinalities(&db).await, (1, 1));

        // Reconstruct the service and pool to prove durable response replay,
        // with Create-only authority and no surviving command-local memory.
        let restarted = service(connect_postgres(&url).await.unwrap());
        let creator = SecurityContext::from_permissions(
            UserRole::Admin,
            actor.user_id,
            [Permission::BLOG_POSTS_CREATE],
        );
        assert_eq!(
            restarted
                .create_post(tenant, creator.clone(), command.clone())
                .await
                .unwrap(),
            id
        );
        assert_eq!(cardinalities(&db).await, (1, 1));
        let mut changed = command;
        changed.title = "Different payload".into();
        let error = restarted
            .create_post(tenant, creator, changed)
            .await
            .unwrap_err();
        assert_eq!(BlogPublicError::from(error).status, 409);
        assert_eq!(cardinalities(&db).await, (1, 1));
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn creation_outbox_failure_rolls_back_and_allows_the_same_command() {
    let control_url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&control_url).await.unwrap();
    let name = unique_postgres_database_name("blog_create_rollback");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&control_url, &name);
    let result = tokio::spawn(async move {
        let db = connect_postgres(&url).await.unwrap();
        support::apply_blog_schema(&db).await.unwrap();
        db.execute_unprepared(
            "CREATE FUNCTION reject_creation_event() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$; \
             CREATE TRIGGER reject_creation_event BEFORE INSERT ON sys_events \
             FOR EACH ROW EXECUTE FUNCTION reject_creation_event()",
        )
        .await
        .unwrap();
        let tenant = Uuid::new_v4();
        let actor = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
        let command = input("creation-rollback");
        let posts = service(db.clone());
        let error = posts
            .create_post(tenant, actor.clone(), command.clone())
            .await
            .unwrap_err();
        assert!(!matches!(error, BlogError::CreationRejected(_)));
        assert_eq!(cardinalities(&db).await, (0, 0));
        db.execute_unprepared("DROP TRIGGER reject_creation_event ON sys_events")
            .await
            .unwrap();
        let id = posts
            .create_post(tenant, actor.clone(), command.clone())
            .await
            .unwrap();
        assert_eq!(posts.create_post(tenant, actor, command).await.unwrap(), id);
        assert_eq!(cardinalities(&db).await, (1, 1));
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

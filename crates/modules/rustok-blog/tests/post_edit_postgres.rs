mod support;

use std::{sync::Arc, time::Duration};

use rustok_api::{Patch, RichTextDocument};
use rustok_blog::{
    BlogError, PostService,
    dto::{CreatePostInput, UpdatePostInput},
};
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
use tokio::sync::Barrier;
use uuid::Uuid;

fn service(db: DatabaseConnection) -> PostService {
    PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db))),
    )
}

#[test]
fn postgres_execution_requires_explicit_test_database_configuration() {
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "simultaneous_edits_commit_one_revision_and_one_localized_event",
            "--nocapture",
        ])
        .env_remove("RUSTOK_BLOG_TEST_DATABASE_URL")
        .env(
            "DATABASE_URL",
            "postgresql://runtime_must_not_be_used/forbidden",
        )
        .output()
        .unwrap();
    assert!(
        !child.status.success(),
        "an explicitly selected PostgreSQL scenario must not report success without its fixture URL"
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(
        output.contains("set RUSTOK_BLOG_TEST_DATABASE_URL explicitly"),
        "{output}"
    );
}

fn edit(version: i32, title: &str) -> UpdatePostInput {
    UpdatePostInput {
        locale: Some("en".into()),
        title: Some(title.into()),
        content: None,
        excerpt: Patch::Set(format!("{title} excerpt")),
        slug: None,
        tags: None,
        category_id: Patch::Keep,
        featured_image_url: Patch::Keep,
        seo_title: Patch::Keep,
        seo_description: Patch::Keep,
        channel_slugs: None,
        metadata: None,
        version,
        scheduled_at: Patch::Keep,
    }
}

async fn outbox_count(db: &DatabaseConnection) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT COUNT(*)::bigint AS count FROM sys_events",
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "count")
    .unwrap()
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn simultaneous_edits_commit_one_revision_and_one_localized_event() {
    let url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name("blog_edit_cas");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&url, &name);
    let result = tokio::spawn(async move {
        let db = connect_postgres(&url).await.unwrap();
        support::apply_blog_schema(&db).await.unwrap();
        let tenant_id = Uuid::new_v4();
        let actor = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
        let posts = service(db.clone());
        let id = posts.create_post(tenant_id, actor.clone(), CreatePostInput {
            locale: "en".into(), title: "Original".into(),
            content: RichTextDocument::single_paragraph("Original body"),
            excerpt: None, slug: Some("edit-race".into()), publish: false,
            tags: vec![], category_id: None, featured_image_url: None,
            seo_title: None, seo_description: None, channel_slugs: None,
            metadata: None, scheduled_at: None,
        }).await.unwrap();
        let original = posts.get_post(tenant_id, actor.clone(), id, "en").await.unwrap();
        let before = outbox_count(&db).await;
        let lock = db.begin().await.unwrap();
        lock.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT id FROM blog_posts WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
            [tenant_id.into(), id.into()])).await.unwrap().unwrap();

        let start = Arc::new(Barrier::new(2));
        let mut workers = Vec::new();
        let prefix = format!("blog_edit_{}", Uuid::new_v4().simple());
        for (index, title) in ["Writer A", "Writer B"].into_iter().enumerate() {
            let mut options = ConnectOptions::new(url.clone());
            options.max_connections(1).min_connections(1)
                .connect_timeout(Duration::from_secs(5))
                .acquire_timeout(Duration::from_secs(5)).sqlx_logging(false);
            let worker = Database::connect(options).await.unwrap();
            worker.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                "SELECT set_config('application_name', $1, false)",
                [format!("{prefix}_{index}").into()])).await.unwrap();
            // Keep one physical session per worker so its application name
            // identifies the actual blocked UPDATE, not another pool member.
            let worker = service(worker);
            let start = start.clone();
            let actor = actor.clone();
            let version = original.version;
            workers.push(tokio::spawn(async move {
                start.wait().await;
                worker.update_post(tenant_id, id, actor, edit(version, title)).await
            }));
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let row = db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                    "SELECT COUNT(*)::bigint AS count FROM pg_stat_activity WHERE application_name IN ($1, $2) AND wait_event_type = 'Lock'",
                    [format!("{prefix}_0").into(), format!("{prefix}_1").into()])).await.unwrap().unwrap();
                if row.try_get::<i64>("", "count").unwrap() == 2 { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("both editors must reach competing updates before the lock is released");
        lock.commit().await.unwrap();
        let first = workers.remove(0).await.unwrap();
        let second = workers.remove(0).await.unwrap();
        let (expected, loser) = match (first, second) {
            (Ok(_), Err(error)) => ("Writer A", error),
            (Err(error), Ok(_)) => ("Writer B", error),
            results => panic!("exactly one editor must commit: {results:?}"),
        };
        assert!(matches!(loser, BlogError::Conflict(_)), "{loser:?}");
        let saved = posts.get_post(tenant_id, actor.clone(), id, "en").await.unwrap();
        assert_eq!(saved.title, expected);
        assert_eq!(saved.excerpt, Some(format!("{expected} excerpt")));
        assert_eq!(saved.content_plain_text, "Original body");
        assert_eq!(saved.version, original.version + 1);
        assert_eq!(outbox_count(&db).await, before + 1);

        let denied = posts.update_post(Uuid::new_v4(), id, actor.clone(), edit(saved.version, "Foreign"))
            .await.expect_err("foreign tenant must not mutate the same owner identity");
        assert!(matches!(denied, BlogError::PostNotFound(_)));
        let unchanged = posts.get_post(tenant_id, actor, id, "en").await.unwrap();
        assert_eq!(unchanged.title, expected);
        assert_eq!(unchanged.version, saved.version);
        assert_eq!(outbox_count(&db).await, before + 1);
    }).await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

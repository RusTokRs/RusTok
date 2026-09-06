use chrono::Utc;
use rustok_comments::entities::{comment, comment_thread};
use rustok_comments::migrations;
use rustok_comments::{CommentStatus, CommentThreadStatus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use std::{collections::HashSet, error::Error};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const COMMENTS_TEST_DATABASE_ENV: &str = "RUSTOK_COMMENTS_TEST_DATABASE_URL";

async fn setup_sqlite_database() -> DatabaseConnection {
    let database_url = format!(
        "sqlite:file:comments_thread_write_invariants_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let db = Database::connect(database_url)
        .await
        .expect("SQLite connection should succeed");
    apply_comments_migrations(&db)
        .await
        .expect("comments migrations should apply");
    db
}

async fn apply_comments_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    let manager = SchemaManager::new(db);
    for migration in migrations::migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

fn new_thread(tenant_id: Uuid, thread_id: Uuid, target_id: Uuid) -> comment_thread::ActiveModel {
    let now = Utc::now();
    comment_thread::ActiveModel {
        id: Set(thread_id),
        tenant_id: Set(tenant_id),
        target_type: Set("blog_post".to_string()),
        target_id: Set(target_id),
        status: Set(CommentThreadStatus::Open),
        comment_count: Set(0),
        last_commented_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
}

fn new_comment(
    tenant_id: Uuid,
    thread_id: Uuid,
    comment_id: Uuid,
    supplied_position: i64,
) -> comment::ActiveModel {
    let now = Utc::now();
    comment::ActiveModel {
        id: Set(comment_id),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread_id),
        author_id: Set(Uuid::new_v4()),
        parent_comment_id: Set(None),
        status: Set(CommentStatus::Approved),
        position: Set(supplied_position),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    }
}

async fn refresh_thread_count_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    thread_id: Uuid,
) -> Result<comment_thread::Model, DbErr> {
    let thread = comment_thread::Entity::find_by_id(thread_id)
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| DbErr::Custom(format!("comment thread {thread_id} is missing")))?;
    let mut stale_thread: comment_thread::ActiveModel = thread.into();
    stale_thread.comment_count = Set(i32::MAX);
    stale_thread.updated_at = Set(Utc::now().into());
    stale_thread.update(txn).await
}

async fn insert_comment_and_refresh_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    thread_id: Uuid,
    supplied_position: i64,
) -> Result<comment::Model, DbErr> {
    let txn = db.begin().await?;
    let inserted = new_comment(tenant_id, thread_id, Uuid::new_v4(), supplied_position)
        .insert(&txn)
        .await?;
    refresh_thread_count_in_tx(&txn, tenant_id, thread_id).await?;
    txn.commit().await?;
    Ok(inserted)
}

async fn soft_delete_comment_and_refresh_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    thread_id: Uuid,
    comment_id: Uuid,
) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    let existing = comment::Entity::find_by_id(comment_id)
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread_id))
        .one(&txn)
        .await?
        .ok_or_else(|| DbErr::Custom(format!("comment {comment_id} is missing")))?;
    let mut active: comment::ActiveModel = existing.into();
    active.deleted_at = Set(Some(Utc::now().into()));
    active.updated_at = Set(Utc::now().into());
    active.update(&txn).await?;
    refresh_thread_count_in_tx(&txn, tenant_id, thread_id).await?;
    txn.commit().await?;
    Ok(())
}

#[tokio::test]
async fn active_model_hooks_override_stale_positions_and_counts() {
    let db = setup_sqlite_database().await;
    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();

    new_thread(tenant_id, thread_id, Uuid::new_v4())
        .insert(&db)
        .await
        .expect("thread should insert");

    let first = new_comment(tenant_id, thread_id, Uuid::new_v4(), 99)
        .insert(&db)
        .await
        .expect("first comment should insert");
    assert_eq!(first.position, 1);

    let second = new_comment(tenant_id, thread_id, Uuid::new_v4(), -5)
        .insert(&db)
        .await
        .expect("second comment should insert");
    assert_eq!(second.position, 2);

    let mut deleted_first: comment::ActiveModel = first.into();
    deleted_first.deleted_at = Set(Some(Utc::now().into()));
    deleted_first.updated_at = Set(Utc::now().into());
    deleted_first
        .update(&db)
        .await
        .expect("first comment should soft-delete");

    let thread = comment_thread::Entity::find_by_id(thread_id)
        .one(&db)
        .await
        .expect("thread lookup should succeed")
        .expect("thread should exist");
    let mut stale_thread: comment_thread::ActiveModel = thread.into();
    stale_thread.comment_count = Set(999);
    stale_thread.updated_at = Set(Utc::now().into());
    let repaired = stale_thread
        .update(&db)
        .await
        .expect("thread update should recompute its count");

    assert_eq!(repaired.comment_count, 1);
}

#[tokio::test]
async fn status_only_thread_update_preserves_comment_count() {
    let db = setup_sqlite_database().await;
    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();

    new_thread(tenant_id, thread_id, Uuid::new_v4())
        .insert(&db)
        .await
        .expect("thread should insert");
    insert_comment_and_refresh_count(&db, tenant_id, thread_id, 999)
        .await
        .expect("comment should insert and refresh count");

    let thread = comment_thread::Entity::find_by_id(thread_id)
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("thread lookup should succeed")
        .expect("thread should exist");
    assert_eq!(thread.comment_count, 1);

    let mut status_update: comment_thread::ActiveModel = thread.into();
    status_update.status = Set(CommentThreadStatus::Closed);
    status_update.updated_at = Set(Utc::now().into());
    let updated = status_update
        .update(&db)
        .await
        .expect("status-only update should succeed");

    assert_eq!(updated.status, CommentThreadStatus::Closed);
    assert_eq!(updated.comment_count, 1);
}

#[tokio::test]
async fn unique_position_index_rejects_active_model_bypass() {
    let db = setup_sqlite_database().await;
    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();

    new_thread(tenant_id, thread_id, Uuid::new_v4())
        .insert(&db)
        .await
        .expect("thread should insert");

    comment::Entity::insert(new_comment(tenant_id, thread_id, Uuid::new_v4(), 1))
        .exec(&db)
        .await
        .expect("first direct insert should succeed");
    let duplicate = comment::Entity::insert(new_comment(tenant_id, thread_id, Uuid::new_v4(), 1))
        .exec(&db)
        .await;

    assert!(
        duplicate.is_err(),
        "unique thread position must reject bypass"
    );
}

struct PostgresCommentsTestDb {
    control: DatabaseConnection,
    db_a: DatabaseConnection,
    db_b: DatabaseConnection,
    schema_name: String,
}

impl PostgresCommentsTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{COMMENTS_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping comments thread concurrency test"
            );
            return Ok(None);
        };

        let control = connect_postgres(&database_url).await?;
        let schema_name = format!(
            "rustok_comments_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let setup_result = async {
            let db_a = connect_postgres(&database_url).await?;
            let db_b = connect_postgres(&database_url).await?;
            set_search_path(&db_a, &schema_name).await?;
            set_search_path(&db_b, &schema_name).await?;
            apply_comments_migrations(&db_a).await?;
            Ok::<_, Box<dyn Error + Send + Sync>>((db_a, db_b))
        }
        .await;

        match setup_result {
            Ok((db_a, db_b)) => Ok(Some(Self {
                control,
                db_a,
                db_b,
                schema_name,
            })),
            Err(error) => {
                let _ = control
                    .execute_unprepared(&format!(
                        r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#
                    ))
                    .await;
                Err(error)
            }
        }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_creates_and_delete_preserve_thread_invariants() -> TestResult<()> {
    let Some(test_db) = PostgresCommentsTestDb::setup("thread_concurrency").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let thread_id = Uuid::new_v4();
    new_thread(tenant_id, thread_id, Uuid::new_v4())
        .insert(&test_db.db_a)
        .await?;

    let (first, second) = tokio::join!(
        insert_comment_and_refresh_count(&test_db.db_a, tenant_id, thread_id, 99),
        insert_comment_and_refresh_count(&test_db.db_b, tenant_id, thread_id, -5),
    );
    let first = first?;
    let second = second?;
    let first_positions: HashSet<i64> = [first.position, second.position].into_iter().collect();
    assert_eq!(first_positions, HashSet::from([1, 2]));

    let (third, deleted) = tokio::join!(
        insert_comment_and_refresh_count(&test_db.db_a, tenant_id, thread_id, 777),
        soft_delete_comment_and_refresh_count(&test_db.db_b, tenant_id, thread_id, first.id,),
    );
    let third = third?;
    deleted?;
    assert_eq!(third.position, 3);

    let comments = comment::Entity::find()
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread_id))
        .order_by_asc(comment::Column::Position)
        .all(&test_db.db_a)
        .await?;
    let positions: Vec<i64> = comments.iter().map(|comment| comment.position).collect();
    let unique_positions: HashSet<i64> = positions.iter().copied().collect();
    assert_eq!(positions, vec![1, 2, 3]);
    assert_eq!(unique_positions.len(), positions.len());

    let active_count = comment::Entity::find()
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread_id))
        .filter(comment::Column::DeletedAt.is_null())
        .count(&test_db.db_a)
        .await?;
    let thread = comment_thread::Entity::find_by_id(thread_id)
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .one(&test_db.db_a)
        .await?
        .expect("thread should exist after concurrent writes");
    assert_eq!(active_count, 2);
    assert_eq!(thread.comment_count, active_count as i32);

    test_db.cleanup().await
}

fn postgres_database_url() -> Option<String> {
    std::env::var(COMMENTS_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect_postgres(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
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

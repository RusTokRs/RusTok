use rustok_comments::entities::comment_thread;
use rustok_comments::migrations;
use rustok_comments::{CommentStatus, CommentsService, CreateCommentInput};
use rustok_core::{SecurityContext, UserRole};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use std::{collections::HashSet, error::Error};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const COMMENTS_TEST_DATABASE_ENV: &str = "RUSTOK_COMMENTS_TEST_DATABASE_URL";

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
                "{COMMENTS_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping first-thread concurrency test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_comments_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let setup_result = async {
            let db_a = connect(&database_url).await?;
            let db_b = connect(&database_url).await?;
            set_search_path(&db_a, &schema_name).await?;
            set_search_path(&db_b, &schema_name).await?;
            let manager = SchemaManager::new(&db_a);
            for migration in migrations::migrations() {
                migration.up(&manager).await?;
            }
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
async fn postgres_concurrent_first_comments_share_one_thread() -> TestResult<()> {
    let Some(test_db) = PostgresCommentsTestDb::setup("first_thread").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let service_a = CommentsService::new(test_db.db_a.clone());
    let service_b = CommentsService::new(test_db.db_b.clone());
    let security_a = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let security_b = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let (first, second) = tokio::join!(
        service_a.create_comment(
            tenant_id,
            security_a,
            comment_input(target_id, "first concurrent comment"),
        ),
        service_b.create_comment(
            tenant_id,
            security_b,
            comment_input(target_id, "second concurrent comment"),
        ),
    );
    let first = first?;
    let second = second?;

    assert_eq!(first.thread_id, second.thread_id);
    let positions: HashSet<i64> = [first.position, second.position].into_iter().collect();
    assert_eq!(positions, HashSet::from([1, 2]));

    let threads = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq("blog_post"))
        .filter(comment_thread::Column::TargetId.eq(target_id))
        .all(&test_db.db_a)
        .await?;
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, first.thread_id);
    assert_eq!(threads[0].comment_count, 2);

    test_db.cleanup().await
}

use rustok_api::RichTextDocument;

fn richtext(text: &str) -> RichTextDocument {
    serde_json::from_value(serde_json::json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    }))
    .expect("test richtext")
}

fn comment_input(target_id: Uuid, body: &str) -> CreateCommentInput {
    CreateCommentInput {
        target_type: "blog_post".to_string(),
        target_id,
        locale: "en".to_string(),
        body: richtext(body),
        parent_comment_id: None,
        status: CommentStatus::Pending,
    }
}

fn postgres_database_url() -> Option<String> {
    std::env::var(COMMENTS_TEST_DATABASE_ENV)
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

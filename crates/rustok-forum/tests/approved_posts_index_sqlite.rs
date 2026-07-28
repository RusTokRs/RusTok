use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const TOPIC_INDEX: &str = "idx_forum_topics_tenant_author_retained";
const REPLY_INDEX: &str = "idx_forum_replies_tenant_author_approved_retained";

async fn setup() -> sea_orm::DatabaseConnection {
    let database_url = format!(
        "sqlite:file:forum_approved_posts_index_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("approved-post index SQLite database should connect");
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    db
}

#[tokio::test]
async fn approved_posts_aggregate_uses_partial_author_indexes_on_sqlite() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            EXPLAIN QUERY PLAN
            SELECT
                (
                    SELECT COUNT(*)
                    FROM forum_topics topic
                    WHERE topic.tenant_id = ?1
                      AND topic.author_id = ?2
                      AND topic.deleted_at IS NULL
                ) AS approved_topics,
                (
                    SELECT COUNT(*)
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE reply.tenant_id = ?1
                      AND reply.author_id = ?2
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                      AND topic.deleted_at IS NULL
                ) AS approved_replies
            "#,
            vec![tenant_id.into(), user_id.into()],
        ))
        .await
        .expect("approved-post EXPLAIN QUERY PLAN should succeed");

    let details = rows
        .into_iter()
        .map(|row| {
            row.try_get::<String>("", "detail")
                .expect("SQLite query plan row should expose detail")
        })
        .collect::<Vec<_>>();
    let plan = details.join("\n");

    assert!(
        plan.contains(TOPIC_INDEX),
        "approved-topic subquery must use {TOPIC_INDEX}; observed plan:\n{plan}"
    );
    assert!(
        plan.contains(REPLY_INDEX),
        "approved-reply subquery must use {REPLY_INDEX}; observed plan:\n{plan}"
    );

    for (index_name, required_fragments) in [
        (
            TOPIC_INDEX,
            ["tenant_id, author_id", "author_id IS NOT NULL", "deleted_at IS NULL"].as_slice(),
        ),
        (
            REPLY_INDEX,
            [
                "tenant_id, author_id, topic_id",
                "author_id IS NOT NULL",
                "status = 'approved'",
                "deleted_at IS NULL",
            ]
            .as_slice(),
        ),
    ] {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = ?1",
                vec![index_name.into()],
            ))
            .await
            .expect("SQLite index definition lookup should succeed")
            .unwrap_or_else(|| panic!("SQLite migration should create {index_name}"));
        let definition = row
            .try_get::<String>("", "sql")
            .expect("SQLite index definition should be text");
        for fragment in required_fragments {
            assert!(
                definition.contains(fragment),
                "{index_name} is missing `{fragment}`: {definition}"
            );
        }
    }
}

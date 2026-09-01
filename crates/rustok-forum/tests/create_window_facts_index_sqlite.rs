use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const TOPIC_INDEX: &str = "idx_forum_topics_tenant_author_created_at";
const REPLY_INDEX: &str = "idx_forum_replies_tenant_author_created_at";

async fn setup() -> sea_orm::DatabaseConnection {
    let database_url = format!(
        "sqlite:file:forum_create_window_index_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("create-window index SQLite database should connect");

    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("SQLite platform user fixture should be created");

    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
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
async fn create_window_queries_use_author_time_indexes_on_sqlite() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    for (table, index_name) in [
        ("forum_topics", TOPIC_INDEX),
        ("forum_replies", REPLY_INDEX),
    ] {
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!(
                    "EXPLAIN QUERY PLAN \
                     SELECT COUNT(*) AS create_count \
                     FROM {table} \
                     WHERE tenant_id = ?1 \
                       AND author_id = ?2 \
                       AND created_at >= ?3 \
                       AND created_at <= ?4"
                ),
                vec![
                    tenant_id.into(),
                    user_id.into(),
                    "2026-07-28T11:00:00Z".into(),
                    "2026-07-28T12:00:00Z".into(),
                ],
            ))
            .await
            .expect("create-window EXPLAIN QUERY PLAN should succeed");
        let plan = rows
            .into_iter()
            .map(|row| {
                row.try_get::<String>("", "detail")
                    .expect("SQLite query plan row should expose detail")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            plan.contains(index_name),
            "{table} create-window query must use {index_name}; observed plan:\n{plan}"
        );

        assert_index_definition(
            &db,
            index_name,
            &[
                "tenant_id, author_id, created_at DESC",
                "author_id IS NOT NULL",
            ],
        )
        .await;
    }
}

async fn assert_index_definition(
    db: &sea_orm::DatabaseConnection,
    index_name: &str,
    required_fragments: &[&str],
) {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
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

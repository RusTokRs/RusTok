use rustok_core::MigrationSource;
use rustok_forum::{ForumModule, ForumTopicVisibilityScope, ForumTopicVisibilityService};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:forum_page_builder_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("Forum Page Builder visibility evidence SQLite should connect");

    db.execute_unprepared(
        r#"
CREATE TABLE taxonomy_terms (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_value TEXT NOT NULL DEFAULT '',
    canonical_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)
"#,
    )
    .await
    .expect("Forum Page Builder visibility taxonomy prerequisite should create");

    let manager = SchemaManager::new(&db);
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Forum migration should apply to visibility evidence SQLite");
    }
    db
}

#[tokio::test]
async fn page_builder_owner_visibility_preserves_category_floor_tenant_and_topic_state() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let public_category = Uuid::new_v4();
    let private_category = Uuid::new_v4();
    let private_child_category = Uuid::new_v4();
    let foreign_private_category = Uuid::new_v4();

    db.execute_unprepared(&format!(
        r#"
INSERT INTO forum_categories
    (id, tenant_id, parent_id, position, moderated, topic_count, reply_count)
VALUES
    ('{public_category}', '{tenant_id}', NULL, 0, 0, 0, 0),
    ('{private_category}', '{tenant_id}', NULL, 1, 0, 0, 0),
    ('{private_child_category}', '{tenant_id}', '{private_category}', 2, 0, 0, 0),
    ('{foreign_private_category}', '{foreign_tenant_id}', NULL, 0, 0, 0, 0);
INSERT INTO forum_category_policies
    (category_id, tenant_id, allows_topics, visibility_override)
VALUES
    ('{private_category}', '{tenant_id}', 1, 'authenticated'),
    ('{foreign_private_category}', '{foreign_tenant_id}', 1, 'authenticated');
"#,
    ))
    .await
    .expect("Forum visibility categories should seed");

    let public_topic = Uuid::new_v4();
    let private_topic = Uuid::new_v4();
    let private_child_topic = Uuid::new_v4();
    let closed_public_topic = Uuid::new_v4();
    let foreign_topic = Uuid::new_v4();
    db.execute_unprepared(&format!(
        r#"
INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{public_topic}', '{tenant_id}', '{public_category}', 'open', '{{}}', 0, 0, 0),
    ('{private_topic}', '{tenant_id}', '{private_category}', 'open', '{{}}', 0, 0, 0),
    ('{private_child_topic}', '{tenant_id}', '{private_child_category}', 'open', '{{}}', 0, 0, 0),
    ('{closed_public_topic}', '{tenant_id}', '{public_category}', 'closed', '{{}}', 0, 0, 0),
    ('{foreign_topic}', '{foreign_tenant_id}', '{foreign_private_category}', 'open', '{{}}', 0, 0, 0);
"#,
    ))
    .await
    .expect("Forum visibility topics should seed");

    let candidates = vec![
        private_topic,
        public_topic,
        private_child_topic,
        closed_public_topic,
        foreign_topic,
    ];
    let service = ForumTopicVisibilityService::new(db);

    let anonymous = service
        .filter_visible_topic_ids(
            tenant_id,
            &candidates,
            &ForumTopicVisibilityScope::storefront(None)
                .expect("anonymous Forum visibility scope should construct"),
        )
        .await
        .expect("anonymous Forum visibility should resolve");
    assert_eq!(anonymous, vec![public_topic]);

    let authenticated = service
        .filter_visible_topic_ids(
            tenant_id,
            &candidates,
            &ForumTopicVisibilityScope::storefront_for_viewer(None, true)
                .expect("authenticated Forum visibility scope should construct"),
        )
        .await
        .expect("authenticated Forum visibility should resolve");
    assert_eq!(
        authenticated,
        vec![private_topic, public_topic, private_child_topic],
        "authenticated viewers may see authenticated category descendants, but not closed or foreign-tenant topics"
    );
}

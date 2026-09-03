use rustok_core::MigrationSource;
use rustok_forum::{ForumModule, ForumTopicVisibilityScope, ForumTopicVisibilityService};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
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
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL
)
"#,
    )
    .await
    .expect("users table should create");

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Taxonomy migration should apply");
    }
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

    use sea_orm::Statement;
    let backend = db.get_database_backend();

    for (cat_id, t_id, parent, pos) in [
        (public_category, tenant_id, None, 0),
        (private_category, tenant_id, None, 1),
        (private_child_category, tenant_id, Some(private_category), 2),
        (foreign_private_category, foreign_tenant_id, None, 0),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            "INSERT INTO forum_categories (id, tenant_id, parent_id, position, moderated, topic_count, reply_count) VALUES (?, ?, ?, ?, 0, 0, 0)",
            [cat_id.into(), t_id.into(), parent.into(), pos.into()],
        ))
        .await
        .expect("Forum category should insert");
    }

    for (cat_id, t_id) in [
        (private_category, tenant_id),
        (foreign_private_category, foreign_tenant_id),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            "INSERT INTO forum_category_policies (category_id, tenant_id, allows_topics, visibility_override, updated_at) VALUES (?, ?, 1, 'authenticated', CURRENT_TIMESTAMP)",
            [cat_id.into(), t_id.into()],
        ))
        .await
        .expect("Forum category policy should insert");
    }

    let public_topic = Uuid::new_v4();
    let private_topic = Uuid::new_v4();
    let private_child_topic = Uuid::new_v4();
    let closed_public_topic = Uuid::new_v4();
    let foreign_topic = Uuid::new_v4();

    for (top_id, t_id, cat_id, st) in [
        (public_topic, tenant_id, public_category, "open"),
        (private_topic, tenant_id, private_category, "open"),
        (
            private_child_topic,
            tenant_id,
            private_child_category,
            "open",
        ),
        (closed_public_topic, tenant_id, public_category, "closed"),
        (
            foreign_topic,
            foreign_tenant_id,
            foreign_private_category,
            "open",
        ),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            "INSERT INTO forum_topics (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count) VALUES (?, ?, ?, ?, '{}', 0, 0, 0)",
            [top_id.into(), t_id.into(), cat_id.into(), st.into()],
        ))
        .await
        .expect("Forum topic should insert");
    }

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

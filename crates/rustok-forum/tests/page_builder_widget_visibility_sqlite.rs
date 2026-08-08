use rustok_forum::{ForumTopicVisibilityScope, ForumTopicVisibilityService};
use sea_orm::{ConnectionTrait, Database};
use uuid::Uuid;

async fn setup() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Forum Page Builder visibility evidence SQLite should connect");
    db.execute_unprepared(
        r#"
PRAGMA foreign_keys = OFF;
CREATE TABLE forum_categories (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    parent_id TEXT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    icon TEXT NULL,
    color TEXT NULL,
    moderated INTEGER NOT NULL DEFAULT 0,
    topic_count INTEGER NOT NULL DEFAULT 0,
    reply_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE forum_category_policies (
    category_id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    allows_topics INTEGER NOT NULL DEFAULT 1,
    visibility_override TEXT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE forum_topics (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    author_id TEXT NULL,
    status TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    is_pinned INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0,
    reply_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_reply_at TEXT NULL
);
CREATE TABLE forum_topic_channel_access (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    channel_slug TEXT NOT NULL,
    PRIMARY KEY (tenant_id, topic_id, channel_slug)
);
"#,
    )
    .await
    .expect("Forum Page Builder visibility evidence tables should create");
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
INSERT INTO forum_categories (id, tenant_id, parent_id) VALUES
    ('{public_category}', '{tenant_id}', NULL),
    ('{private_category}', '{tenant_id}', NULL),
    ('{private_child_category}', '{tenant_id}', '{private_category}'),
    ('{foreign_private_category}', '{foreign_tenant_id}', NULL);
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
INSERT INTO forum_topics (id, tenant_id, category_id, status) VALUES
    ('{public_topic}', '{tenant_id}', '{public_category}', 'open'),
    ('{private_topic}', '{tenant_id}', '{private_category}', 'open'),
    ('{private_child_topic}', '{tenant_id}', '{private_child_category}', 'open'),
    ('{closed_public_topic}', '{tenant_id}', '{public_category}', 'closed'),
    ('{foreign_topic}', '{foreign_tenant_id}', '{foreign_private_category}', 'open');
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

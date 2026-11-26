//! Entity-generated Forum-only schema shared by relation and admission regressions.
use super::forum_under_test::{TopicStatus, entities::*};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, Schema, Statement,
};
use uuid::Uuid;

pub async fn fixture() -> (DatabaseConnection, Uuid, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    let enabled: i32 = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index(0)
        .unwrap();
    assert_eq!(
        enabled, 1,
        "these regressions must never disable FK enforcement"
    );
    let schema = Schema::new(DatabaseBackend::Sqlite);
    macro_rules! table {
        ($entity:ident) => {
            db.execute_raw(
                DatabaseBackend::Sqlite.build(&schema.create_table_from_entity($entity::Entity)),
            )
            .await
            .unwrap();
        };
    }
    table!(forum_category);
    table!(forum_topic);
    table!(forum_category_audience_policy);
    table!(forum_category_audience_role);
    table!(forum_category_audience_channel);
    table!(forum_category_audience_group);
    table!(forum_category_audience_user);
    table!(forum_category_topic_create_audience_policy);
    table!(forum_category_topic_create_audience_role);
    table!(forum_category_topic_create_audience_channel);
    table!(forum_category_topic_create_audience_group);
    table!(forum_category_topic_create_audience_user);
    table!(forum_category_reply_create_audience_policy);
    table!(forum_category_reply_create_audience_role);
    table!(forum_category_reply_create_audience_channel);
    table!(forum_category_reply_create_audience_group);
    table!(forum_category_reply_create_audience_user);
    table!(forum_category_moderation_audience_policy);
    table!(forum_category_moderation_audience_role);
    table!(forum_category_moderation_audience_channel);
    table!(forum_category_moderation_audience_group);
    table!(forum_category_moderation_audience_user);
    table!(forum_topic_audience_policy);
    table!(forum_topic_audience_role);
    table!(forum_topic_audience_channel);
    table!(forum_topic_audience_group);
    table!(forum_topic_audience_user);
    table!(forum_topic_reply_create_audience_policy);
    table!(forum_topic_reply_create_audience_role);
    table!(forum_topic_reply_create_audience_channel);
    table!(forum_topic_reply_create_audience_group);
    table!(forum_topic_reply_create_audience_user);
    let tenant = Uuid::new_v4();
    let category = Uuid::new_v4();
    let topic = Uuid::new_v4();
    forum_category::ActiveModel {
        id: Set(category),
        tenant_id: Set(tenant),
        moderated: Set(false),
        topic_count: Set(0),
        reply_count: Set(0),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    forum_topic::ActiveModel {
        id: Set(topic),
        tenant_id: Set(tenant),
        category_id: Set(category),
        author_id: Set(None),
        status: Set(TopicStatus::Open),
        metadata: Set(serde_json::json!({})),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        last_reply_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    (db, tenant, category, topic)
}

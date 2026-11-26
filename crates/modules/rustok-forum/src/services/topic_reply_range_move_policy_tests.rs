//! Transactional command policy comparison without Taxonomy storage.
use super::*;
use crate as forum_under_test;
use crate::entities::*;
use sea_orm::{Database, Schema};

#[path = "../../tests/support/taxonomy_reads.rs"]
mod taxonomy_reads;
use taxonomy_reads::TestTaxonomy;

async fn fixture(
    port: &TestTaxonomy,
) -> (DatabaseConnection, forum_topic::Model, forum_topic::Model) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(DatabaseBackend::Sqlite);
    macro_rules! table {
        ($module:ident) => {
            db.execute_raw(
                DatabaseBackend::Sqlite.build(&schema.create_table_from_entity($module::Entity)),
            )
            .await
            .unwrap();
        };
    }
    table!(forum_category);
    table!(forum_category_policy);
    table!(forum_category_audience_policy);
    table!(forum_category_audience_role);
    table!(forum_category_audience_channel);
    table!(forum_category_audience_group);
    table!(forum_category_audience_user);
    table!(forum_category_reply_create_audience_policy);
    table!(forum_category_reply_create_audience_role);
    table!(forum_category_reply_create_audience_channel);
    table!(forum_category_reply_create_audience_group);
    table!(forum_category_reply_create_audience_user);
    table!(forum_topic);
    table!(forum_topic_channel_access);
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
    let other_root = Uuid::new_v4();
    for id in [port.parent, port.category, other_root] {
        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(port.tenant),
            moderated: Set(false),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let mut topics = Vec::new();
    for category_id in [port.category, other_root] {
        topics.push(
            forum_topic::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(port.tenant),
                category_id: Set(category_id),
                author_id: Set(None),
                status: Set(TopicStatus::Open),
                metadata: Set(json!({})),
                is_pinned: Set(false),
                is_locked: Set(false),
                reply_count: Set(0),
                last_reply_at: Set(None),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await
            .unwrap(),
        );
    }
    let target = topics.pop().unwrap();
    let source = topics.pop().unwrap();
    (db, source, target)
}

#[tokio::test]
async fn range_move_compares_uncommitted_inherited_base_visibility() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, source, target) = fixture(&port).await;
    let txn = db.begin().await.unwrap();
    validate_equal_access_in_tx(&port, &txn, port.tenant, &source, &target)
        .await
        .unwrap();
    forum_category_policy::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        allows_topics: Set(true),
        visibility_override: Set(Some(crate::ForumCategoryVisibility::Authenticated)),
        updated_at: Set(Utc::now().into()),
    }
    .insert(&txn)
    .await
    .unwrap();
    assert!(
        matches!(validate_equal_access_in_tx(&port, &txn, port.tenant, &source, &target).await, Err(ForumError::Validation(message)) if message == "Forum reply range move requires exactly equal effective base visibility")
    );
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn range_move_compares_inherited_richer_visibility_not_just_topic_layers() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, source, target) = fixture(&port).await;
    let txn = db.begin().await.unwrap();
    forum_category_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(Some(1)),
        updated_at: Set(Utc::now().into()),
    }
    .insert(&txn)
    .await
    .unwrap();
    assert!(
        matches!(validate_equal_access_in_tx(&port, &txn, port.tenant, &source, &target).await, Err(ForumError::Validation(message)) if message == "Forum reply range move requires exactly equal effective visibility policy")
    );
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn range_move_compares_inherited_reply_creation_and_propagates_provider_failure() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, source, target) = fixture(&port).await;
    let txn = db.begin().await.unwrap();
    forum_category_reply_create_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(Some(1)),
        updated_at: Set(Utc::now().into()),
    }
    .insert(&txn)
    .await
    .unwrap();
    assert!(
        matches!(validate_equal_access_in_tx(&port, &txn, port.tenant, &source, &target).await, Err(ForumError::Validation(message)) if message == "Forum reply range move requires exactly equal effective reply-create policy")
    );
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    assert!(matches!(
        validate_equal_access_in_tx(&port, &txn, port.tenant, &source, &target).await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    txn.rollback().await.unwrap();
}

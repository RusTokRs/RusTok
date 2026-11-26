//! Transaction-local inherited anonymous floor, independent of Taxonomy storage.
use super::*;
use crate as forum_under_test;
use crate::entities::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, Database, DatabaseBackend, Schema, TransactionTrait,
};
use std::sync::Arc;

#[path = "../../tests/support/taxonomy_reads.rs"]
mod taxonomy_reads;
use taxonomy_reads::TestTaxonomy;

async fn fixture(port: &TestTaxonomy) -> (DatabaseConnection, Uuid) {
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
    table!(forum_topic);
    table!(forum_topic_audience_policy);
    table!(forum_topic_audience_role);
    table!(forum_topic_audience_channel);
    table!(forum_topic_audience_group);
    table!(forum_topic_audience_user);
    for id in [port.parent, port.category] {
        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(port.tenant),
            moderated: Set(false),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let topic = Uuid::new_v4();
    forum_topic::ActiveModel {
        id: Set(topic),
        tenant_id: Set(port.tenant),
        category_id: Set(port.category),
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
    (db, topic)
}

#[tokio::test]
async fn anonymous_owner_floor_reads_uncommitted_ancestor_policy_and_rolls_back() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic) = fixture(&port).await;
    let visibility = ForumTopicAudienceVisibilityService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let viewer = ForumTopicAudienceViewer::public();
    let txn = db.begin().await.unwrap();
    assert!(
        visibility
            .is_topic_owner_visible_in_tx(&txn, port.tenant, topic, &viewer)
            .await
            .unwrap()
    );
    // The restriction exists only in this transaction. A pool fallback either
    // loses it or blocks on the fixture's single SQLite connection.
    forum_category_policy::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        allows_topics: Set(true),
        visibility_override: Set(Some(crate::ForumCategoryVisibility::Authenticated)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&txn)
    .await
    .unwrap();
    assert!(
        !visibility
            .is_topic_owner_visible_in_tx(&txn, port.tenant, topic, &viewer)
            .await
            .unwrap()
    );
    txn.rollback().await.unwrap();
    let txn = db.begin().await.unwrap();
    assert!(
        visibility
            .is_topic_owner_visible_in_tx(&txn, port.tenant, topic, &viewer)
            .await
            .unwrap()
    );
    txn.rollback().await.unwrap();
    assert!(!port.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn anonymous_owner_transaction_propagates_selected_provider_failure() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    port.fail_at = Some("parents");
    let (db, topic) = fixture(&port).await;
    let visibility = ForumTopicAudienceVisibilityService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let txn = db.begin().await.unwrap();
    assert!(matches!(
        visibility
            .is_topic_owner_visible_in_tx(
                &txn,
                port.tenant,
                topic,
                &ForumTopicAudienceViewer::public()
            )
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    txn.rollback().await.unwrap();
    assert_eq!(port.calls.lock().unwrap().as_slice(), &["parents"]);
}

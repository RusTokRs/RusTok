//! `require_in_tx` must enforce denial itself, not return an unchecked preview.
use crate as forum_under_test;
use crate::entities::*;
use crate::{
    ForumError, ForumReplyCreateAudienceAuthorizationService,
    ForumTopicCreateAudienceAuthorizationService,
};
use rustok_api::Permission;
use rustok_core::SecurityContext;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, Statement, TransactionTrait,
};
use std::sync::Arc;
use uuid::Uuid;

#[path = "../../tests/support/audience_schema.rs"]
mod audience_schema;
#[path = "../../tests/support/taxonomy_reads.rs"]
mod taxonomy_reads;
use taxonomy_reads::TestTaxonomy;

async fn fixture() -> (DatabaseConnection, TestTaxonomy, forum_topic::Model) {
    let (db, tenant, category, topic_id) = audience_schema::fixture().await;
    let mut port = TestTaxonomy::new();
    port.tenant = tenant;
    port.category = category;
    port.require_transaction = true;
    forum_category::ActiveModel {
        id: Set(port.parent),
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
    let topic = forum_topic::Entity::find_by_id(topic_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    (db, port, topic)
}

fn security(user: Uuid) -> SecurityContext {
    SecurityContext::from_permission_snapshot(
        Some(user),
        &[
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_REPLIES_CREATE,
        ],
    )
}

#[tokio::test]
async fn transactional_require_enforces_ancestor_denial_and_recovers_after_rollback() {
    let (db, port, topic) = fixture().await;
    let user = Uuid::new_v4();
    let topics = ForumTopicCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let replies = ForumReplyCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let txn = db.begin().await.unwrap();
    for prefix in [
        "forum_category_topic_create_audience",
        "forum_category_reply_create_audience",
    ] {
        txn.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
            format!("INSERT INTO {prefix}_policies (tenant_id, category_id, minimum_trust_level, updated_at) VALUES ($1, $2, NULL, CURRENT_TIMESTAMP)"),
            [port.tenant.into(), port.parent.into()])).await.unwrap();
        txn.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
            format!("INSERT INTO {prefix}_users (tenant_id, category_id, user_id, effect) VALUES ($1, $2, $3, 'deny')"),
            [port.tenant.into(), port.parent.into(), user.into()])).await.unwrap();
    }
    assert!(
        matches!(topics.require_in_tx(&txn, port.tenant, port.category, &security(user), None).await,
        Err(ForumError::Forbidden(message)) if message == "Forum topic creation is unavailable for the current audience")
    );
    assert!(
        matches!(replies.require_in_tx(&txn, port.tenant, &topic, &security(user), None).await,
        Err(ForumError::Forbidden(message)) if message == "Forum reply creation is unavailable for the current audience")
    );
    txn.rollback().await.unwrap();
    let txn = db.begin().await.unwrap();
    assert!(
        topics
            .require_in_tx(&txn, port.tenant, port.category, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
    assert!(
        replies
            .require_in_tx(&txn, port.tenant, &topic, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn transactional_require_enforces_topic_local_reply_denial_without_blocking_topic_creation() {
    let (db, port, topic) = fixture().await;
    let user = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    txn.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
        "INSERT INTO forum_topic_reply_create_audience_policies (tenant_id, topic_id, minimum_trust_level, updated_at) VALUES ($1, $2, NULL, CURRENT_TIMESTAMP)",
        [port.tenant.into(), topic.id.into()])).await.unwrap();
    txn.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
        "INSERT INTO forum_topic_reply_create_audience_users (tenant_id, topic_id, user_id, effect) VALUES ($1, $2, $3, 'deny')",
        [port.tenant.into(), topic.id.into(), user.into()])).await.unwrap();
    let replies = ForumReplyCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        matches!(replies.require_in_tx(&txn, port.tenant, &topic, &security(user), None).await,
        Err(ForumError::Forbidden(message)) if message == "Forum reply creation is unavailable for the current audience")
    );
    let topics = ForumTopicCreateAudienceAuthorizationService::new(db, None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        topics
            .require_in_tx(&txn, port.tenant, port.category, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
    txn.rollback().await.unwrap();
}

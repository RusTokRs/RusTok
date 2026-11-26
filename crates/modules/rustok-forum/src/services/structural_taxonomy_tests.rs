//! Forum-only transaction regressions: selected hierarchy, lifecycle and deletion disclosure.
//! No concrete Taxonomy tables or provider fallback are available to these tests.
use crate as forum_under_test;
use crate::ForumError;
use crate::entities::*;
use rustok_api::Permission;
use rustok_core::SecurityContext;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, Schema, Statement, TransactionTrait,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[path = "../../tests/support/audience_schema.rs"]
mod audience_schema;
#[path = "../../tests/support/taxonomy_reads.rs"]
mod taxonomy_reads;
use taxonomy_reads::TestTaxonomy;

async fn fixture() -> (DatabaseConnection, TestTaxonomy, forum_topic::Model) {
    let (db, tenant, category, topic) = audience_schema::fixture().await;
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
    table!(forum_category_lifecycle);
    table!(forum_category_policy);
    table!(forum_topic_channel_access);
    table!(forum_reply);
    table!(forum_solution);
    table!(forum_topic_move_operation);
    table!(forum_topic_merge_operation);
    table!(forum_domain_event);
    db.execute_unprepared("ALTER TABLE forum_topics ADD COLUMN deleted_at TEXT NULL")
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE forum_topic_move_locks (tenant_id BLOB PRIMARY KEY, touched_at TEXT NOT NULL);
        CREATE TABLE forum_topic_merge_locks (tenant_id BLOB PRIMARY KEY, touched_at TEXT NOT NULL);
        CREATE TABLE forum_topic_solution_locks (tenant_id BLOB, topic_id BLOB, touched_at TEXT NOT NULL, PRIMARY KEY (tenant_id, topic_id));
        CREATE TABLE forum_topic_route_tombstone_visibility (tenant_id BLOB, topic_id BLOB, publicly_disclosable BOOLEAN NOT NULL, route_channel_restricted BOOLEAN NOT NULL, route_channel_count BIGINT NOT NULL, route_channel_digest TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY (tenant_id, topic_id));
        CREATE TABLE forum_topic_route_tombstone_channels (tenant_id BLOB, topic_id BLOB, channel_slug TEXT NOT NULL, PRIMARY KEY (tenant_id, topic_id, channel_slug));
        CREATE TABLE observed_topic_writes (topic_id BLOB);
        CREATE TRIGGER observe_topic_writes AFTER UPDATE ON forum_topics BEGIN INSERT INTO observed_topic_writes VALUES (NEW.id); END;")
        .await.unwrap();
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
    let topic = forum_topic::Entity::find_by_id(topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    (db, port, topic)
}

async fn archive_parent(txn: &sea_orm::DatabaseTransaction, port: &TestTaxonomy) {
    forum_category_lifecycle::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        archived_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(txn)
    .await
    .unwrap();
}

#[tokio::test]
async fn lifecycle_uses_selected_ancestor_and_uncommitted_state_then_recovers_after_rollback() {
    let (db, port, _) = fixture().await;
    let txn = db.begin().await.unwrap();
    archive_parent(&txn, &port).await;
    assert!(matches!(
        super::category_lifecycle::ensure_category_tree_target_is_active_in_tx(
            &port,
            &txn,
            port.tenant,
            port.category
        )
        .await,
        Err(ForumError::Validation(_))
    ));
    assert!(matches!(
        super::category_lifecycle::ensure_category_restore_target_is_active_in_tx(
            &port,
            &txn,
            port.tenant,
            port.category
        )
        .await,
        Err(ForumError::Validation(_))
    ));
    txn.rollback().await.unwrap();
    let txn = db.begin().await.unwrap();
    super::category_lifecycle::ensure_category_tree_target_is_active_in_tx(
        &port,
        &txn,
        port.tenant,
        port.category,
    )
    .await
    .unwrap();
    txn.rollback().await.unwrap();
    assert_eq!(*port.calls.lock().unwrap(), vec!["parents"; 3]);
}

#[tokio::test]
async fn lifecycle_rejects_incomplete_projection_and_preserves_typed_provider_error() {
    let (db, mut port, _) = fixture().await;
    port.parent_projection = Some(HashMap::from([(port.category, None)]));
    let txn = db.begin().await.unwrap();
    assert!(matches!(
        super::category_lifecycle::ensure_category_tree_target_is_active_in_tx(
            &port,
            &txn,
            port.tenant,
            port.category
        )
        .await,
        Err(ForumError::Validation(_))
    ));
    txn.rollback().await.unwrap();
    port.parent_projection = None;
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    let txn = db.begin().await.unwrap();
    let error = super::category_lifecycle::ensure_category_restore_target_is_active_in_tx(
        &port,
        &txn,
        port.tenant,
        port.category,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        ForumError::CapabilityFailure {
            retryable: true,
            ..
        }
    ));
    txn.rollback().await.unwrap();
}

fn manager() -> SecurityContext {
    SecurityContext::from_permission_snapshot(
        Some(Uuid::new_v4()),
        &[Permission::FORUM_TOPICS_MANAGE],
    )
}

fn bus(db: &DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
}

async fn assert_no_command_writes(db: &DatabaseConnection) {
    assert_eq!(
        forum_topic_move_operation::Entity::find()
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        forum_topic_merge_operation::Entity::find()
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        forum_domain_event::Entity::find().count(db).await.unwrap(),
        0
    );
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS writes FROM observed_topic_writes",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "writes").unwrap(), 0);
}

#[tokio::test]
async fn move_rolls_back_placement_when_selected_post_write_policy_fails() {
    let (db, mut port, topic) = fixture().await;
    // Two lifecycle reads, target category policy, then post-write topic policy.
    port.fail_at = Some("parents");
    port.fail_on_call = 4;
    let result = super::topic_move::ForumTopicMoveService::new(db.clone(), bus(&db))
        .with_taxonomy_port(Arc::new(port.clone()))
        .move_topic(
            port.tenant,
            topic.id,
            manager(),
            super::topic_move::MoveForumTopicInput {
                operation_id: Uuid::new_v4(),
                target_category_id: port.parent,
                reason: "provider rollback".into(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(ForumError::CapabilityFailure {
            retryable: true,
            ..
        })
    ));
    assert_eq!(*port.calls.lock().unwrap(), vec!["parents"; 4]);
    assert_eq!(
        forum_topic::Entity::find_by_id(topic.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        topic
    );
    assert_no_command_writes(&db).await;
}

#[tokio::test]
async fn merge_rolls_back_target_when_selected_post_write_policy_fails() {
    let (db, mut port, source) = fixture().await;
    let reply = forum_reply::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(port.tenant),
        topic_id: Set(source.id),
        author_id: Set(None),
        parent_reply_id: Set(None),
        status: Set(crate::ReplyStatus::Approved),
        position: Set(1),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let mut source_active: forum_topic::ActiveModel = source.into();
    source_active.reply_count = Set(1);
    let source = source_active.update(&db).await.unwrap();
    let mut target: forum_topic::ActiveModel = source.clone().into();
    target.id = Set(Uuid::new_v4());
    target.category_id = Set(port.parent);
    target.reply_count = Set(0);
    let target = target.insert(&db).await.unwrap();
    db.execute_unprepared("DELETE FROM observed_topic_writes")
        .await
        .unwrap();
    // Cross-category merge is valid; it must consult the target policy, not demand equality.
    port.fail_at = Some("parents");
    port.fail_on_call = 3;
    let result = super::topic_merge::ForumTopicMergeService::new(db.clone(), bus(&db))
        .with_taxonomy_port(Arc::new(port.clone()))
        .merge_topic(
            port.tenant,
            target.id,
            manager(),
            super::topic_merge::MergeForumTopicInput {
                operation_id: Uuid::new_v4(),
                source_topic_id: source.id,
                reason: "provider rollback".into(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(ForumError::CapabilityFailure {
            retryable: true,
            ..
        })
    ));
    assert_eq!(*port.calls.lock().unwrap(), vec!["parents"; 3]);
    assert_eq!(
        forum_topic::Entity::find_by_id(source.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        source
    );
    assert_eq!(
        forum_topic::Entity::find_by_id(target.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        target
    );
    assert_eq!(
        forum_reply::Entity::find_by_id(reply.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        reply
    );
    assert_no_command_writes(&db).await;
}

use super::topic_owner::route_tombstone_visibility::ForumTopicRouteTombstoneVisibilityService as Tombstones;

#[tokio::test]
async fn tombstone_snapshot_reads_selected_base_and_richer_ancestor_in_writer_transaction() {
    for base_floor in [true, false] {
        let (db, port, topic) = fixture().await;
        let txn = db.begin().await.unwrap();
        if base_floor {
            forum_category_policy::ActiveModel {
                category_id: Set(port.parent),
                tenant_id: Set(port.tenant),
                allows_topics: Set(true),
                visibility_override: Set(Some(
                    crate::visibility::ForumCategoryVisibility::Authenticated,
                )),
                updated_at: Set(chrono::Utc::now().into()),
            }
            .insert(&txn)
            .await
            .unwrap();
        } else {
            forum_category_audience_policy::ActiveModel {
                tenant_id: Set(port.tenant),
                category_id: Set(port.parent),
                minimum_trust_level: Set(Some(1)),
                updated_at: Set(chrono::Utc::now().into()),
            }
            .insert(&txn)
            .await
            .unwrap();
        }
        Tombstones::record_locked_delete_snapshot_in_tx(&port, &txn, port.tenant, &topic)
            .await
            .unwrap();
        let row = txn
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT publicly_disclosable FROM forum_topic_route_tombstone_visibility",
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(!row.try_get::<bool>("", "publicly_disclosable").unwrap());
        assert_eq!(
            *port.calls.lock().unwrap(),
            vec!["parents"; if base_floor { 1 } else { 2 }]
        );
        txn.rollback().await.unwrap();
        assert!(
            !Tombstones::new(db.clone())
                .can_disclose_public_gone(port.tenant, topic.id, None)
                .await
                .unwrap()
        );
        // The rolled-back private policy/snapshot must not contaminate the next attempt.
        let txn = db.begin().await.unwrap();
        Tombstones::record_locked_delete_snapshot_in_tx(&port, &txn, port.tenant, &topic)
            .await
            .unwrap();
        txn.commit().await.unwrap();
        assert!(
            Tombstones::new(db)
                .can_disclose_public_gone(port.tenant, topic.id, None)
                .await
                .unwrap()
        );
    }
}

#[tokio::test]
async fn tombstone_provider_failure_never_publishes_a_disclosable_snapshot() {
    for fail_on_call in [1, 2] {
        let (db, mut port, topic) = fixture().await;
        port.fail_at = Some("parents");
        port.fail_on_call = fail_on_call;
        let txn = db.begin().await.unwrap();
        let result =
            Tombstones::record_locked_delete_snapshot_in_tx(&port, &txn, port.tenant, &topic).await;
        assert!(matches!(
            result,
            Err(ForumError::CapabilityFailure {
                retryable: true,
                ..
            })
        ));
        txn.rollback().await.unwrap();
        assert!(
            !Tombstones::new(db)
                .can_disclose_public_gone(port.tenant, topic.id, None)
                .await
                .unwrap()
        );
    }
}

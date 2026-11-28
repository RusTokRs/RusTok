use std::collections::BTreeSet;

use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, QueryTrait, TransactionTrait,
    sea_query::{LockBehavior, LockType, OnConflict},
};
use uuid::Uuid;

use rustok_core::{Error, Result};

use crate::entity as sys_event;

mod subscription {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_event_consumers")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub event_type: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub consumer_key: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod receipt {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_event_consumer_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub outbox_event_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub consumer_key: String,
        pub event_type: String,
        pub acknowledged_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Durable acknowledgement boundary for consumers that must observe an Outbox
/// event before the canonical row can be pruned.
#[derive(Clone, Debug)]
pub struct OutboxConsumer {
    db: DatabaseConnection,
}

impl OutboxConsumer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Registration is additive and idempotent. Disabling a worker does not
    /// silently release its retention obligation.
    pub async fn register_required_event_types(
        &self,
        consumer_key: &str,
        event_types: &[String],
    ) -> Result<()> {
        validate_consumer_key(consumer_key)?;

        let mut normalized = BTreeSet::new();
        for event_type in event_types {
            if event_type.contains('\0') || event_type.trim() != event_type || event_type.is_empty() || event_type.len() > 255 {
                return Err(Error::Validation(
                    "outbox consumer event type must contain 1 to 255 unpadded bytes".to_string(),
                ));
            }
            normalized.insert(event_type.clone());
        }

        if normalized.is_empty() {
            return Err(Error::Validation(
                "outbox consumer must register at least one event type".to_string(),
            ));
        }

        let txn = self.begin_registry_write().await?;
        for event_type in normalized {
            let lookup = || subscription::Entity::find_by_id((event_type.clone(), consumer_key.to_string()));
            if lookup().one(&txn).await?.is_some() {
                continue;
            }
            let created_at = crate::database_now(&txn).await?;
            let affected = subscription::Entity::insert(subscription::ActiveModel {
                event_type: Set(event_type.clone()),
                consumer_key: Set(consumer_key.to_string()),
                created_at: Set(created_at),
            })
            .on_conflict(
                OnConflict::columns([
                    subscription::Column::EventType,
                    subscription::Column::ConsumerKey,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(&txn)
            .await?;
            let saved = lookup().one(&txn).await?;
            if affected != 1 || !saved.is_some_and(|row| row.created_at == created_at) {
                return Err(Error::Validation("outbox consumer registration admission mismatch".into()));
            }
        }
        txn.commit().await?;
        Ok(())
    }

    // Serialize registry mutations, including inserts of absent keys.
    async fn begin_registry_write(&self) -> Result<DatabaseTransaction> {
        let txn = self.db.begin().await?;
        match txn.get_database_backend() {
            DatabaseBackend::Postgres => {
                txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                    .await?;
                txn.execute_unprepared(
                    "LOCK TABLE sys_event_consumers IN SHARE ROW EXCLUSIVE MODE",
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {
                txn.execute_unprepared(
                    "UPDATE sys_event_consumers SET consumer_key = consumer_key WHERE 1 = 0",
                )
                .await?;
            }
            _ => return Err(Error::Validation("unsupported outbox consumer database".to_string())),
        }
        Ok(txn)
    }

    /// Acknowledges a terminal owner receipt during startup backfill. A missing
    /// event row is already outside Outbox retention and needs no further hold,
    /// but the consumer must still have an active registration.
    pub async fn acknowledge_existing(
        &self,
        consumer_key: &str,
        outbox_event_id: Uuid,
    ) -> Result<()> {
        validate_consumer_key(consumer_key)?;
        if outbox_event_id.is_nil() {
            return Err(Error::Validation(
                "outbox consumer acknowledgement event id must not be nil".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        Self::acknowledge_on(&txn, consumer_key, outbox_event_id, None).await?;
        txn.commit().await?;
        Ok(())
    }

    /// Retires a consumer only after every existing event for its registered
    /// event types has a terminal acknowledgement. Stop the worker and quiesce
    /// producers of those event types before calling.
    pub async fn retire_if_drained(&self, consumer_key: &str) -> Result<()> {
        validate_consumer_key(consumer_key)?;

        let txn = self.begin_registry_write().await?;
        if txn.get_database_backend() == DatabaseBackend::Postgres {
            // Never wait for this later fence while holding the registry:
            // an event writer may need to acknowledge in its owner transaction.
            txn.execute_unprepared("LOCK TABLE sys_events IN SHARE MODE NOWAIT")
                .await?;
        }
        let subscriptions = subscription::Entity::find()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .all(&txn)
            .await?;

        let expected = subscriptions.len() as u64;
        for subscription in subscriptions {
            let acknowledgements = receipt::Entity::find()
                .select_only()
                .column(receipt::Column::OutboxEventId)
                .filter(receipt::Column::ConsumerKey.eq(consumer_key))
                .filter(receipt::Column::EventType.eq(subscription.event_type.as_str()))
                .into_query();
            let pending = sys_event::Entity::find()
                .filter(sys_event::Column::EventType.eq(subscription.event_type))
                .filter(sys_event::Column::Id.not_in_subquery(acknowledgements))
                .one(&txn)
                .await?;

            if let Some(event) = pending {
                txn.rollback().await?;
                return Err(Error::Validation(format!(
                    "cannot retire Outbox consumer {consumer_key}: event {} is not acknowledged",
                    event.id
                )));
            }
        }

        let removed = subscription::Entity::delete_many()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .exec(&txn)
            .await?;
        if removed.rows_affected != expected {
            return Err(Error::Validation("outbox consumer retirement affected unexpected rows".into()));
        }
        txn.commit().await?;
        Ok(())
    }

    /// Persists the acknowledgement in the caller's transaction so the owner's
    /// terminal receipt and its Outbox acknowledgement commit together.
    pub async fn acknowledge_in_transaction(
        txn: &DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> std::result::Result<(), sea_orm::DbErr> {
        Self::reserve_effect_in_transaction(txn, consumer_key, outbox_event_id, event_type)
            .await.map(|_| ())
    }

    /// Reserve an exact consumer effect in the caller's SQL transaction.
    /// True admits a new effect; false is an exact prior receipt. The caller
    /// MUST commit the new receipt and owner writes together, and roll back
    /// the whole transaction on any error. Never use for external I/O or
    /// commit the reservation before the effect. Missing events fail closed.
    pub async fn reserve_effect_in_transaction(
        txn: &DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> std::result::Result<bool, sea_orm::DbErr> {
        if consumer_key.contains('\0') || consumer_key.trim().is_empty()
            || consumer_key.trim() != consumer_key
            || consumer_key.len() > 128
            || outbox_event_id.is_nil()
            || event_type.contains('\0')
            || event_type.trim().is_empty()
            || event_type.trim() != event_type
            || event_type.len() > 255
        {
            return Err(sea_orm::DbErr::Custom("invalid outbox consumer effect identity".into()));
        }
        Self::acknowledge_on(txn, consumer_key, outbox_event_id, Some(event_type)).await
    }

    // Only trusted startup backfill of terminal receipts may accept a
    // previously pruned event.
    async fn acknowledge_on(
        txn: &DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        expected_event_type: Option<&str>,
    ) -> std::result::Result<bool, sea_orm::DbErr> {
        let backend = txn.get_database_backend();
        match backend {
            DatabaseBackend::Postgres => {
                // ACK can run late in an owner transaction. Fail fast on
                // registry contention rather than inverting owner locks.
                txn.execute_unprepared("LOCK TABLE sys_event_consumers IN SHARE MODE NOWAIT")
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                txn.execute_unprepared(
                    "UPDATE sys_event_consumers SET consumer_key = consumer_key WHERE 1 = 0",
                )
                .await?;
            }
            _ => return Err(sea_orm::DbErr::Custom("unsupported outbox consumer database".to_string())),
        }

        let query = sys_event::Entity::find_by_id(outbox_event_id);
        let event = if backend == DatabaseBackend::Postgres {
            // Serialize canonical ACKs for this event without waiting with owner locks.
            query.lock_with_behavior(LockType::Update, LockBehavior::Nowait).one(txn).await?
        } else {
            query.one(txn).await?
        };
        let Some(event) = event else {
            if expected_event_type.is_some() {
                return Err(sea_orm::DbErr::RecordNotFound(format!("sys_events row {outbox_event_id}")));
            }
            // The registry fence protects this read. Missing-event backfill
            // is not an exception to active consumer registration.
            if subscription::Entity::find()
                .filter(subscription::Column::ConsumerKey.eq(consumer_key))
                .one(txn)
                .await?
                .is_none()
            {
                return Err(sea_orm::DbErr::Custom(
                    "outbox consumer is not registered".to_string(),
                ));
            }
            return Ok(false);
        };
        let event_type = expected_event_type.unwrap_or(event.event_type.as_str());
        if event.event_type != event_type {
            return Err(sea_orm::DbErr::Custom(
                "outbox consumer acknowledgement event type mismatch".to_string(),
            ));
        }

        let query = subscription::Entity::find()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .filter(subscription::Column::EventType.eq(event_type));
        let registered = if backend == DatabaseBackend::Postgres {
            query.lock_with_behavior(LockType::Share, LockBehavior::Nowait).one(txn).await?
        } else {
            query.one(txn).await?
        };
        if registered.is_none() {
            return Err(sea_orm::DbErr::Custom(
                "outbox consumer is not registered for this event type".to_string(),
            ));
        }

        let lookup = || receipt::Entity::find()
            .filter(receipt::Column::OutboxEventId.eq(outbox_event_id))
            .filter(receipt::Column::ConsumerKey.eq(consumer_key));
        // Source row/writer ownership serializes this read and the insert.
        if let Some(existing) = lookup().one(txn).await? {
            if existing.event_type != event_type {
                return Err(sea_orm::DbErr::Custom(
                    "outbox consumer acknowledgement conflicts with its prior event type".into(),
                ));
            }
            return Ok(false);
        }
        let acknowledged_at = crate::database_now(txn).await?;
        let affected = receipt::Entity::insert(receipt::ActiveModel {
            outbox_event_id: Set(outbox_event_id),
            consumer_key: Set(consumer_key.to_string()),
            event_type: Set(event_type.to_string()),
            acknowledged_at: Set(acknowledged_at),
        })
        .on_conflict(
            OnConflict::columns([receipt::Column::OutboxEventId, receipt::Column::ConsumerKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

        if affected != 1 {
            return Err(sea_orm::DbErr::Custom(format!(
                "new outbox consumer acknowledgement affected {affected} rows"
            )));
        }
        let existing = lookup().one(txn).await?.ok_or_else(|| {
            sea_orm::DbErr::RecordNotFound(format!(
                "sys_event_consumer_receipts row {consumer_key}/{outbox_event_id}"
            ))
        })?;
        if existing.event_type != event_type || existing.acknowledged_at != acknowledged_at {
            return Err(sea_orm::DbErr::Custom(
                "new outbox consumer acknowledgement readback mismatch".into(),
            ));
        }
        Ok(true)
    }
}

fn validate_consumer_key(consumer_key: &str) -> Result<()> {
    if consumer_key.contains('\0') || consumer_key.trim().is_empty()
        || consumer_key.trim() != consumer_key
        || consumer_key.len() > 128
    {
        return Err(Error::Validation(
            "outbox consumer key must contain 1 to 128 unpadded bytes".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::OutboxConsumer;
    use crate::OutboxTransport;
    use rustok_core::events::EventTransport;
    use rustok_events::{DomainEvent, EventEnvelope};
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

    async fn test_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        crate::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("outbox schema");
        db
    }

    #[tokio::test]
    async fn consumer_registration_and_acknowledgement_are_idempotent() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "notifications.outbox-intake";
        let event_type = "tenant.updated".to_string();
        consumer
            .register_required_event_types(key, std::slice::from_ref(&event_type))
            .await
            .expect("register");
        consumer
            .register_required_event_types(key, std::slice::from_ref(&event_type))
            .await
            .expect("re-register");

        let tenant_id = Uuid::new_v4();
        let envelope =
            EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone())
            .publish(envelope.clone())
            .await
            .expect("persist event");

        for _ in 0..2 {
            let txn = db.begin().await.expect("begin ack");
            OutboxConsumer::acknowledge_in_transaction(
                &txn,
                key,
                envelope.id,
                envelope.event_type.as_str(),
            )
            .await
            .expect("ack event");
            txn.commit().await.expect("commit ack");
        }

        let count = db
            .query_one_raw(sea_orm::Statement::from_string(
                db.get_database_backend(),
                "SELECT COUNT(*) AS count FROM sys_event_consumer_receipts".to_string(),
            ))
            .await
            .expect("query")
            .expect("row")
            .try_get::<i64>("", "count")
            .expect("count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn effect_admission_rolls_back_with_owner_writes_and_replays_once() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        let key = "consumer.effect";
        consumer.register_required_event_types(key, &[envelope.event_type.clone()]).await.unwrap();
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.unwrap();
        db.execute_unprepared("CREATE TABLE test_effects (id INTEGER PRIMARY KEY)").await.unwrap();
        for commit in [false, true] {
            let txn = db.begin().await.unwrap();
            assert!(OutboxConsumer::reserve_effect_in_transaction(&txn, key, envelope.id, &envelope.event_type).await.unwrap());
            txn.execute_unprepared("INSERT INTO test_effects VALUES (1)").await.unwrap();
            assert!(!OutboxConsumer::reserve_effect_in_transaction(&txn, key, envelope.id, &envelope.event_type).await.unwrap());
            if commit { txn.commit().await.unwrap(); } else { txn.rollback().await.unwrap(); }
        }
        let txn = db.begin().await.unwrap();
        assert!(!OutboxConsumer::reserve_effect_in_transaction(&txn, key, envelope.id, &envelope.event_type).await.unwrap());
        txn.commit().await.unwrap();
        let count = db.query_one_raw(sea_orm::Statement::from_string(db.get_database_backend(), "SELECT COUNT(*) AS count FROM test_effects")).await.unwrap().unwrap().try_get::<i64>("", "count").unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn acknowledgement_requires_registered_event_type() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let envelope =
            EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone())
            .publish(envelope.clone())
            .await
            .expect("persist event");

        let txn = db.begin().await.expect("begin ack");
        assert!(
            OutboxConsumer::acknowledge_in_transaction(
                &txn,
                "unregistered.consumer",
                envelope.id,
                envelope.event_type.as_str(),
            )
            .await
            .is_err()
        );
        txn.rollback().await.expect("rollback");
    }

    #[tokio::test]
    async fn missing_event_backfill_requires_an_active_consumer() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "consumer.backfill";
        let event_id = Uuid::new_v4();
        assert!(consumer.acknowledge_existing(key, event_id).await.is_err());
        consumer
            .register_required_event_types(key, &["tenant.updated".to_string()])
            .await
            .expect("register");
        consumer.acknowledge_existing(key, event_id).await.expect("pruned event");
        let txn = db.begin().await.expect("begin");
        assert!(OutboxConsumer::acknowledge_in_transaction(
            &txn, key, event_id, "tenant.updated"
        ).await.is_err());
        txn.rollback().await.expect("rollback");
        consumer.retire_if_drained(key).await.expect("retire");
        assert!(consumer.acknowledge_existing(key, event_id).await.is_err());
    }

    #[tokio::test]
    async fn consumer_retirement_requires_a_complete_drain() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let event_type = "tenant.updated".to_string();
        consumer
            .register_required_event_types("consumer.retiring", std::slice::from_ref(&event_type))
            .await
            .expect("register consumer");

        let tenant_id = Uuid::new_v4();
        let envelope =
            EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone())
            .publish(envelope.clone())
            .await
            .expect("persist event");

        assert!(db.execute_unprepared(
            "DELETE FROM sys_event_consumers WHERE consumer_key = 'consumer.retiring'"
        ).await.is_err(), "raw SQL cannot retire a consumer with unacknowledged events");
        assert!(db.execute_unprepared(
            "UPDATE sys_event_consumers SET created_at = CURRENT_TIMESTAMP"
        ).await.is_err(), "consumer registrations are immutable");
        assert!(
            consumer
                .retire_if_drained("consumer.retiring")
                .await
                .is_err()
        );
        consumer
            .acknowledge_existing("consumer.retiring", envelope.id)
            .await
            .expect("acknowledge event");
        consumer
            .retire_if_drained("consumer.retiring")
            .await
            .expect("retire drained consumer");
    }

    #[tokio::test]
    async fn retired_consumer_cannot_acknowledge_and_re_registration_preserves_receipts() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "consumer.lifecycle";
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        let types = [envelope.event_type.clone()];
        consumer.register_required_event_types(key, &types).await.expect("register");
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("event");
        consumer.acknowledge_existing(key, envelope.id).await.expect("ack");
        consumer.retire_if_drained(key).await.expect("retire");
        assert!(consumer.acknowledge_existing(key, envelope.id).await.is_err());
        consumer.register_required_event_types(key, &types).await.expect("register again");
        consumer.retire_if_drained(key).await.expect("prior terminal receipt remains valid");
    }

    #[tokio::test]
    async fn receipt_mutation_cannot_invalidate_a_drained_consumer() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "consumer.lifecycle";
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        consumer.register_required_event_types(key, &[envelope.event_type.clone()])
            .await.expect("register");
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("event");
        consumer.acknowledge_existing(key, envelope.id).await.expect("ack");
        assert!(db.execute_unprepared(
            "UPDATE sys_event_consumer_receipts SET event_type = 'wrong.type'"
        ).await.is_err(), "receipt identity is immutable");
        assert!(db.execute_unprepared(
            "DELETE FROM sys_event_consumer_receipts"
        ).await.is_err(), "a receipt cannot be removed while its source event exists");
        consumer.retire_if_drained(key).await.expect("valid terminal receipt remains authoritative");
    }

    #[tokio::test]
    async fn raw_receipt_insert_requires_matching_event_and_active_subscription() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("event");

        let raw = |consumer: &str, event_type: &str| format!(
            "INSERT INTO sys_event_consumer_receipts (outbox_event_id, consumer_key, event_type, acknowledged_at) SELECT id, '{}', '{}', CURRENT_TIMESTAMP FROM sys_events",
            consumer, event_type,
        );
        assert!(db.execute_unprepared(&raw("unregistered.consumer", &envelope.event_type)).await.is_err());
        let consumer = OutboxConsumer::new(db.clone());
        consumer.register_required_event_types(
            "consumer.receipt-guard", &[envelope.event_type.clone()],
        ).await.expect("register");
        assert!(db.execute_unprepared(&raw("consumer.receipt-guard", "wrong.event")).await.is_err());

        consumer.acknowledge_existing("consumer.receipt-guard", envelope.id).await.expect("ack");
        let before = db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT acknowledged_at FROM sys_event_consumer_receipts",
        )).await.expect("query").expect("receipt")
            .try_get::<String>("", "acknowledged_at").expect("timestamp");
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.expect("recursive triggers off");
        let replace = format!(
            "INSERT OR REPLACE INTO sys_event_consumer_receipts (outbox_event_id, consumer_key, event_type, acknowledged_at) SELECT id, 'consumer.receipt-guard', '{}', '2000-01-01 00:00:00' FROM sys_events",
            envelope.event_type,
        );
        db.execute_unprepared(&replace).await.expect("exact duplicate is ignored");
        let after = db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT acknowledged_at FROM sys_event_consumer_receipts",
        )).await.expect("query").expect("receipt")
            .try_get::<String>("", "acknowledged_at").expect("timestamp");
        assert_eq!(after, before, "SQLite REPLACE cannot rewrite a terminal ACK");
    }

    #[tokio::test]
    async fn registration_replace_cannot_rewrite_the_original_registry_row() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        consumer.register_required_event_types(
            "consumer.registry-guard", &["tenant.updated".to_string()],
        ).await.expect("register");
        let before = db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT created_at FROM sys_event_consumers WHERE consumer_key = 'consumer.registry-guard'",
        )).await.expect("query").expect("registration")
            .try_get::<String>("", "created_at").expect("timestamp");
        db.execute_unprepared(
            "INSERT OR REPLACE INTO sys_event_consumers (event_type, consumer_key, created_at) VALUES ('tenant.updated', 'consumer.registry-guard', '2000-01-01 00:00:00')"
        ).await.expect("exact duplicate is ignored");
        let after = db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT created_at FROM sys_event_consumers WHERE consumer_key = 'consumer.registry-guard'",
        )).await.expect("query").expect("registration")
            .try_get::<String>("", "created_at").expect("timestamp");
        assert_eq!(after, before, "SQLite REPLACE cannot rewrite registration history");
    }

    #[tokio::test]
    async fn suppressed_registration_and_ack_do_not_admit_effects() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let event = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        let types = [event.event_type.clone()];
        db.execute_unprepared("CREATE TRIGGER suppress_registration BEFORE INSERT ON sys_event_consumers BEGIN SELECT RAISE(IGNORE); END").await.unwrap();
        assert!(consumer.register_required_event_types("consumer.suppressed", &types).await.is_err());
        db.execute_unprepared("DROP TRIGGER suppress_registration").await.unwrap();
        consumer.register_required_event_types("consumer.suppressed", &types).await.unwrap();
        OutboxTransport::new(db.clone()).publish(event.clone()).await.unwrap();
        db.execute_unprepared("CREATE TRIGGER suppress_ack BEFORE INSERT ON sys_event_consumer_receipts BEGIN SELECT RAISE(IGNORE); END").await.unwrap();
        let txn = db.begin().await.unwrap();
        assert!(OutboxConsumer::reserve_effect_in_transaction(&txn, "consumer.suppressed", event.id, &event.event_type).await.is_err());
        txn.rollback().await.unwrap();
        db.execute_unprepared("DROP TRIGGER suppress_ack").await.unwrap();
        let txn = db.begin().await.unwrap();
        assert!(OutboxConsumer::reserve_effect_in_transaction(&txn, "consumer.suppressed", event.id, &event.event_type).await.unwrap());
        txn.commit().await.unwrap();
    }

    #[tokio::test]
    async fn registration_rejects_empty_or_invalid_contracts() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db);
        assert!(
            consumer
                .register_required_event_types("", &["tenant.updated".to_string()])
                .await
                .is_err()
        );
        assert!(
            consumer
                .register_required_event_types("valid.consumer", &[])
                .await
                .is_err()
        );
        assert!(
            consumer
                .register_required_event_types("valid.consumer", &[" tenant.updated".to_string()],)
                .await
                .is_err()
        );
    }
}

use std::collections::BTreeSet;

use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, QueryTrait, TransactionTrait,
    sea_query::{LockType, OnConflict},
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
            if event_type.trim() != event_type || event_type.is_empty() || event_type.len() > 255 {
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
            subscription::Entity::insert(subscription::ActiveModel {
                event_type: Set(event_type),
                consumer_key: Set(consumer_key.to_string()),
                created_at: Set(crate::database_now(&txn).await?),
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
    /// event row is already outside Outbox retention and needs no further hold.
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

        subscription::Entity::delete_many()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .exec(&txn)
            .await?;
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
        if consumer_key.trim().is_empty()
            || consumer_key.trim() != consumer_key
            || consumer_key.len() > 128
            || outbox_event_id.is_nil()
            || event_type.trim().is_empty()
            || event_type.trim() != event_type
            || event_type.len() > 255
        {
            return Err(sea_orm::DbErr::Custom(
                "invalid outbox consumer acknowledgement identity".to_string(),
            ));
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
    ) -> std::result::Result<(), sea_orm::DbErr> {
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
            query.lock(LockType::Share).one(txn).await?
        } else {
            query.one(txn).await?
        };
        let Some(event) = event else {
            return if expected_event_type.is_none() {
                Ok(())
            } else {
                Err(sea_orm::DbErr::RecordNotFound(format!("sys_events row {outbox_event_id}")))
            };
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
            query.lock(LockType::Share).one(txn).await?
        } else {
            query.one(txn).await?
        };
        if registered.is_none() {
            return Err(sea_orm::DbErr::Custom(
                "outbox consumer is not registered for this event type".to_string(),
            ));
        }

        let affected = receipt::Entity::insert(receipt::ActiveModel {
            outbox_event_id: Set(outbox_event_id),
            consumer_key: Set(consumer_key.to_string()),
            event_type: Set(event_type.to_string()),
            acknowledged_at: Set(crate::database_now(txn).await?),
        })
        .on_conflict(
            OnConflict::columns([receipt::Column::OutboxEventId, receipt::Column::ConsumerKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

        if affected == 1 {
            return Ok(());
        }
        if affected != 0 {
            return Err(sea_orm::DbErr::Custom(format!(
                "outbox consumer acknowledgement affected {affected} rows"
            )));
        }

        let existing = receipt::Entity::find()
            .filter(receipt::Column::OutboxEventId.eq(outbox_event_id))
            .filter(receipt::Column::ConsumerKey.eq(consumer_key))
            .one(txn)
            .await?
            .ok_or_else(|| {
                sea_orm::DbErr::RecordNotFound(format!(
                    "sys_event_consumer_receipts row {consumer_key}/{outbox_event_id}"
                ))
            })?;
        if existing.event_type != event_type {
            return Err(sea_orm::DbErr::Custom(
                "outbox consumer acknowledgement conflicts with its prior event type".to_string(),
            ));
        }

        Ok(())
    }
}

fn validate_consumer_key(consumer_key: &str) -> Result<()> {
    if consumer_key.trim().is_empty()
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
    async fn retirement_rejects_a_receipt_for_the_wrong_event_type() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "consumer.lifecycle";
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        consumer.register_required_event_types(key, &[envelope.event_type.clone()])
            .await.expect("register");
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("event");
        consumer.acknowledge_existing(key, envelope.id).await.expect("ack");
        db.execute_unprepared("UPDATE sys_event_consumer_receipts SET event_type = 'wrong.type'")
            .await.expect("simulate corrupt receipt");
        assert!(consumer.retire_if_drained(key).await.is_err());
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

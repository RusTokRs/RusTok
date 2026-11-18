use std::collections::BTreeSet;

use chrono::Utc;
use sea_orm::{ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, TransactionTrait, sea_query::OnConflict};
use uuid::Uuid;

use rustok_core::{Error, Result};
use crate::entity as sys_event;

mod subscription {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_event_consumers")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)] pub event_type: String,
        #[sea_orm(primary_key, auto_increment = false)] pub consumer_key: String,
        pub created_at: DateTime<Utc>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)] pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

mod receipt {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_event_consumer_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)] pub outbox_event_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)] pub consumer_key: String,
        pub event_type: String,
        pub acknowledged_at: DateTime<Utc>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)] pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

/// Durable acknowledgement boundary for consumers that must observe an Outbox
/// event before the canonical row can be pruned.
#[derive(Clone, Debug)]
pub struct OutboxConsumer { db: DatabaseConnection }

impl OutboxConsumer {
    pub fn new(db: DatabaseConnection) -> Self { Self { db } }

    /// Registration is additive and idempotent. Disabling a worker does not
    /// silently release its retention obligation.
    pub async fn register_required_event_types(&self, consumer_key: &str, event_types: &[String]) -> Result<()> {
        validate_consumer_key(consumer_key)?;
        let mut normalized = BTreeSet::new();
        for event_type in event_types {
            if event_type.trim() != event_type || event_type.is_empty() || event_type.len() > 255 {
                return Err(Error::Validation("outbox consumer event type must contain 1 to 255 unpadded bytes".to_string()));
            }
            normalized.insert(event_type.clone());
        }
        if normalized.is_empty() {
            return Err(Error::Validation("outbox consumer must register at least one event type".to_string()));
        }
        let txn = self.db.begin().await?;
        for event_type in normalized {
            subscription::Entity::insert(subscription::ActiveModel {
                event_type: Set(event_type), consumer_key: Set(consumer_key.to_string()), created_at: Set(Utc::now()),
            })
            .on_conflict(OnConflict::columns([subscription::Column::EventType, subscription::Column::ConsumerKey]).do_nothing().to_owned())
            .exec_without_returning(&txn).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    /// Acknowledges a terminal owner receipt during startup backfill. A missing
    /// event row is already outside Outbox retention and needs no further hold.
    pub async fn acknowledge_existing(&self, consumer_key: &str, outbox_event_id: Uuid) -> Result<()> {
        validate_consumer_key(consumer_key)?;
        if outbox_event_id.is_nil() { return Err(Error::Validation("outbox consumer acknowledgement event id must not be nil".to_string())); }
        let txn = self.db.begin().await?;
        let Some(event) = sys_event::Entity::find_by_id(outbox_event_id).one(&txn).await? else {
            txn.commit().await?;
            return Ok(());
        };
        let registered = subscription::Entity::find()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .filter(subscription::Column::EventType.eq(event.event_type.as_str()))
            .one(&txn)
            .await?;
        if registered.is_none() {
            txn.commit().await?;
            return Ok(());
        }
        Self::acknowledge_in_transaction(&txn, consumer_key, outbox_event_id, event.event_type.as_str()).await?;
        txn.commit().await?;
        Ok(())
    }

    /// Retires a consumer only after every existing event for its registered
    /// event types has a terminal acknowledgement. Stop the worker and quiesce
    /// producers of those event types before calling.
    pub async fn retire_if_drained(&self, consumer_key: &str) -> Result<()> {
        validate_consumer_key(consumer_key)?;
        let txn = self.db.begin().await?;
        let subscriptions = subscription::Entity::find()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .all(&txn)
            .await?;
        for subscription in subscriptions {
            let acknowledgements = receipt::Entity::find()
                .select_only()
                .column(receipt::Column::OutboxEventId)
                .filter(receipt::Column::ConsumerKey.eq(consumer_key))
                .into_query();
            let pending = sys_event::Entity::find()
                .filter(sys_event::Column::EventType.eq(subscription.event_type))
                .filter(sys_event::Column::Id.not_in_subquery(acknowledgements))
                .one(&txn)
                .await?;
            if let Some(event) = pending {
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
    pub async fn acknowledge_in_transaction(txn: &DatabaseTransaction, consumer_key: &str, outbox_event_id: Uuid, event_type: &str) -> std::result::Result<(), sea_orm::DbErr> {
        if consumer_key.trim().is_empty() || consumer_key.trim() != consumer_key || consumer_key.len() > 128 || outbox_event_id.is_nil() || event_type.trim().is_empty() || event_type.trim() != event_type || event_type.len() > 255 {
            return Err(sea_orm::DbErr::Custom("invalid outbox consumer acknowledgement identity".to_string()));
        }
        let event = sys_event::Entity::find_by_id(outbox_event_id).one(txn).await?.ok_or_else(|| sea_orm::DbErr::RecordNotFound(format!("sys_events row {outbox_event_id}")))?;
        if event.event_type != event_type {
            return Err(sea_orm::DbErr::Custom("outbox consumer acknowledgement event type mismatch".to_string()));
        }
        let registered = subscription::Entity::find()
            .filter(subscription::Column::ConsumerKey.eq(consumer_key))
            .filter(subscription::Column::EventType.eq(event_type))
            .one(txn).await?;
        if registered.is_none() {
            return Err(sea_orm::DbErr::Custom("outbox consumer is not registered for this event type".to_string()));
        }
        let affected = receipt::Entity::insert(receipt::ActiveModel {
            outbox_event_id: Set(outbox_event_id), consumer_key: Set(consumer_key.to_string()), event_type: Set(event_type.to_string()), acknowledged_at: Set(Utc::now()),
        })
        .on_conflict(OnConflict::columns([receipt::Column::OutboxEventId, receipt::Column::ConsumerKey]).do_nothing().to_owned())
        .exec_without_returning(txn).await?;
        if affected == 1 { return Ok(()); }
        if affected != 0 { return Err(sea_orm::DbErr::Custom(format!("outbox consumer acknowledgement affected {affected} rows"))); }
        let existing = receipt::Entity::find()
            .filter(receipt::Column::OutboxEventId.eq(outbox_event_id))
            .filter(receipt::Column::ConsumerKey.eq(consumer_key))
            .one(txn).await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound(format!("sys_event_consumer_receipts row {consumer_key}/{outbox_event_id}")))?;
        if existing.event_type != event_type {
            return Err(sea_orm::DbErr::Custom("outbox consumer acknowledgement conflicts with its prior event type".to_string()));
        }
        Ok(())
    }
}

fn validate_consumer_key(consumer_key: &str) -> Result<()> {
    if consumer_key.trim().is_empty() || consumer_key.trim() != consumer_key || consumer_key.len() > 128 {
        return Err(Error::Validation("outbox consumer key must contain 1 to 128 unpadded bytes".to_string()));
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
        let db = Database::connect("sqlite::memory:").await.expect("in-memory sqlite");
        crate::SysEventsMigration.up(&SchemaManager::new(&db)).await.expect("outbox schema");
        db
    }

    #[tokio::test]
    async fn consumer_registration_and_acknowledgement_are_idempotent() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let key = "notifications.outbox-intake";
        let event_type = "tenant.updated".to_string();
        consumer.register_required_event_types(key, std::slice::from_ref(&event_type)).await.expect("register");
        consumer.register_required_event_types(key, std::slice::from_ref(&event_type)).await.expect("re-register");
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("persist event");
        for _ in 0..2 {
            let txn = db.begin().await.expect("begin ack");
            OutboxConsumer::acknowledge_in_transaction(&txn, key, envelope.id, envelope.event_type.as_str()).await.expect("ack event");
            txn.commit().await.expect("commit ack");
        }
        let count = db.query_one(sea_orm::Statement::from_string(db.get_database_backend(), "SELECT COUNT(*) AS count FROM sys_event_consumer_receipts".to_string())).await.expect("query").expect("row").try_get::<i64>("", "count").expect("count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn acknowledgement_requires_registered_event_type() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("persist event");
        let txn = db.begin().await.expect("begin ack");
        assert!(OutboxConsumer::acknowledge_in_transaction(&txn, "unregistered.consumer", envelope.id, envelope.event_type.as_str()).await.is_err());
        txn.rollback().await.expect("rollback");
    }

    #[tokio::test]
    async fn consumer_retirement_requires_a_complete_drain() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db.clone());
        let event_type = "tenant.updated".to_string();
        consumer.register_required_event_types("consumer.retiring", std::slice::from_ref(&event_type)).await.expect("register consumer");
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.expect("persist event");
        assert!(consumer.retire_if_drained("consumer.retiring").await.is_err());
        consumer.acknowledge_existing("consumer.retiring", envelope.id).await.expect("acknowledge event");
        consumer.retire_if_drained("consumer.retiring").await.expect("retire drained consumer");
    }

    #[tokio::test]
    async fn registration_rejects_empty_or_invalid_contracts() {
        let db = test_db().await;
        let consumer = OutboxConsumer::new(db);
        assert!(consumer.register_required_event_types("", &["tenant.updated".to_string()]).await.is_err());
        assert!(consumer.register_required_event_types("valid.consumer", &[]).await.is_err());
        assert!(consumer.register_required_event_types("valid.consumer", &[" tenant.updated".to_string()]).await.is_err());
    }
}

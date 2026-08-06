use std::ops::Deref;

use crate::{
    error::CommerceResult,
    services::index_refresh::{
        product_locale_refresh_target, record_product_locale_refreshes_in_tx,
    },
};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    QueryResult, Statement, TransactionTrait,
};
use uuid::Uuid;

/// Owns one product write transaction and its transactional outbox publisher.
///
/// Product entity changes and domain events must use the same database
/// transaction. The wrapper makes publishing through any non-transactional
/// transport unavailable to product write paths before the transaction commits.
pub(crate) struct ProductWriteTransaction {
    transaction: DatabaseTransaction,
    event_bus: TransactionalEventBus,
}

impl ProductWriteTransaction {
    pub(crate) async fn begin(
        db: &DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> CommerceResult<Self> {
        Ok(Self {
            transaction: db.begin().await?,
            event_bus,
        })
    }

    pub(crate) async fn publish(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> CommerceResult<()> {
        let product_id = product_locale_refresh_target(&event);
        let root_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(&self.transaction, tenant_id, actor_id, event)
            .await?;

        if let Some(product_id) = product_id {
            // Capture the exact post-command Product source state in the same
            // transaction as the durable root event. Any source/ledger failure
            // rolls back both the owner mutation and its event publication.
            record_product_locale_refreshes_in_tx(
                &self.transaction,
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn commit(self) -> CommerceResult<()> {
        self.transaction.commit().await?;
        Ok(())
    }
}

impl Deref for ProductWriteTransaction {
    type Target = DatabaseTransaction;

    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for ProductWriteTransaction {
    fn get_database_backend(&self) -> DbBackend {
        self.transaction.get_database_backend()
    }

    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        self.transaction.execute(statement).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.transaction.execute_unprepared(sql).await
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.transaction.query_one(statement).await
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.transaction.query_all(statement).await
    }

    fn support_returning(&self) -> bool {
        self.transaction.support_returning()
    }

    fn is_mock_connection(&self) -> bool {
        self.transaction.is_mock_connection()
    }
}

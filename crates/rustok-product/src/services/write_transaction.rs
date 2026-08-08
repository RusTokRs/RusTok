use std::{future::Future, ops::Deref};

use crate::{
    error::{CommerceError, CommerceResult},
    services::index_refresh::{
        product_locale_refresh_target, record_product_locale_refreshes_in_tx,
        record_product_variant_refreshes_in_tx,
    },
};
use rustok_events::DomainEvent;
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, ExecResult,
    QueryResult, Statement, TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone)]
struct ProductOperationReceipt {
    lease: idempotency::Lease,
    response_json: Value,
}

tokio::task_local! {
    static PRODUCT_OPERATION_RECEIPT: ProductOperationReceipt;
}

/// Bind one Product owner receipt to the current async write execution.
///
/// The scope is intentionally task-local: the shared `ProductCatalogSchemaService`
/// stays stateless across concurrent callers, while `ProductWriteTransaction::begin`
/// captures the receipt only for the explicitly wrapped owner command.
pub(crate) async fn with_product_operation_receipt<F, T>(
    lease: idempotency::Lease,
    response_json: Value,
    future: F,
) -> T
where
    F: Future<Output = T>,
{
    PRODUCT_OPERATION_RECEIPT
        .scope(
            ProductOperationReceipt {
                lease,
                response_json,
            },
            future,
        )
        .await
}

/// The receipt operation UUID is also a stable Product-owned resource identity
/// for create operations that opt into durable owner replay.
pub(crate) fn current_product_operation_id() -> Option<Uuid> {
    PRODUCT_OPERATION_RECEIPT
        .try_with(|receipt| receipt.lease.operation_id)
        .ok()
}

/// Owns one product write transaction and its transactional outbox publisher.
///
/// Product entity changes and domain events must use the same database
/// transaction. The wrapper makes publishing through any non-transactional
/// transport unavailable to product write paths before the transaction commits.
/// When a Product owner receipt scope is active, terminal success is completed
/// in this same transaction before commit.
pub(crate) struct ProductWriteTransaction {
    transaction: DatabaseTransaction,
    event_bus: TransactionalEventBus,
    operation_receipt: Option<ProductOperationReceipt>,
}

impl ProductWriteTransaction {
    pub(crate) async fn begin(
        db: &DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> CommerceResult<Self> {
        let operation_receipt = PRODUCT_OPERATION_RECEIPT.try_with(Clone::clone).ok();
        Ok(Self {
            transaction: db.begin().await?,
            event_bus,
            operation_receipt,
        })
    }

    pub(crate) async fn publish(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> CommerceResult<()> {
        let product_attribute_id = product_index_revision_touch_target(&event);
        if let Some(product_id) = product_attribute_id {
            self.bump_product_index_revision(tenant_id, product_id)
                .await?;
        }

        // Existing lifecycle Product events keep their established Product + ProductVariant fan-out.
        // ProductAttributeValuesChanged is Product-only: it advances the Product clock and locale
        // refresh ledger but must not re-emit every unchanged ProductVariant.
        let lifecycle_product_id = product_locale_refresh_target(&event);
        let product_locale_id = lifecycle_product_id.or(product_attribute_id);
        let product_variant_id = lifecycle_product_id;
        let root_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(&self.transaction, tenant_id, actor_id, event)
            .await?;

        if let Some(product_id) = product_locale_id {
            // Capture the exact post-command Product source state. Any source/ledger failure rolls
            // back both the owner mutation and its event publication.
            record_product_locale_refreshes_in_tx(
                &self.transaction,
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(product_id) = product_variant_id {
            record_product_variant_refreshes_in_tx(
                &self.transaction,
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;
        }

        Ok(())
    }

    async fn bump_product_index_revision(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()> {
        if self.transaction.get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }

        // `trg_products_bump_index_revision` owns the actual +1 operation. Updating only the clock
        // column avoids fabricating Product-SalesChannel convergence work because that trigger listens
        // only to metadata/tenant/id changes. The update is in the same transaction as EAV writes,
        // outbox publication, graph projection and refresh-ledger capture.
        let result = self
            .transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE products SET index_revision = index_revision WHERE tenant_id = $1 AND id = $2",
                vec![tenant_id.into(), product_id.into()],
            ))
            .await?;
        if result.rows_affected() != 1 {
            return Err(CommerceError::Validation(
                "Product attribute Index revision target is missing".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn commit(self) -> CommerceResult<()> {
        if let Some(receipt) = self.operation_receipt.as_ref() {
            idempotency::complete(&self.transaction, receipt.lease, &receipt.response_json)
                .await
                .map_err(|error| {
                    tracing::error!(
                        operation_id = %receipt.lease.operation_id,
                        internal_code = %error.code,
                        retryable = error.retryable,
                        "Product owner receipt completion failed inside write transaction"
                    );
                    CommerceError::Database(DbErr::Custom(format!(
                        "product owner receipt completion failed: {}",
                        error.code
                    )))
                })?;
        }
        self.transaction.commit().await?;
        Ok(())
    }
}

fn product_index_revision_touch_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductAttributeValuesChanged { product_id } => Some(*product_id),
        _ => None,
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

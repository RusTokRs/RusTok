use std::{
    future::Future,
    ops::Deref,
    sync::{Arc, Mutex},
};

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
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone)]
struct ProductOperationReceipt {
    lease: idempotency::Lease,
    response_json: Arc<Mutex<Option<Value>>>,
}

tokio::task_local! {
    static PRODUCT_OPERATION_RECEIPT: ProductOperationReceipt;
}

/// Bind one Product owner receipt to the current async write execution.
///
/// The scope is intentionally task-local: the shared `ProductCatalogSchemaService`
/// stays stateless across concurrent callers, while `ProductWriteTransaction::begin`
/// captures the receipt only for the explicitly wrapped owner command. The concrete
/// owner write method records its actual result before commit, so transaction-derived
/// fields such as category path never need a preflight read outside the owner transaction.
pub(crate) async fn with_product_operation_receipt<F, T>(lease: idempotency::Lease, future: F) -> T
where
    F: Future<Output = T>,
{
    PRODUCT_OPERATION_RECEIPT
        .scope(
            ProductOperationReceipt {
                lease,
                response_json: Arc::new(Mutex::new(None)),
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

/// Record the actual owner result that must be committed into the active receipt.
///
/// Direct internal Product callers have no receipt scope, so this is intentionally a
/// no-op for them. Receipt-scoped callers must record one result before transaction
/// commit; a missing or poisoned result slot fails closed and rolls back the owner write.
pub(crate) fn record_product_operation_result<T: Serialize>(value: &T) -> CommerceResult<()> {
    let response_json = serde_json::to_value(value).map_err(|error| {
        CommerceError::Database(DbErr::Custom(format!(
            "product owner receipt result encoding failed: {error}"
        )))
    })?;

    match PRODUCT_OPERATION_RECEIPT.try_with(|receipt| {
        let mut slot = receipt.response_json.lock().map_err(|_| {
            CommerceError::Database(DbErr::Custom(
                "product owner receipt result slot is unavailable".to_string(),
            ))
        })?;
        *slot = Some(response_json);
        Ok(())
    }) {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}

/// Owns one product write transaction and its transactional outbox publisher.
///
/// Product entity changes and domain events must use the same database
/// transaction. The wrapper makes publishing through any non-transactional
/// transport unavailable to product write paths before the transaction commits.
/// When a Product owner receipt scope is active, terminal success is completed
/// in this same transaction before commit using the actual result recorded by the
/// owner write method.
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
            // The same atomic boundary includes both refresh ledgers:
            // any failure rolls back both the owner mutation and its event publication.
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
            .execute_raw(Statement::from_sql_and_values(
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
            let response_json = receipt
                .response_json
                .lock()
                .map_err(|_| {
                    CommerceError::Database(DbErr::Custom(
                        "product owner receipt result slot is unavailable".to_string(),
                    ))
                })?
                .clone()
                .ok_or_else(|| {
                    CommerceError::Database(DbErr::Custom(
                        "product owner receipt result was not recorded before commit".to_string(),
                    ))
                })?;
            idempotency::complete(&self.transaction, receipt.lease, &response_json)
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

    async fn execute<S: sea_orm::StatementBuilder>(&self, stmt: &S) -> Result<ExecResult, DbErr> {
        self.transaction.execute(stmt).await
    }

    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.transaction.execute_raw(stmt).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.transaction.execute_unprepared(sql).await
    }

    async fn query_one<S: sea_orm::StatementBuilder>(
        &self,
        stmt: &S,
    ) -> Result<Option<QueryResult>, DbErr> {
        self.transaction.query_one(stmt).await
    }

    async fn query_one_raw(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.transaction.query_one_raw(stmt).await
    }

    async fn query_all<S: sea_orm::StatementBuilder>(
        &self,
        stmt: &S,
    ) -> Result<Vec<QueryResult>, DbErr> {
        self.transaction.query_all(stmt).await
    }

    async fn query_all_raw(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.transaction.query_all_raw(stmt).await
    }

    fn support_returning(&self) -> bool {
        self.transaction.support_returning()
    }

    fn is_mock_connection(&self) -> bool {
        self.transaction.is_mock_connection()
    }
}

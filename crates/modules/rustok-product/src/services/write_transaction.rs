use std::{
    future::Future,
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{
    error::{CommerceError, CommerceResult},
    services::{
        catalog::{
            record_product_image_translation_changes_in_tx,
            record_product_translation_change_in_tx,
            record_product_variant_translation_changes_in_tx,
        },
        catalog_schema_service::ProductCatalogSchemaService,
        index_refresh::{
            product_locale_refresh_target, record_product_locale_refreshes_in_tx,
            record_product_variant_refreshes_in_tx,
        },
    },
};
use rustok_api::PortError;
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
    // Encoding happens before `commit`, but encoding failure must still cross the transaction
    // boundary so `commit` can await rollback before reporting it to the caller.
    response_json: Arc<Mutex<Option<Result<Value, String>>>>,
    // A receipt-bound write that returns early cannot let `DatabaseTransaction::Drop` be the
    // only rollback mechanism. SeaORM queues that rollback for the next connection operation;
    // a separate receipt finalizer could otherwise record a terminal failure before owner writes
    // and locks are known to be released. `ProductWriteTransaction::Drop` moves the live
    // transaction here, and the enclosing receipt scope awaits each rollback before exposing the
    // command result to its failure finalizer.
    deferred_rollbacks: Arc<Mutex<Vec<DatabaseTransaction>>>,
}

impl ProductOperationReceipt {
    fn new(lease: idempotency::Lease) -> Self {
        Self {
            lease,
            response_json: Arc::new(Mutex::new(None)),
            deferred_rollbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn defer_rollback(&self, transaction: DatabaseTransaction) {
        let mut deferred = match self.deferred_rollbacks.lock() {
            Ok(deferred) => deferred,
            Err(poisoned) => {
                tracing::error!(
                    operation_id = %self.lease.operation_id,
                    "Product owner deferred rollback slot was poisoned; recovering pending transaction"
                );
                poisoned.into_inner()
            }
        };
        deferred.push(transaction);
    }

    async fn rollback_deferred_transactions(&self) -> CommerceResult<usize> {
        let deferred = {
            let mut deferred = match self.deferred_rollbacks.lock() {
                Ok(deferred) => deferred,
                Err(poisoned) => {
                    tracing::error!(
                        operation_id = %self.lease.operation_id,
                        "Product owner deferred rollback slot was poisoned while finalizing"
                    );
                    poisoned.into_inner()
                }
            };
            std::mem::take(&mut *deferred)
        };
        let count = deferred.len();

        let mut rollback_failed = false;
        for transaction in deferred {
            if let Err(rollback_error) = transaction.rollback().await {
                rollback_failed = true;
                tracing::error!(
                    operation_id = %self.lease.operation_id,
                    %rollback_error,
                    "failed to roll back Product owner transaction before receipt failure finalization"
                );
            }
        }

        if rollback_failed {
            return Err(CommerceError::Database(DbErr::Custom(
                "product owner transaction rollback failed before receipt failure finalization"
                    .to_owned(),
            )));
        }

        Ok(count)
    }
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
/// If an owner method returns before committing, this scope awaits its rollback before returning
/// the typed error to any independent receipt failure finalizer.
pub(crate) async fn with_product_operation_receipt<F, T, E>(
    lease: idempotency::Lease,
    future: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: From<CommerceError>,
{
    let receipt = ProductOperationReceipt::new(lease);
    let result = PRODUCT_OPERATION_RECEIPT
        .scope(receipt.clone(), future)
        .await;
    let rolled_back = receipt
        .rollback_deferred_transactions()
        .await
        .map_err(E::from)?;

    if result.is_ok() && rolled_back > 0 {
        return Err(E::from(CommerceError::Database(DbErr::Custom(
            "product owner write returned success without committing its transaction".to_owned(),
        ))));
    }

    result
}

/// Persists only a deterministic Product owner failure into an admitted receipt.
///
/// Retryable port errors represent an unknown/transient infrastructure outcome. They must retain
/// the processing lease so a later admission can reclaim the operation rather than replaying a
/// terminal unavailable response for the caller's idempotency key.
pub(crate) async fn fail_product_operation_receipt(
    database: &DatabaseConnection,
    lease: idempotency::Lease,
    error: &PortError,
) -> Result<(), PortError> {
    if error.retryable {
        return Ok(());
    }

    idempotency::fail(database, lease, error).await
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
/// Direct internal Product callers have no receipt scope, so this is intentionally a no-op for
/// them. Receipt-scoped callers defer encoding failures to `ProductWriteTransaction::commit`:
/// every caller still owns a live transaction there, allowing it to await rollback before the
/// error escapes. Returning an encoding or poisoned-slot error here would drop that transaction
/// asynchronously through SeaORM's `Drop` path.
pub(crate) fn record_product_operation_result<T: Serialize>(value: &T) {
    let _ = PRODUCT_OPERATION_RECEIPT.try_with(|receipt| {
        let response_json = serde_json::to_value(value).map_err(|error| error.to_string());
        let mut slot = match receipt.response_json.lock() {
            Ok(slot) => slot,
            Err(poisoned) => {
                tracing::error!(
                    operation_id = %receipt.lease.operation_id,
                    "Product owner receipt result slot was poisoned; replacing its pending result"
                );
                poisoned.into_inner()
            }
        };
        *slot = Some(response_json);
    });
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
    transaction: Option<DatabaseTransaction>,
    event_bus: TransactionalEventBus,
    operation_receipt: Option<ProductOperationReceipt>,
}

/// Rolls back an owner mutation synchronously when receipt finalization cannot proceed.
///
/// SeaORM's transaction drop path starts rollback asynchronously. That is not sufficient after a
/// receipt lease was rejected: a reclaimed caller may begin the same owner operation immediately,
/// so the failed owner transaction must release its writes and locks before this function returns.
async fn rollback_product_receipt_failure(
    transaction: DatabaseTransaction,
    lease: idempotency::Lease,
    error: CommerceError,
) -> CommerceResult<()> {
    transaction.rollback().await.map_err(|rollback_error| {
        tracing::error!(
            operation_id = %lease.operation_id,
            %rollback_error,
            "failed to roll back Product owner transaction after receipt finalization failure"
        );
        CommerceError::Database(DbErr::Custom(
            "product owner transaction rollback failed after receipt finalization failure"
                .to_owned(),
        ))
    })?;
    Err(error)
}

/// Completes a Product owner receipt and commits the owner transaction as one atomic outcome.
async fn complete_product_receipt_and_commit(
    transaction: DatabaseTransaction,
    receipt: ProductOperationReceipt,
) -> CommerceResult<()> {
    let response_json = receipt
        .response_json
        .lock()
        .map_err(|_| {
            CommerceError::Database(DbErr::Custom(
                "product owner receipt result slot is unavailable".to_string(),
            ))
        })
        .and_then(|slot| match slot.clone() {
            Some(Ok(response_json)) => Ok(response_json),
            Some(Err(error)) => Err(CommerceError::Database(DbErr::Custom(format!(
                "product owner receipt result encoding failed: {error}"
            )))),
            None => Err(CommerceError::Database(DbErr::Custom(
                "product owner receipt result was not recorded before commit".to_string(),
            ))),
        });
    let response_json = match response_json {
        Ok(response_json) => response_json,
        Err(error) => {
            return rollback_product_receipt_failure(transaction, receipt.lease, error).await;
        }
    };

    if let Err(error) = idempotency::complete(&transaction, receipt.lease, &response_json).await {
        tracing::error!(
            operation_id = %receipt.lease.operation_id,
            internal_code = %error.code,
            retryable = error.retryable,
            "Product owner receipt completion failed inside write transaction"
        );
        return rollback_product_receipt_failure(
            transaction,
            receipt.lease,
            CommerceError::Database(DbErr::Custom(format!(
                "product owner receipt completion failed: {}",
                error.code
            ))),
        )
        .await;
    }

    transaction.commit().await?;
    Ok(())
}

impl ProductWriteTransaction {
    pub(crate) async fn begin(
        db: &DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> CommerceResult<Self> {
        let operation_receipt = PRODUCT_OPERATION_RECEIPT.try_with(Clone::clone).ok();
        Ok(Self {
            transaction: Some(db.begin().await?),
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
        self.publish_internal(tenant_id, actor_id, event, None)
            .await
    }

    pub(crate) async fn publish_product_deleted(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        product_id: Uuid,
        deleted_image_ids: &[Uuid],
    ) -> CommerceResult<()> {
        self.publish_internal(
            tenant_id,
            actor_id,
            DomainEvent::ProductDeleted { product_id },
            Some(deleted_image_ids),
        )
        .await
    }

    async fn publish_internal(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
        deleted_image_ids: Option<&[Uuid]>,
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
        let variant_translation_change_target = product_variant_translation_change_target(&event);
        let attribute_translation_change_target =
            product_attribute_translation_change_target(&event);
        let attribute_schema_translation_change_target =
            product_attribute_schema_translation_change_target(&event);
        let category_form_translation_change_target =
            product_category_form_translation_change_target(&event);
        let category_seo_translation_change_target =
            product_category_seo_translation_change_target(&event);
        let attribute_value_translation_change_target =
            product_attribute_value_translation_change_target(&event);
        let variant_attribute_value_translation_change_target =
            product_variant_attribute_value_translation_change_target(&event);
        let root_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(self.active_transaction(), tenant_id, actor_id, event)
            .await?;

        if let Some(product_id) = lifecycle_product_id {
            // Translation change evidence is captured from the exact post-command owner state and
            // committed with the same Product event. Unrelated ProductUpdated events are suppressed
            // by the journal when the Translation resource revision and lifecycle are unchanged.
            record_product_translation_change_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;

            // Image Translation revisions also include parent Product lifecycle and Image owner
            // semantics. Live Product events fan out through exact post-command Image state with
            // semantic dedupe. Product deletion supplies pre-delete Image identities so even an
            // Image that predates the journal receives durable first-delete evidence.
            record_product_image_translation_changes_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
                deleted_image_ids,
            )
            .await?;
        }

        if let Some((product_id, variant_id)) = variant_translation_change_target {
            // Variant Translation revisions include the parent Product lifecycle. Product lifecycle
            // events therefore fan out across that Product's Variants, while explicit Variant events
            // can narrow capture to one child. The journal suppresses unchanged semantic revisions.
            record_product_variant_translation_changes_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
                variant_id,
            )
            .await?;
        }

        if let Some(attribute_id) = attribute_translation_change_target {
            // Attribute definition copy and active option labels form one Product-owned Translation
            // aggregate. Every canonical Attribute/Option event recomputes the exact post-command
            // aggregate inside this transaction. Semantic dedupe suppresses non-copy owner changes.
            ProductCatalogSchemaService::record_attribute_translation_change_in_tx(
                self.active_transaction(),
                tenant_id,
                attribute_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(schema_id) = attribute_schema_translation_change_target {
            // Attribute Schema name/description and schema-group labels form one Product-owned
            // Translation aggregate. Binding-only events intentionally pass this boundary too;
            // semantic revision dedupe suppresses them unless presentation copy actually changed.
            ProductCatalogSchemaService::record_attribute_schema_translation_change_in_tx(
                self.active_transaction(),
                tenant_id,
                schema_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(category_id) = category_form_translation_change_target {
            // Product category-form copy owns only category-local group labels. Category canonical
            // name/slug/description remain Taxonomy-owned. Binding/schema-mode events pass this
            // boundary so semantic revision dedupe can prove they do not manufacture copy changes.
            ProductCatalogSchemaService::record_category_form_translation_change_in_tx(
                self.active_transaction(),
                tenant_id,
                category_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(category_id) = category_seo_translation_change_target {
            // Product Category SEO is a separate owner surface from Taxonomy canonical Category copy
            // and from SEO-module overrides. Canonical Category lifecycle events capture the exact
            // post-command SEO aggregate; semantic revision/lifecycle dedupe suppresses unrelated edits.
            ProductCatalogSchemaService::record_category_seo_translation_change_in_tx(
                self.active_transaction(),
                tenant_id,
                category_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(product_id) = attribute_value_translation_change_target {
            // Localized Product EAV values are independent Translation resources keyed by their
            // canonical value-row UUID. Only value writes and Product hard-delete cross this boundary;
            // dynamic category/schema membership never manufactures EAV Translation changes.
            ProductCatalogSchemaService::record_attribute_value_translation_changes_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;
        }

        if let Some((product_id, variant_id)) = variant_attribute_value_translation_change_target {
            // Localized Variant EAV values are independent row-owned Translation resources. Canonical
            // Variant EAV writes surface as VariantUpdated, while Variant/Product hard-delete records
            // tombstones from the durable journal after storage cascade deletion.
            ProductCatalogSchemaService::record_variant_attribute_value_translation_changes_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
                variant_id,
            )
            .await?;
        }

        if let Some(product_id) = product_locale_id {
            // Capture the exact post-command Product source state. Any source/ledger failure rolls
            // back both the owner mutation and its event publication.
            // The same atomic boundary includes both refresh ledgers:
            // any failure rolls back both the owner mutation and its event publication.
            record_product_locale_refreshes_in_tx(
                self.active_transaction(),
                tenant_id,
                product_id,
                root_event_id,
            )
            .await?;
        }

        if let Some(product_id) = product_variant_id {
            record_product_variant_refreshes_in_tx(
                self.active_transaction(),
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
        if self.active_transaction().get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }

        // `trg_products_bump_index_revision` owns the actual +1 operation. Updating only the clock
        // column avoids fabricating Product-SalesChannel convergence work because that trigger listens
        // only to metadata/tenant/id changes. The update is in the same transaction as EAV writes,
        // outbox publication, graph projection and refresh-ledger capture.
        let result = self
            .active_transaction()
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

    pub(crate) async fn commit(mut self) -> CommerceResult<()> {
        let transaction = self.transaction.take().ok_or_else(|| {
            CommerceError::Database(DbErr::Custom(
                "Product write transaction was consumed before commit".to_owned(),
            ))
        })?;

        if let Some(receipt) = self.operation_receipt.take() {
            return complete_product_receipt_and_commit(transaction, receipt).await;
        }

        transaction.commit().await?;
        Ok(())
    }

    fn active_transaction(&self) -> &DatabaseTransaction {
        self.transaction
            .as_ref()
            .expect("ProductWriteTransaction used after its transaction was consumed")
    }
}

impl Drop for ProductWriteTransaction {
    fn drop(&mut self) {
        let Some(transaction) = self.transaction.take() else {
            return;
        };

        if let Some(receipt) = &self.operation_receipt {
            receipt.defer_rollback(transaction);
        }
        // Direct Product callers have no receipt scope. Let the transaction retain SeaORM's
        // normal best-effort drop rollback behavior for those paths.
    }
}

fn product_index_revision_touch_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductAttributeValuesChanged { product_id } => Some(*product_id),
        _ => None,
    }
}

fn product_attribute_translation_change_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductAttributeCreated { attribute_id }
        | DomainEvent::ProductAttributeUpdated { attribute_id }
        | DomainEvent::ProductAttributeDeleted { attribute_id }
        | DomainEvent::ProductAttributeOptionCreated { attribute_id, .. }
        | DomainEvent::ProductAttributeOptionUpdated { attribute_id, .. }
        | DomainEvent::ProductAttributeOptionDeleted { attribute_id, .. } => Some(*attribute_id),
        _ => None,
    }
}

fn product_attribute_schema_translation_change_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductAttributeSchemaCreated { schema_id }
        | DomainEvent::ProductAttributeSchemaUpdated { schema_id }
        | DomainEvent::ProductAttributeSchemaDeleted { schema_id }
        | DomainEvent::ProductAttributeSchemaBindingsChanged { schema_id } => Some(*schema_id),
        _ => None,
    }
}

fn product_category_form_translation_change_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::CatalogCategoryCreated { category_id }
        | DomainEvent::CatalogCategoryUpdated { category_id }
        | DomainEvent::CatalogCategoryDeleted { category_id }
        | DomainEvent::CatalogCategoryAttributesChanged { category_id }
        | DomainEvent::CatalogCategorySchemaModeChanged { category_id } => Some(*category_id),
        _ => None,
    }
}

fn product_category_seo_translation_change_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::CatalogCategoryCreated { category_id }
        | DomainEvent::CatalogCategoryUpdated { category_id }
        | DomainEvent::CatalogCategoryDeleted { category_id } => Some(*category_id),
        _ => None,
    }
}

fn product_attribute_value_translation_change_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductAttributeValuesChanged { product_id }
        | DomainEvent::ProductDeleted { product_id } => Some(*product_id),
        _ => None,
    }
}

fn product_variant_attribute_value_translation_change_target(
    event: &DomainEvent,
) -> Option<(Uuid, Option<Uuid>)> {
    match event {
        DomainEvent::ProductDeleted { product_id } => Some((*product_id, None)),
        DomainEvent::VariantUpdated {
            variant_id,
            product_id,
        }
        | DomainEvent::VariantDeleted {
            variant_id,
            product_id,
        } => Some((*product_id, Some(*variant_id))),
        _ => None,
    }
}

fn product_variant_translation_change_target(event: &DomainEvent) -> Option<(Uuid, Option<Uuid>)> {
    match event {
        DomainEvent::ProductCreated { product_id }
        | DomainEvent::ProductUpdated { product_id }
        | DomainEvent::ProductPublished { product_id }
        | DomainEvent::ProductDeleted { product_id } => Some((*product_id, None)),
        DomainEvent::VariantCreated {
            variant_id,
            product_id,
        }
        | DomainEvent::VariantUpdated {
            variant_id,
            product_id,
        }
        | DomainEvent::VariantDeleted {
            variant_id,
            product_id,
        } => Some((*product_id, Some(*variant_id))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PRODUCT_OPERATION_RECEIPT, ProductOperationReceipt, ProductWriteTransaction,
        complete_product_receipt_and_commit, record_product_operation_result,
        with_product_operation_receipt,
    };
    use crate::CommerceError;
    use rustok_outbox::{SysEventsMigration, idempotency};
    use rustok_test_utils::mock_transactional_event_bus;
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait,
        TryGetable,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use serde::{Serialize, Serializer};
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use uuid::Uuid;

    async fn receipt_probe_database() -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");
        database
            .execute_unprepared("CREATE TABLE product_receipt_probe (id INTEGER PRIMARY KEY)")
            .await
            .expect("probe table should exist");
        database
    }

    async fn receipt_probe_databases_with_independent_connections() -> (
        sea_orm::DatabaseConnection,
        sea_orm::DatabaseConnection,
        PathBuf,
    ) {
        let path = std::env::temp_dir().join(format!(
            "rustok-product-receipt-rollback-{}.sqlite",
            Uuid::new_v4()
        ));
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let mut owner_options = ConnectOptions::new(url.clone());
        owner_options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let owner_database = Database::connect(owner_options)
            .await
            .expect("owner database should connect");
        owner_database
            .execute_unprepared("CREATE TABLE product_receipt_probe (id INTEGER PRIMARY KEY)")
            .await
            .expect("probe table should exist");
        let mut observer_options = ConnectOptions::new(url);
        observer_options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let observer_database = Database::connect(observer_options)
            .await
            .expect("independent observer database should connect");
        observer_database
            .execute_unprepared("PRAGMA busy_timeout = 0")
            .await
            .expect("observer lock timeout should be disabled");

        (owner_database, observer_database, path)
    }

    async fn assert_probe_is_rolled_back(database: &sea_orm::DatabaseConnection) {
        let row = database
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM product_receipt_probe".to_owned(),
            ))
            .await
            .expect("probe count should query")
            .expect("probe count row should exist");
        let count: i64 = row.try_get("", "count").expect("probe count should decode");
        assert_eq!(
            count, 0,
            "receipt finalization failure must await transaction rollback"
        );
    }

    struct FailingReceiptResponse;

    impl Serialize for FailingReceiptResponse {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom(
                "forced Product receipt response encoding failure",
            ))
        }
    }

    #[tokio::test]
    async fn receipt_completion_failure_rolls_back_staged_product_mutation() {
        let database = receipt_probe_database().await;
        let transaction = database.begin().await.expect("transaction should begin");
        transaction
            .execute_unprepared("INSERT INTO product_receipt_probe (id) VALUES (1)")
            .await
            .expect("probe mutation should stage");
        let receipt = ProductOperationReceipt {
            lease: idempotency::Lease {
                operation_id: Uuid::new_v4(),
                token: Uuid::new_v4(),
            },
            response_json: Arc::new(Mutex::new(Some(Ok(json!({"ok": true}))))),
            deferred_rollbacks: Arc::new(Mutex::new(Vec::new())),
        };

        let error = complete_product_receipt_and_commit(transaction, receipt)
            .await
            .expect_err("an unknown receipt lease must reject completion");
        assert!(matches!(error, CommerceError::Database(_)));
        assert_probe_is_rolled_back(&database).await;
    }

    #[tokio::test]
    async fn receipt_result_encoding_failure_rolls_back_staged_product_mutation() {
        let database = receipt_probe_database().await;
        let transaction = database.begin().await.expect("transaction should begin");
        transaction
            .execute_unprepared("INSERT INTO product_receipt_probe (id) VALUES (1)")
            .await
            .expect("probe mutation should stage");
        let lease = idempotency::Lease {
            operation_id: Uuid::new_v4(),
            token: Uuid::new_v4(),
        };

        let error = with_product_operation_receipt(lease, async move {
            record_product_operation_result(&FailingReceiptResponse);
            let receipt = PRODUCT_OPERATION_RECEIPT
                .try_with(Clone::clone)
                .expect("receipt scope should remain active");
            complete_product_receipt_and_commit(transaction, receipt).await
        })
        .await
        .expect_err("a receipt result encoding failure must reject commit");
        assert!(matches!(error, CommerceError::Database(_)));
        assert_probe_is_rolled_back(&database).await;
    }

    #[tokio::test]
    async fn receipt_scoped_early_error_awaits_owner_transaction_rollback() {
        let (database, observer, path) =
            receipt_probe_databases_with_independent_connections().await;
        let lease = idempotency::Lease {
            operation_id: Uuid::new_v4(),
            token: Uuid::new_v4(),
        };

        let error = with_product_operation_receipt(lease, async {
            let transaction =
                ProductWriteTransaction::begin(&database, mock_transactional_event_bus()).await?;
            transaction
                .execute_unprepared("INSERT INTO product_receipt_probe (id) VALUES (1)")
                .await?;

            Err::<(), CommerceError>(CommerceError::Validation(
                "forced Product owner error before commit".to_owned(),
            ))
        })
        .await
        .expect_err("an owner error before commit must be returned after rollback");

        assert!(matches!(error, CommerceError::Validation(_)));
        // This write runs through a different SQLite connection with no busy wait. It would fail
        // with `database is locked` if SeaORM's Drop-queued rollback were still pending when the
        // receipt scope returned. In particular, it cannot make progress by issuing another query
        // against the owner connection, which was the former false-positive test shape.
        observer
            .execute_unprepared("INSERT INTO product_receipt_probe (id) VALUES (2)")
            .await
            .expect(
                "rollback must release the owner write lock before the receipt wrapper returns",
            );

        let row = observer
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM product_receipt_probe".to_owned(),
            ))
            .await
            .expect("probe count should query")
            .expect("probe count row should exist");
        let count: i64 = row.try_get("", "count").expect("probe count should decode");
        assert_eq!(count, 1, "the staged owner mutation must be rolled back");

        observer
            .close()
            .await
            .expect("observer database should close");
        database.close().await.expect("owner database should close");
        for sqlite_file in [
            path.clone(),
            PathBuf::from(format!("{}-shm", path.display())),
            PathBuf::from(format!("{}-wal", path.display())),
        ] {
            let _ = std::fs::remove_file(sqlite_file);
        }
    }

    #[tokio::test]
    async fn missing_receipt_result_rolls_back_staged_product_mutation() {
        let database = receipt_probe_database().await;
        let transaction = database.begin().await.expect("transaction should begin");
        transaction
            .execute_unprepared("INSERT INTO product_receipt_probe (id) VALUES (1)")
            .await
            .expect("probe mutation should stage");
        let receipt = ProductOperationReceipt {
            lease: idempotency::Lease {
                operation_id: Uuid::new_v4(),
                token: Uuid::new_v4(),
            },
            response_json: Arc::new(Mutex::new(None)),
            deferred_rollbacks: Arc::new(Mutex::new(Vec::new())),
        };

        let error = complete_product_receipt_and_commit(transaction, receipt)
            .await
            .expect_err("a missing receipt result must reject commit");
        assert!(matches!(error, CommerceError::Database(_)));
        assert_probe_is_rolled_back(&database).await;
    }
}

impl Deref for ProductWriteTransaction {
    type Target = DatabaseTransaction;

    fn deref(&self) -> &Self::Target {
        self.active_transaction()
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for ProductWriteTransaction {
    fn get_database_backend(&self) -> DbBackend {
        self.active_transaction().get_database_backend()
    }

    async fn execute<S: sea_orm::StatementBuilder>(&self, stmt: &S) -> Result<ExecResult, DbErr> {
        self.active_transaction().execute(stmt).await
    }

    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.active_transaction().execute_raw(stmt).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.active_transaction().execute_unprepared(sql).await
    }

    async fn query_one<S: sea_orm::StatementBuilder>(
        &self,
        stmt: &S,
    ) -> Result<Option<QueryResult>, DbErr> {
        self.active_transaction().query_one(stmt).await
    }

    async fn query_one_raw(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.active_transaction().query_one_raw(stmt).await
    }

    async fn query_all<S: sea_orm::StatementBuilder>(
        &self,
        stmt: &S,
    ) -> Result<Vec<QueryResult>, DbErr> {
        self.active_transaction().query_all(stmt).await
    }

    async fn query_all_raw(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.active_transaction().query_all_raw(stmt).await
    }

    fn support_returning(&self) -> bool {
        self.active_transaction().support_returning()
    }

    fn is_mock_connection(&self) -> bool {
        self.active_transaction().is_mock_connection()
    }
}

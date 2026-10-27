/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Receipt-bound owner commands for Product relations.
//!
//! A relation write is a `PortCallPolicy::write()` operation: the caller supplies a durable
//! idempotency key, the Product Relations owner admits one receipt for it, completes that receipt
//! inside the same database transaction as the write, and replays the stored response for every
//! later retry. A key is scoped to the tenant and to this owner; reusing one key with another
//! payload is rejected as a conflict instead of being silently aliased.

use rustok_api::PortError;
use rustok_outbox::idempotency;
use sea_orm::{DatabaseTransaction, TransactionTrait};
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{
    CreateProductRelationInput, ProductRelationDto, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use crate::error::ProductRelationError;
use crate::services::relation_service::{
    ProductRelationService, create_relation_in_tx, delete_relation_in_tx, reorder_relations_in_tx,
    update_relation_in_tx,
};

/// Durable owner identity for Product relation receipts.
pub const PRODUCT_RELATION_OWNER_SLUG: &str = "product_relations";

/// Receipt operation name of the idempotent relation creation command.
pub const CREATE_RELATION_OPERATION: &str = "create_relation";

/// Receipt operation name of the idempotent relation update command.
pub const UPDATE_RELATION_OPERATION: &str = "update_relation";

/// Receipt operation name of the idempotent relation deletion command.
pub const DELETE_RELATION_OPERATION: &str = "delete_relation";

/// Receipt operation name of the idempotent relation reorder command.
pub const REORDER_RELATIONS_OPERATION: &str = "reorder_relations";

/// Caller identity of one idempotent Product relation command.
#[derive(Clone, Copy, Debug)]
pub struct ProductRelationCommandContext<'a> {
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub idempotency_key: &'a str,
}

impl<'a> ProductRelationCommandContext<'a> {
    pub fn new(tenant_id: Uuid, actor_id: Option<Uuid>, idempotency_key: &'a str) -> Self {
        Self {
            tenant_id,
            actor_id,
            idempotency_key,
        }
    }
}

/// Outcome of one idempotent Product relation command.
#[derive(Debug, Error)]
pub enum ProductRelationCommandError {
    #[error(transparent)]
    Domain(#[from] ProductRelationError),
    #[error(transparent)]
    Receipt(#[from] PortError),
}

impl From<sea_orm::DbErr> for ProductRelationCommandError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Domain(ProductRelationError::Database(err))
    }
}

impl ProductRelationService {
    /// Creates one relation with durable owner replay semantics.
    pub async fn create_relation_idempotent(
        &self,
        context: ProductRelationCommandContext<'_>,
        input: CreateProductRelationInput,
    ) -> Result<ProductRelationDto, ProductRelationCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "input": &input,
        });
        let lease =
            match admit_relation_operation(self, context, CREATE_RELATION_OPERATION, &request)
                .await?
            {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_relation_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match create_relation_in_tx(&txn, context.tenant_id, input).await {
            Ok(relation) => {
                complete_relation_operation(txn, context, lease, &relation).await?;
                Ok(relation)
            }
            Err(error) => {
                rollback_relation_operation(txn, context, lease).await?;
                finish_failed_relation_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Updates one relation with durable owner replay semantics.
    pub async fn update_relation_idempotent(
        &self,
        context: ProductRelationCommandContext<'_>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> Result<ProductRelationDto, ProductRelationCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "relation_id": relation_id,
            "input": &input,
        });
        let lease =
            match admit_relation_operation(self, context, UPDATE_RELATION_OPERATION, &request)
                .await?
            {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_relation_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match update_relation_in_tx(&txn, context.tenant_id, relation_id, input).await {
            Ok(relation) => {
                complete_relation_operation(txn, context, lease, &relation).await?;
                Ok(relation)
            }
            Err(error) => {
                rollback_relation_operation(txn, context, lease).await?;
                finish_failed_relation_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Deletes one relation with durable owner replay semantics.
    pub async fn delete_relation_idempotent(
        &self,
        context: ProductRelationCommandContext<'_>,
        relation_id: Uuid,
    ) -> Result<(), ProductRelationCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "relation_id": relation_id,
        });
        let lease =
            match admit_relation_operation(self, context, DELETE_RELATION_OPERATION, &request)
                .await?
            {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_unit_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match delete_relation_in_tx(&txn, context.tenant_id, relation_id).await {
            Ok(()) => {
                complete_relation_operation(txn, context, lease, &()).await?;
                Ok(())
            }
            Err(error) => {
                rollback_relation_operation(txn, context, lease).await?;
                finish_failed_relation_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Reorders a complete relation group with durable owner replay semantics.
    pub async fn reorder_relations_idempotent(
        &self,
        context: ProductRelationCommandContext<'_>,
        input: ReorderProductRelationsInput,
    ) -> Result<Vec<ProductRelationDto>, ProductRelationCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "input": &input,
        });
        let lease =
            match admit_relation_operation(self, context, REORDER_RELATIONS_OPERATION, &request)
                .await?
            {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_relations_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match reorder_relations_in_tx(&txn, context.tenant_id, input).await {
            Ok(relations) => {
                complete_relation_operation(txn, context, lease, &relations).await?;
                Ok(relations)
            }
            Err(error) => {
                rollback_relation_operation(txn, context, lease).await?;
                finish_failed_relation_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }
}

async fn admit_relation_operation<T: serde::Serialize>(
    service: &ProductRelationService,
    context: ProductRelationCommandContext<'_>,
    operation: &'static str,
    request: &T,
) -> Result<idempotency::Admission, PortError> {
    idempotency::admit(
        service.database(),
        idempotency::OwnerOperationScope::Tenant(context.tenant_id),
        PRODUCT_RELATION_OWNER_SLUG,
        context.idempotency_key,
        operation,
        request,
    )
    .await
}

/// Completes a successful owner receipt and commits its mutation atomically.
///
/// Receipt completion can fail after the domain mutation has staged writes (for example, when a
/// stale lease is detected). Roll back explicitly before returning that error: SeaORM's drop path
/// schedules rollback asynchronously, which is too weak at this receipt boundary because a
/// competing lease holder must not observe a still-open owner transaction.
async fn complete_relation_operation<T: serde::Serialize>(
    txn: DatabaseTransaction,
    context: ProductRelationCommandContext<'_>,
    lease: idempotency::Lease,
    response: &T,
) -> Result<(), ProductRelationCommandError> {
    if let Err(receipt_error) = idempotency::complete(&txn, lease, response).await {
        rollback_relation_operation(txn, context, lease).await?;
        return Err(receipt_error.into());
    }

    txn.commit().await?;
    Ok(())
}

/// Rolls back the owner mutation before a separate connection finalizes a terminal receipt.
///
/// `idempotency::fail` has an explicit post-rollback precondition: writing the receipt while the
/// owner transaction is still open can race its rollback (or block behind its locks). A rollback
/// failure leaves the processing lease reclaimable and is reported as retryable rather than
/// claiming that the domain failure was durably recorded.
async fn rollback_relation_operation(
    txn: DatabaseTransaction,
    context: ProductRelationCommandContext<'_>,
    lease: idempotency::Lease,
) -> Result<(), PortError> {
    txn.rollback().await.map_err(|rollback_error| {
        tracing::error!(
            tenant_id = %context.tenant_id,
            operation_id = %lease.operation_id,
            %rollback_error,
            "failed to roll back Product relation owner transaction before receipt finalization"
        );
        PortError::unavailable(
            "product_relation.transaction_rollback_unavailable",
            "product relation operation could not be completed safely",
        )
    })
}

/// Stores a terminal owner failure so a later retry replays the same typed failure.
async fn finish_failed_relation_operation(
    service: &ProductRelationService,
    context: ProductRelationCommandContext<'_>,
    lease: idempotency::Lease,
    error: &ProductRelationError,
) -> Result<(), PortError> {
    let port_error = relation_command_error(error);
    if port_error.retryable {
        return Ok(());
    }
    idempotency::fail(service.database(), lease, &port_error)
        .await
        .inspect_err(|receipt_error| {
            tracing::error!(
                tenant_id = %context.tenant_id,
                operation_id = %lease.operation_id,
                internal_code = %receipt_error.code,
                "failed to persist Product relation terminal failure receipt"
            );
        })
}

/// Maps one Product relation domain failure onto the owner port error vocabulary.
pub fn relation_command_error(error: &ProductRelationError) -> PortError {
    match error {
        ProductRelationError::SelfRelationNotAllowed(_)
        | ProductRelationError::InvalidInput(_)
        | ProductRelationError::InvalidRelationType(_)
        | ProductRelationError::DuplicateRelationInReorder(_)
        | ProductRelationError::IncompleteReorder { .. }
        | ProductRelationError::PositionOutOfRange { .. } => PortError::validation(
            "product_relation.invalid_input",
            "product relation request is invalid",
        ),
        ProductRelationError::RelationAlreadyExists { .. } => PortError::conflict(
            "product_relation.already_exists",
            "product relation already exists",
        ),
        ProductRelationError::RelationNotFound(_) => PortError::not_found(
            "product_relation.not_found",
            "product relation was not found",
        ),
        ProductRelationError::ProductNotFound(_) => PortError::not_found(
            "product_relation.product_not_found",
            "product relation target product was not found",
        ),
        ProductRelationError::Database(_) | ProductRelationError::Outbox(_) => {
            PortError::unavailable(
                "product_relation.storage_unavailable",
                "product relation storage is temporarily unavailable",
            )
        }
    }
}

fn decode_relation_receipt(
    value: serde_json::Value,
) -> Result<ProductRelationDto, ProductRelationCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product relation owner receipt replay"
        );
        PortError::invariant_violation(
            "product_relation.receipt_corrupt",
            "product relation operation could not be completed safely",
        )
        .into()
    })
}

fn decode_relations_receipt(
    value: serde_json::Value,
) -> Result<Vec<ProductRelationDto>, ProductRelationCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product relation reorder receipt replay"
        );
        PortError::invariant_violation(
            "product_relation.receipt_corrupt",
            "product relation operation could not be completed safely",
        )
        .into()
    })
}

fn decode_unit_receipt(value: serde_json::Value) -> Result<(), ProductRelationCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product relation owner receipt replay"
        );
        PortError::invariant_violation(
            "product_relation.receipt_corrupt",
            "product relation operation could not be completed safely",
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_outbox::SysEventsMigration;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

    #[tokio::test]
    async fn receipt_completion_failure_rolls_back_staged_relation_mutation() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");
        database
            .execute_unprepared("CREATE TABLE relation_receipt_probe (id INTEGER PRIMARY KEY)")
            .await
            .expect("probe table should exist");

        let transaction = database.begin().await.expect("transaction should begin");
        transaction
            .execute_unprepared("INSERT INTO relation_receipt_probe (id) VALUES (1)")
            .await
            .expect("probe mutation should stage");

        let error = complete_relation_operation(
            transaction,
            ProductRelationCommandContext::new(Uuid::new_v4(), None, "receipt-key"),
            idempotency::Lease {
                operation_id: Uuid::new_v4(),
                token: Uuid::new_v4(),
            },
            &serde_json::json!({"ok": true}),
        )
        .await
        .expect_err("an unknown receipt lease must reject completion");
        assert!(matches!(
            error,
            ProductRelationCommandError::Receipt(ref error)
                if error.code == "outbox.operation_receipt_state_invalid"
        ));

        let row = database
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM relation_receipt_probe".to_owned(),
            ))
            .await
            .expect("probe count should query")
            .expect("probe count row should exist");
        let count: i64 = row.try_get("", "count").expect("probe count should decode");
        assert_eq!(
            count, 0,
            "completion failure must await transaction rollback"
        );
    }
}

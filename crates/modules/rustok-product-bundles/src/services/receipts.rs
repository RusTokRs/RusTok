/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Receipt-bound owner commands for Product bundles.
//!
//! Every Product Bundle mutation is a `PortCallPolicy::write()` operation: each caller supplies a
//! durable idempotency key, the Product Bundles owner admits one receipt for it and completes that
//! receipt inside the same database transaction as the write. A retry after a timeout or a double
//! click therefore replays the stored response instead of reapplying a mutation. A key is scoped
//! to the tenant and to this owner; reusing one key with another payload is rejected as a conflict.

use rustok_api::PortError;
use rustok_outbox::idempotency;
use sea_orm::{DatabaseTransaction, TransactionTrait};
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{BundleDto, BundleItemDto, BundleItemInput, CreateBundleInput, UpdateBundleInput};
use crate::error::BundleError;
use crate::services::bundle_service::{
    BundleService, add_bundle_item_in_tx, create_bundle_in_tx, delete_bundle_in_tx,
    remove_bundle_item_in_tx, update_bundle_in_tx,
};

/// Durable owner identity for Product bundle receipts.
pub const PRODUCT_BUNDLE_OWNER_SLUG: &str = "product_bundles";

/// Receipt operation name of the idempotent bundle creation command.
pub const CREATE_BUNDLE_OPERATION: &str = "create_bundle";

/// Receipt operation name of the idempotent bundle-item insertion command.
pub const ADD_BUNDLE_ITEM_OPERATION: &str = "add_bundle_item";

/// Receipt operation name of the idempotent bundle update command.
pub const UPDATE_BUNDLE_OPERATION: &str = "update_bundle";

/// Receipt operation name of the idempotent bundle deletion command.
pub const DELETE_BUNDLE_OPERATION: &str = "delete_bundle";

/// Receipt operation name of the idempotent bundle-item removal command.
pub const REMOVE_BUNDLE_ITEM_OPERATION: &str = "remove_bundle_item";

/// Caller identity of one idempotent Product bundle command.
#[derive(Clone, Copy, Debug)]
pub struct BundleCommandContext<'a> {
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub idempotency_key: &'a str,
}

impl<'a> BundleCommandContext<'a> {
    pub fn new(tenant_id: Uuid, actor_id: Option<Uuid>, idempotency_key: &'a str) -> Self {
        Self {
            tenant_id,
            actor_id,
            idempotency_key,
        }
    }
}

/// Outcome of one idempotent Product bundle command.
#[derive(Debug, Error)]
pub enum BundleCommandError {
    #[error(transparent)]
    Domain(#[from] BundleError),
    #[error(transparent)]
    Receipt(#[from] PortError),
}

impl From<sea_orm::DbErr> for BundleCommandError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Domain(BundleError::Database(err))
    }
}

impl BundleService {
    /// Creates one bundle with durable owner replay semantics.
    pub async fn create_bundle_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        input: CreateBundleInput,
    ) -> Result<BundleDto, BundleCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "input": &input,
        });
        let lease =
            match admit_bundle_operation(self, context, CREATE_BUNDLE_OPERATION, &request).await? {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_bundle_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match create_bundle_in_tx(&txn, context.tenant_id, input).await {
            Ok(bundle) => {
                idempotency::complete(&txn, lease, &bundle).await?;
                txn.commit().await?;
                Ok(bundle)
            }
            Err(error) => {
                rollback_bundle_operation(txn, context, lease).await?;
                finish_failed_bundle_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Adds one item to an existing bundle with durable owner replay semantics.
    pub async fn add_bundle_item_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> Result<BundleItemDto, BundleCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "bundle_id": bundle_id,
            "item": &item,
        });
        let lease = match admit_bundle_operation(self, context, ADD_BUNDLE_ITEM_OPERATION, &request)
            .await?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_bundle_item_receipt(value),
            idempotency::Admission::ReplayError(error) => return Err(error.into()),
        };

        let txn = self.database().begin().await?;
        match add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item).await {
            Ok(created_item) => {
                idempotency::complete(&txn, lease, &created_item).await?;
                txn.commit().await?;
                Ok(created_item)
            }
            Err(error) => {
                rollback_bundle_operation(txn, context, lease).await?;
                finish_failed_bundle_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Updates one bundle with durable owner replay semantics.
    pub async fn update_bundle_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        bundle_id: Uuid,
        input: UpdateBundleInput,
    ) -> Result<BundleDto, BundleCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "bundle_id": bundle_id,
            "input": &input,
        });
        let lease =
            match admit_bundle_operation(self, context, UPDATE_BUNDLE_OPERATION, &request).await? {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_bundle_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match update_bundle_in_tx(&txn, context.tenant_id, bundle_id, input).await {
            Ok(bundle) => {
                idempotency::complete(&txn, lease, &bundle).await?;
                txn.commit().await?;
                Ok(bundle)
            }
            Err(error) => {
                rollback_bundle_operation(txn, context, lease).await?;
                finish_failed_bundle_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Deletes one bundle with durable owner replay semantics.
    pub async fn delete_bundle_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        bundle_id: Uuid,
    ) -> Result<(), BundleCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "bundle_id": bundle_id,
        });
        let lease =
            match admit_bundle_operation(self, context, DELETE_BUNDLE_OPERATION, &request).await? {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_unit_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match delete_bundle_in_tx(&txn, context.tenant_id, bundle_id).await {
            Ok(()) => {
                idempotency::complete(&txn, lease, &()).await?;
                txn.commit().await?;
                Ok(())
            }
            Err(error) => {
                rollback_bundle_operation(txn, context, lease).await?;
                finish_failed_bundle_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }

    /// Removes one bundle item with durable owner replay semantics.
    pub async fn remove_bundle_item_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        bundle_id: Uuid,
        item_id: Uuid,
    ) -> Result<(), BundleCommandError> {
        let request = serde_json::json!({
            "actor": context.actor_id,
            "bundle_id": bundle_id,
            "item_id": item_id,
        });
        let lease =
            match admit_bundle_operation(self, context, REMOVE_BUNDLE_ITEM_OPERATION, &request)
                .await?
            {
                idempotency::Admission::Run(lease) => lease,
                idempotency::Admission::Replay(value) => return decode_unit_receipt(value),
                idempotency::Admission::ReplayError(error) => return Err(error.into()),
            };

        let txn = self.database().begin().await?;
        match remove_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item_id).await {
            Ok(()) => {
                idempotency::complete(&txn, lease, &()).await?;
                txn.commit().await?;
                Ok(())
            }
            Err(error) => {
                rollback_bundle_operation(txn, context, lease).await?;
                finish_failed_bundle_operation(self, context, lease, &error).await?;
                Err(error.into())
            }
        }
    }
}

async fn admit_bundle_operation<T: serde::Serialize>(
    service: &BundleService,
    context: BundleCommandContext<'_>,
    operation: &'static str,
    request: &T,
) -> Result<idempotency::Admission, PortError> {
    idempotency::admit(
        service.database(),
        idempotency::OwnerOperationScope::Tenant(context.tenant_id),
        PRODUCT_BUNDLE_OWNER_SLUG,
        context.idempotency_key,
        operation,
        request,
    )
    .await
}

/// Rolls back the owner mutation before a separate connection finalizes a terminal receipt.
///
/// `idempotency::fail` must run after rollback has completed. Otherwise the independent receipt
/// update can race the transaction rollback or wait behind its locks. On rollback failure the
/// receipt stays processing and reclaimable, so callers receive a retryable storage error instead
/// of an undurable terminal domain outcome.
async fn rollback_bundle_operation(
    txn: DatabaseTransaction,
    context: BundleCommandContext<'_>,
    lease: idempotency::Lease,
) -> Result<(), PortError> {
    txn.rollback().await.map_err(|rollback_error| {
        tracing::error!(
            tenant_id = %context.tenant_id,
            operation_id = %lease.operation_id,
            %rollback_error,
            "failed to roll back Product bundle owner transaction before receipt finalization"
        );
        PortError::unavailable(
            "product_bundle.transaction_rollback_unavailable",
            "product bundle operation could not be completed safely",
        )
    })
}

/// Stores a terminal owner failure so a later retry replays the same typed failure.
async fn finish_failed_bundle_operation(
    service: &BundleService,
    context: BundleCommandContext<'_>,
    lease: idempotency::Lease,
    error: &BundleError,
) -> Result<(), PortError> {
    let port_error = bundle_command_error(error);
    if port_error.retryable {
        return Ok(());
    }
    idempotency::fail(service.database(), lease, &port_error)
        .await
        .map_err(|receipt_error| {
            tracing::error!(
                tenant_id = %context.tenant_id,
                operation_id = %lease.operation_id,
                internal_code = %receipt_error.code,
                "failed to persist Product bundle terminal failure receipt"
            );
            receipt_error
        })
}

/// Maps one Product bundle domain failure onto the owner port error vocabulary.
pub fn bundle_command_error(error: &BundleError) -> PortError {
    match error {
        BundleError::InvalidInput(_) => PortError::validation(
            "product_bundle.invalid_input",
            "product bundle request is invalid",
        ),
        BundleError::SlugAlreadyExists(_) => PortError::conflict(
            "product_bundle.slug_conflict",
            "product bundle slug conflicts with an existing bundle",
        ),
        BundleError::OwnerProductAlreadyHasBundle(_) => PortError::conflict(
            "product_bundle.owner_product_conflict",
            "the product already owns a bundle",
        ),
        BundleError::ConcurrentWriteConflict => PortError::conflict(
            "product_bundle.concurrent_conflict",
            "the product bundle changed concurrently; refresh and try again",
        ),
        BundleError::NotFound(_) => {
            PortError::not_found("product_bundle.not_found", "product bundle was not found")
        }
        BundleError::SlugNotFound(_) => PortError::not_found(
            "product_bundle.slug_not_found",
            "product bundle was not found",
        ),
        BundleError::ItemNotFound(_) => PortError::not_found(
            "product_bundle.item_not_found",
            "product bundle item was not found",
        ),
        BundleError::Database(_) | BundleError::Outbox(_) => PortError::unavailable(
            "product_bundle.storage_unavailable",
            "product bundle storage is temporarily unavailable",
        ),
    }
}

fn decode_bundle_receipt(value: serde_json::Value) -> Result<BundleDto, BundleCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product bundle owner receipt replay"
        );
        PortError::invariant_violation(
            "product_bundle.receipt_corrupt",
            "product bundle operation could not be completed safely",
        )
        .into()
    })
}

fn decode_bundle_item_receipt(
    value: serde_json::Value,
) -> Result<BundleItemDto, BundleCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product bundle item receipt replay"
        );
        PortError::invariant_violation(
            "product_bundle.receipt_corrupt",
            "product bundle item operation could not be completed safely",
        )
        .into()
    })
}

fn decode_unit_receipt(value: serde_json::Value) -> Result<(), BundleCommandError> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            error = %error,
            "failed to decode Product bundle owner receipt replay"
        );
        PortError::invariant_violation(
            "product_bundle.receipt_corrupt",
            "product bundle operation could not be completed safely",
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortErrorKind;

    #[test]
    fn unique_constraint_race_is_a_terminal_conflict() {
        let error = bundle_command_error(&BundleError::ConcurrentWriteConflict);

        assert_eq!(error.kind, PortErrorKind::Conflict);
        assert_eq!(error.code, "product_bundle.concurrent_conflict");
        assert!(!error.retryable);
    }
}

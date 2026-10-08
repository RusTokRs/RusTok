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
//! Bundle creation and bundle-item insertion are `PortCallPolicy::write()` operations: each caller
//! supplies a durable idempotency key, the Product Bundles owner admits one receipt for it and
//! completes that receipt inside the same database transaction as the write, so a retry after a
//! timeout or a double click replays the stored response instead of failing on slug or reference
//! uniqueness. A key is scoped to the tenant and to this owner; reusing one key with another
//! payload is rejected as a conflict.

use rustok_api::PortError;
use rustok_outbox::idempotency;
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{BundleDto, BundleItemDto, BundleItemInput, CreateBundleInput};
use crate::error::BundleError;
use crate::services::bundle_service::{
    BundleService, add_bundle_item_in_tx, create_bundle_in_tx,
};

/// Durable owner identity for Product bundle receipts.
pub const PRODUCT_BUNDLE_OWNER_SLUG: &str = "product_bundles";

/// Receipt operation name of the idempotent bundle creation command.
pub const CREATE_BUNDLE_OPERATION: &str = "create_bundle";

/// Receipt operation name of the idempotent bundle-item insertion command.
pub const ADD_BUNDLE_ITEM_OPERATION: &str = "add_bundle_item";

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
                drop(txn);
                finish_failed_bundle_operation(self, context, lease, &error).await;
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
        let lease = match admit_bundle_operation(
            self,
            context,
            ADD_BUNDLE_ITEM_OPERATION,
            &request,
        )
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
                drop(txn);
                finish_failed_bundle_operation(self, context, lease, &error).await;
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

/// Stores a terminal owner failure so a later retry replays the same typed failure.
async fn finish_failed_bundle_operation(
    service: &BundleService,
    context: BundleCommandContext<'_>,
    lease: idempotency::Lease,
    error: &BundleError,
) {
    let port_error = bundle_command_error(error);
    if port_error.retryable {
        return;
    }
    if let Err(receipt_error) = idempotency::fail(service.database(), lease, &port_error).await {
        tracing::error!(
            tenant_id = %context.tenant_id,
            operation_id = %lease.operation_id,
            internal_code = %receipt_error.code,
            "failed to persist Product bundle terminal failure receipt"
        );
    }
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

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
use sea_orm::TransactionTrait;
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CreateProductRelationInput, ProductRelationDto};
use crate::error::ProductRelationError;
use crate::services::relation_service::{ProductRelationService, create_relation_in_tx};

/// Durable owner identity for Product relation receipts.
pub const PRODUCT_RELATION_OWNER_SLUG: &str = "product_relations";

/// Receipt operation name of the idempotent relation creation command.
pub const CREATE_RELATION_OPERATION: &str = "create_relation";

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
        let admission = idempotency::admit(
            self.database(),
            idempotency::OwnerOperationScope::Tenant(context.tenant_id),
            PRODUCT_RELATION_OWNER_SLUG,
            context.idempotency_key,
            CREATE_RELATION_OPERATION,
            &request,
        )
        .await?;

        let lease = match admission {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_relation_receipt(value),
            idempotency::Admission::ReplayError(error) => return Err(error.into()),
        };

        let txn = self.database().begin().await?;
        match create_relation_in_tx(&txn, context.tenant_id, input).await {
            Ok(relation) => {
                idempotency::complete(&txn, lease, &relation).await?;
                txn.commit().await?;
                Ok(relation)
            }
            Err(error) => {
                drop(txn);
                let port_error = relation_command_error(&error);
                if !port_error.retryable {
                    // A terminal owner failure must stay observable: a later retry of the same key
                    // replays this exact typed failure instead of rerunning the write.
                    if let Err(receipt_error) =
                        idempotency::fail(self.database(), lease, &port_error).await
                    {
                        tracing::error!(
                            tenant_id = %context.tenant_id,
                            operation_id = %lease.operation_id,
                            internal_code = %receipt_error.code,
                            "failed to persist Product relation terminal failure receipt"
                        );
                    }
                }
                Err(error.into())
            }
        }
    }
}

/// Maps one Product relation domain failure onto the owner port error vocabulary.
pub fn relation_command_error(error: &ProductRelationError) -> PortError {
    match error {
        ProductRelationError::SelfRelationNotAllowed(_)
        | ProductRelationError::InvalidInput(_)
        | ProductRelationError::InvalidRelationType(_) => PortError::validation(
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

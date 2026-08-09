use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    graphql::require_module_enabled,
};
use rustok_fulfillment::error::FulfillmentError;
use rustok_payment::{
    AuthorizeAdminPaymentCollectionRequest, CancelAdminPaymentCollectionRequest,
    CancelAdminRefundRequest, CaptureAdminPaymentCollectionRequest, CompleteAdminRefundRequest,
    CreateAdminRefundRequest,
};
use uuid::Uuid;

use crate::graphql_runtime::{
    fulfillment_orchestration_from_context, payment_command_runtime_from_context,
};
use crate::FulfillmentOrchestrationError;

use super::super::{MODULE_SLUG, require_commerce_permission, types::*};
use super::helpers::*;

fn public_provider_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn payment_command_error_envelope(
    error: &PortError,
) -> (&'static str, &'static str, bool, &'static str) {
    match error.code.as_str() {
        "payment.refund_reserved_reconciliation_required"
        | "payment.provider_invalid_response"
        | "payment.provider_outcome_unknown" => (
            "Payment operation requires reconciliation",
            "PAYMENT_RECONCILIATION_REQUIRED",
            false,
            "reconciliation_required",
        ),
        "payment.refund_reserved_provider_unavailable"
        | "payment.provider_unavailable"
        | "payment.database_unavailable" => (
            "Payment service is temporarily unavailable",
            "PAYMENT_TEMPORARILY_UNAVAILABLE",
            true,
            "temporarily_unavailable",
        ),
        "payment.provider_not_configured" => (
            "Payment operation is not configured",
            "PAYMENT_CONFIGURATION_ERROR",
            false,
            "configuration",
        ),
        "payment.provider_rejected" | "payment.invalid_transition" => (
            "Payment operation conflicts with the current state",
            "PAYMENT_STATE_CONFLICT",
            false,
            "state_conflict",
        ),
        _ => match &error.kind {
            PortErrorKind::Validation => (
                "Payment request is invalid",
                "PAYMENT_REQUEST_INVALID",
                false,
                "validation",
            ),
            PortErrorKind::NotFound => (
                "Payment resource was not found",
                "PAYMENT_RESOURCE_NOT_FOUND",
                false,
                "not_found",
            ),
            PortErrorKind::Conflict => (
                "Payment operation conflicts with the current state",
                "PAYMENT_STATE_CONFLICT",
                false,
                "state_conflict",
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Payment service is temporarily unavailable",
                "PAYMENT_TEMPORARILY_UNAVAILABLE",
                true,
                "temporarily_unavailable",
            ),
            PortErrorKind::InvariantViolation => (
                "Payment operation requires reconciliation",
                "PAYMENT_RECONCILIATION_REQUIRED",
                false,
                "reconciliation_required",
            ),
            PortErrorKind::Forbidden => (
                "Payment request is invalid",
                "PAYMENT_REQUEST_INVALID",
                false,
                "forbidden",
            ),
        },
    }
}

fn fulfillment_error_envelope(error: &FulfillmentError) -> (&'static str, &'static str, bool) {
    match error {
        FulfillmentError::Validation(_) => (
            "Fulfillment request is invalid",
            "FULFILLMENT_REQUEST_INVALID",
            false,
        ),
        FulfillmentError::ShippingOptionNotFound(_) | FulfillmentError::FulfillmentNotFound(_) => (
            "Fulfillment resource was not found",
            "FULFILLMENT_RESOURCE_NOT_FOUND",
            false,
        ),
        FulfillmentError::InvalidTransition { .. } => (
            "Fulfillment operation conflicts with the current state",
            "FULFILLMENT_STATE_CONFLICT",
            false,
        ),
        FulfillmentError::Database(_) => (
            "Fulfillment service is temporarily unavailable",
            "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
    }
}

fn fulfillment_orchestration_error_envelope(
    error: &FulfillmentOrchestrationError,
) -> (&'static str, &'static str, bool) {
    match error {
        FulfillmentOrchestrationError::OrderNotFound(_) => (
            "Order resource was not found",
            "ORDER_RESOURCE_NOT_FOUND",
            false,
        ),
        FulfillmentOrchestrationError::Database(_) => (
            "Fulfillment service is temporarily unavailable",
            "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        FulfillmentOrchestrationError::Fulfillment(source) => fulfillment_error_envelope(source),
        FulfillmentOrchestrationError::Validation(_) => (
            "Fulfillment request is invalid",
            "FULFILLMENT_REQUEST_INVALID",
            false,
        ),
        FulfillmentOrchestrationError::ProviderAfterPersistence { .. }
        | FulfillmentOrchestrationError::PersistenceAfterProvider { .. } => (
            "Fulfillment operation requires reconciliation",
            "FULFILLMENT_RECONCILIATION_REQUIRED",
            false,
        ),
    }
}

fn payment_provider_graphql_error(
    tenant_id: Uuid,
    resource_id: Uuid,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> async_graphql::Error {
    let (message, code, retryable, error_kind) = payment_command_error_envelope(&error);
    tracing::error!(
        owner = "rustok_payment",
        tenant_id_non_nil = !tenant_id.is_nil(),
        resource_id_non_nil = !resource_id.is_nil(),
        operation,
        correlation_id = %context.correlation_id,
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        error_kind,
        public_code = code,
        retryable,
        boundary = "commerce_graphql_payment_command",
        "commerce GraphQL payment owner command failed"
    );
    public_provider_graphql_error(message, code, retryable)
}

fn fulfillment_provider_graphql_error(
    tenant_id: Uuid,
    resource_id: Uuid,
    operation: &'static str,
    error: FulfillmentOrchestrationError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        resource_id = %resource_id,
        operation,
        "commerce GraphQL fulfillment provider operation failed"
    );
    let (message, code, retryable) = fulfillment_orchestration_error_envelope(&error);
    public_provider_graphql_error(message, code, retryable)
}

fn payment_collection_command_context(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    collection_id: Uuid,
    operation: &'static str,
) -> Result<PortContext> {
    let auth = ctx.data::<AuthContext>()?;
    let request = ctx.data_opt::<RequestContext>();
    let locale = request
        .map(|request| request.locale.as_str())
        .filter(|locale| !locale.trim().is_empty())
        .unwrap_or("und");
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("commerce-graphql-payment-command:{operation}:{collection_id}"),
    )
    .with_idempotency_key(format!(
        "graphql-payment-collection:{collection_id}:{operation}"
    ))
    .with_deadline(std::time::Duration::from_secs(2));
    Ok(match request.and_then(|request| request.channel_slug.as_deref()) {
        Some(channel) => context.with_channel(channel),
        None => context,
    })
}

fn payment_refund_create_context(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    collection_id: Uuid,
    creation_key: &str,
) -> Result<PortContext> {
    let auth = ctx.data::<AuthContext>()?;
    let request = ctx.data_opt::<RequestContext>();
    let locale = request
        .map(|request| request.locale.as_str())
        .filter(|locale| !locale.trim().is_empty())
        .unwrap_or("und");
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("commerce-graphql-payment-command:create_refund:{collection_id}"),
    )
    .with_idempotency_key(creation_key.to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    Ok(match request.and_then(|request| request.channel_slug.as_deref()) {
        Some(channel) => context.with_channel(channel),
        None => context,
    })
}

fn payment_refund_transition_context(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    refund_id: Uuid,
    operation: &'static str,
) -> Result<PortContext> {
    let auth = ctx.data::<AuthContext>()?;
    let request = ctx.data_opt::<RequestContext>();
    let locale = request
        .map(|request| request.locale.as_str())
        .filter(|locale| !locale.trim().is_empty())
        .unwrap_or("und");
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("commerce-graphql-payment-command:{operation}:{refund_id}"),
    )
    .with_idempotency_key(format!("graphql-refund:{refund_id}:{operation}"))
    .with_deadline(std::time::Duration::from_secs(2));
    Ok(match request.and_then(|request| request.channel_slug.as_deref()) {
        Some(channel) => context.with_channel(channel),
        None => context,
    })
}

#[derive(Default)]
pub struct CommerceProviderMutation;

#[Object]
impl CommerceProviderMutation {
    async fn authorize_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: AuthorizePaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context =
            payment_collection_command_context(ctx, tenant_id, id, "authorize_payment_collection")?;
        let collection = runtime
            .collection_command_port()
            .authorize_payment_collection(
                context.clone(),
                AuthorizeAdminPaymentCollectionRequest {
                    collection_id: id,
                    input: crate::dto::AuthorizePaymentInput {
                        provider_id: input.provider_id,
                        provider_payment_id: input.provider_payment_id,
                        amount: parse_optional_decimal(input.amount.as_deref())?,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    id,
                    "authorize_payment_collection",
                    &context,
                    error,
                )
            })?;
        Ok(collection.into())
    }

    async fn capture_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CapturePaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context =
            payment_collection_command_context(ctx, tenant_id, id, "capture_payment_collection")?;
        let collection = runtime
            .collection_command_port()
            .capture_payment_collection(
                context.clone(),
                CaptureAdminPaymentCollectionRequest {
                    collection_id: id,
                    input: crate::dto::CapturePaymentInput {
                        amount: parse_optional_decimal(input.amount.as_deref())?,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    id,
                    "capture_payment_collection",
                    &context,
                    error,
                )
            })?;
        Ok(collection.into())
    }

    async fn cancel_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CancelPaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context =
            payment_collection_command_context(ctx, tenant_id, id, "cancel_payment_collection")?;
        let collection = runtime
            .collection_command_port()
            .cancel_payment_collection(
                context.clone(),
                CancelAdminPaymentCollectionRequest {
                    collection_id: id,
                    input: crate::dto::CancelPaymentInput {
                        reason: input.reason,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    id,
                    "cancel_payment_collection",
                    &context,
                    error,
                )
            })?;
        Ok(collection.into())
    }

    async fn create_refund(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        payment_collection_id: Uuid,
        idempotency_key: String,
        input: CreateRefundInputObject,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context = payment_refund_create_context(
            ctx,
            tenant_id,
            payment_collection_id,
            idempotency_key.as_str(),
        )?;
        let refund = runtime
            .refund_command_port()
            .create_refund(
                context.clone(),
                CreateAdminRefundRequest {
                    collection_id: payment_collection_id,
                    creation_key: idempotency_key,
                    input: crate::dto::CreateRefundInput {
                        amount: parse_decimal(&input.amount)?,
                        reason: input.reason,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    payment_collection_id,
                    "create_refund",
                    &context,
                    error,
                )
            })?;
        Ok(refund.into())
    }

    async fn complete_refund(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CompleteRefundInputObject,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context = payment_refund_transition_context(ctx, tenant_id, id, "complete_refund")?;
        let refund = runtime
            .refund_command_port()
            .complete_refund(
                context.clone(),
                CompleteAdminRefundRequest {
                    refund_id: id,
                    input: crate::dto::CompleteRefundInput {
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    id,
                    "complete_refund",
                    &context,
                    error,
                )
            })?;
        Ok(refund.into())
    }

    async fn cancel_refund(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CancelRefundInputObject,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let runtime = payment_command_runtime_from_context(ctx, db.clone());
        let context = payment_refund_transition_context(ctx, tenant_id, id, "cancel_refund")?;
        let refund = runtime
            .refund_command_port()
            .cancel_refund(
                context.clone(),
                CancelAdminRefundRequest {
                    refund_id: id,
                    input: crate::dto::CancelRefundInput {
                        reason: input.reason,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                },
            )
            .await
            .map_err(|error| {
                payment_provider_graphql_error(
                    tenant_id,
                    id,
                    "cancel_refund",
                    &context,
                    error,
                )
            })?;
        Ok(refund.into())
    }

    async fn create_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_CREATE],
            "Permission denied: fulfillments:create required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let order_id = input.order_id;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .create_manual_fulfillment(
                tenant_id,
                crate::dto::CreateFulfillmentInput {
                    order_id,
                    shipping_option_id: input.shipping_option_id,
                    customer_id: input.customer_id,
                    carrier: input.carrier,
                    tracking_number: input.tracking_number,
                    items: Some(
                        input
                            .items
                            .into_iter()
                            .map(|item| {
                                Ok(crate::dto::CreateFulfillmentItemInput {
                                    order_line_item_id: item.order_line_item_id,
                                    quantity: item.quantity,
                                    metadata: parse_optional_metadata(item.metadata.as_deref())?,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                    ),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, order_id, "create_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }

    async fn ship_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: ShipFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .ship_fulfillment(
                tenant_id,
                id,
                crate::dto::ShipFulfillmentInput {
                    carrier: input.carrier,
                    tracking_number: input.tracking_number,
                    items: input.items.map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::dto::FulfillmentItemQuantityInput {
                                fulfillment_item_id: item.fulfillment_item_id,
                                quantity: item.quantity,
                            })
                            .collect()
                    }),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, id, "ship_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }

    async fn deliver_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: DeliverFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .deliver_fulfillment(
                tenant_id,
                id,
                crate::dto::DeliverFulfillmentInput {
                    delivered_note: input.delivered_note,
                    items: input.items.map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::dto::FulfillmentItemQuantityInput {
                                fulfillment_item_id: item.fulfillment_item_id,
                                quantity: item.quantity,
                            })
                            .collect()
                    }),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, id, "deliver_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }

    async fn reopen_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: ReopenFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .reopen_fulfillment(
                tenant_id,
                id,
                crate::dto::ReopenFulfillmentInput {
                    items: input.items.map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::dto::FulfillmentItemQuantityInput {
                                fulfillment_item_id: item.fulfillment_item_id,
                                quantity: item.quantity,
                            })
                            .collect()
                    }),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, id, "reopen_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }

    async fn reship_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: ReshipFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .reship_fulfillment(
                tenant_id,
                id,
                crate::dto::ReshipFulfillmentInput {
                    carrier: input.carrier,
                    tracking_number: input.tracking_number,
                    items: input.items.map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::dto::FulfillmentItemQuantityInput {
                                fulfillment_item_id: item.fulfillment_item_id,
                                quantity: item.quantity,
                            })
                            .collect()
                    }),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, id, "reship_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }

    async fn cancel_fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CancelFulfillmentInputObject,
    ) -> Result<GqlFulfillment> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
            .cancel_fulfillment(
                tenant_id,
                id,
                crate::dto::CancelFulfillmentInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_provider_graphql_error(tenant_id, id, "cancel_fulfillment", error)
            })?;
        Ok(fulfillment.into())
    }
}

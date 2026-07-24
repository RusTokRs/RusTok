use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::Permission;
use rustok_api::{
    AuthContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_order::{OrderError, OrderService};
use rustok_payment::error::PaymentError;
use uuid::Uuid;

use crate::{PaymentOrchestrationError, PostOrderOrchestrationError};
use crate::graphql_runtime::{
    order_change_orchestration_from_context, post_order_orchestration_from_context,
    return_completion_orchestration_from_context,
};

use super::super::{MODULE_SLUG, current_tenant_scope, require_commerce_permission, types::*};
use super::helpers::*;

fn public_fulfillment_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn order_error_envelope(error: &OrderError) -> (&'static str, &'static str, bool) {
    match error {
        OrderError::Validation(_) => ("Order request is invalid", "ORDER_REQUEST_INVALID", false),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            "Order resource was not found",
            "ORDER_RESOURCE_NOT_FOUND",
            false,
        ),
        OrderError::InvalidTransition { .. } => (
            "Order operation conflicts with the current state",
            "ORDER_STATE_CONFLICT",
            false,
        ),
        OrderError::Database(_) => (
            "Order service is temporarily unavailable",
            "ORDER_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        OrderError::Core(_) => (
            "Order operation could not be completed safely",
            "ORDER_OPERATION_FAILED",
            false,
        ),
    }
}

fn payment_error_envelope(error: &PaymentError) -> (&'static str, &'static str, bool) {
    match error {
        PaymentError::Validation(_) => (
            "Payment request is invalid",
            "PAYMENT_REQUEST_INVALID",
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            "Payment resource was not found",
            "PAYMENT_RESOURCE_NOT_FOUND",
            false,
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            "Payment operation conflicts with the current state",
            "PAYMENT_STATE_CONFLICT",
            false,
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::Database(_) => (
            "Payment service is temporarily unavailable",
            "PAYMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            "Payment operation requires reconciliation",
            "PAYMENT_RECONCILIATION_REQUIRED",
            false,
        ),
        PaymentError::ProviderConfiguration { .. } => (
            "Payment operation is not configured",
            "PAYMENT_CONFIGURATION_ERROR",
            false,
        ),
    }
}

fn payment_orchestration_error_envelope(
    error: &PaymentOrchestrationError,
) -> (&'static str, &'static str, bool) {
    match error {
        PaymentOrchestrationError::Provider(source)
        | PaymentOrchestrationError::Payment(source) => payment_error_envelope(source),
        PaymentOrchestrationError::ProviderAfterRefundReservation { .. } => (
            "Payment operation requires reconciliation",
            "PAYMENT_RECONCILIATION_REQUIRED",
            false,
        ),
    }
}

fn order_mutation_graphql_error(
    tenant_id: Uuid,
    resource_id: Uuid,
    operation: &'static str,
    error: OrderError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        resource_id = %resource_id,
        operation,
        "commerce GraphQL fulfillment order mutation failed"
    );
    let (message, code, retryable) = order_error_envelope(&error);
    public_fulfillment_graphql_error(message, code, retryable)
}

fn post_order_graphql_error(
    tenant_id: Uuid,
    resource_id: Uuid,
    operation: &'static str,
    error: PostOrderOrchestrationError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        resource_id = %resource_id,
        operation,
        "commerce GraphQL post-order orchestration failed"
    );

    let (message, code, retryable) = match &error {
        PostOrderOrchestrationError::Order(source) => order_error_envelope(source),
        PostOrderOrchestrationError::Payment(source) => payment_error_envelope(source),
        PostOrderOrchestrationError::PaymentOrchestration(source) => {
            payment_orchestration_error_envelope(source)
        }
        PostOrderOrchestrationError::Validation(_) => (
            "Post-order request is invalid",
            "POST_ORDER_REQUEST_INVALID",
            false,
        ),
    };
    public_fulfillment_graphql_error(message, code, retryable)
}

#[derive(Default)]
pub struct CommerceFulfillmentMutation;

#[Object]
impl CommerceFulfillmentMutation {
    async fn create_storefront_order_return(
        &self,
        ctx: &Context<'_>,
        order_id: Uuid,
        tenant_id: Option<Uuid>,
        input: CreateOrderReturnInputObject,
    ) -> Result<GqlOrderReturn> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Storefront return creation")?;

        ensure_storefront_order_access(db, event_bus, tenant_id, ctx, order_id).await?;

        let item = OrderService::new(db.clone(), event_bus.clone())
            .create_return(tenant_id, order_id, build_create_order_return_input(input)?)
            .await
            .map_err(|error| {
                order_mutation_graphql_error(
                    tenant_id,
                    order_id,
                    "create_storefront_order_return",
                    error,
                )
            })?;

        Ok(item.into())
    }

    async fn mark_order_paid(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: MarkPaidOrderInput,
    ) -> Result<GqlOrder> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Mark order paid")?;
        require_current_actor(&auth, user_id)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .mark_paid(
                tenant_id,
                auth.user_id,
                id,
                input.payment_id,
                input.payment_method,
            )
            .await?;

        Ok(order.into())
    }

    async fn ship_order(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: ShipOrderInput,
    ) -> Result<GqlOrder> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Ship order")?;
        require_current_actor(&auth, user_id)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .ship_order(
                tenant_id,
                auth.user_id,
                id,
                input.tracking_number,
                input.carrier,
            )
            .await?;

        Ok(order.into())
    }

    async fn deliver_order(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: DeliverOrderInput,
    ) -> Result<GqlOrder> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Deliver order")?;
        require_current_actor(&auth, user_id)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .deliver_order(tenant_id, auth.user_id, id, input.delivered_signature)
            .await?;

        Ok(order.into())
    }

    async fn cancel_order(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: CancelOrderInput,
    ) -> Result<GqlOrder> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Cancel order")?;
        require_current_actor(&auth, user_id)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .cancel_order(tenant_id, auth.user_id, id, input.reason)
            .await?;

        Ok(order.into())
    }

    async fn create_order_change(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        order_id: Uuid,
        input: CreateOrderChangeInputObject,
    ) -> Result<GqlOrderChange> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Create order change")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let item = OrderService::new(db.clone(), event_bus.clone())
            .create_order_change(
                tenant_id,
                auth.user_id,
                order_id,
                build_create_order_change_input(input)?,
            )
            .await?;

        Ok(item.into())
    }

    async fn apply_order_change(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: ApplyOrderChangeInputObject,
    ) -> Result<GqlOrderChange> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Apply order change")?;

        let difference_refund = input
            .difference_refund
            .map(|diff| {
                Ok::<_, async_graphql::Error>(crate::ExchangeDifferenceRefundInput {
                    amount: parse_decimal(&diff.amount)?,
                    reason: diff.reason,
                    metadata: parse_optional_metadata(diff.metadata.as_deref())?,
                })
            })
            .transpose()?;
        let metadata = parse_optional_metadata(input.metadata.as_deref())?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let result = order_change_orchestration_from_context(ctx, db.clone(), event_bus.clone())
            .apply_order_change(tenant_id, id, difference_refund, metadata)
            .await
            .map_err(|error| {
                post_order_graphql_error(tenant_id, id, "apply_order_change", error)
            })?;

        Ok(result.order_change.into())
    }

    async fn cancel_order_change(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CancelOrderChangeInputObject,
    ) -> Result<GqlOrderChange> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Cancel order change")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let item = OrderService::new(db.clone(), event_bus.clone())
            .cancel_order_change(
                tenant_id,
                id,
                crate::dto::CancelOrderChangeInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(item.into())
    }

    async fn create_order_return(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        order_id: Uuid,
        input: CreateOrderReturnInputObject,
    ) -> Result<GqlOrderReturn> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Create order return")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let item = OrderService::new(db.clone(), event_bus.clone())
            .create_return(tenant_id, order_id, build_create_order_return_input(input)?)
            .await?;

        Ok(item.into())
    }

    async fn create_order_return_decision(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        order_id: Uuid,
        input: CreateReturnDecisionInputObject,
    ) -> Result<GqlReturnDecision> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Create return decision")?;

        if graphql_decision_requires_payments_update(
            input.decision.action.as_str(),
            input.decision.refund.is_some(),
        ) {
            require_commerce_permission(
                ctx,
                &[Permission::PAYMENTS_UPDATE],
                "Permission denied: payments:update required",
            )?;
        }

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let decision = post_order_orchestration_from_context(ctx, db.clone(), event_bus.clone())
            .create_return_decision(
                tenant_id,
                auth.user_id,
                order_id,
                build_create_return_decision_input(input)?,
            )
            .await
            .map_err(|error| {
                post_order_graphql_error(
                    tenant_id,
                    order_id,
                    "create_order_return_decision",
                    error,
                )
            })?;

        Ok(decision.into())
    }

    async fn complete_order_return(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CompleteOrderReturnInputObject,
    ) -> Result<GqlOrderReturn> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Complete order return")?;

        if input.refund.is_some() {
            require_commerce_permission(
                ctx,
                &[Permission::PAYMENTS_UPDATE],
                "Permission denied: payments:update required",
            )?;
        }

        let command = crate::CompleteReturnResolutionInput {
            resolution_type: input.resolution_type,
            refund_id: input.refund_id,
            order_change_id: input.order_change_id,
            refund: input
                .refund
                .map(|refund| {
                    Ok::<_, async_graphql::Error>(crate::CompleteReturnRefundInput {
                        payment_collection_id: refund.payment_collection_id,
                        amount: parse_decimal(&refund.amount)?,
                        reason: refund.reason,
                        metadata: parse_optional_metadata(refund.metadata.as_deref())?,
                        complete: refund.complete.unwrap_or(false),
                    })
                })
                .transpose()?,
            exchange: input
                .exchange
                .map(|exchange| {
                    Ok::<_, async_graphql::Error>(crate::CompleteReturnExchangeInput {
                        description: exchange.description,
                        preview: parse_json_payload(
                            exchange.preview.as_str(),
                            "Invalid JSON preview payload",
                        )?,
                        metadata: parse_optional_metadata(exchange.metadata.as_deref())?,
                    })
                })
                .transpose()?,
            claim: input
                .claim
                .map(|claim| {
                    Ok::<_, async_graphql::Error>(crate::CompleteReturnClaimInput {
                        description: claim.description,
                        preview: parse_json_payload(
                            claim.preview.as_str(),
                            "Invalid JSON preview payload",
                        )?,
                        metadata: parse_optional_metadata(claim.metadata.as_deref())?,
                    })
                })
                .transpose()?,
            metadata: parse_optional_metadata(input.metadata.as_deref())?,
        };

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let item = return_completion_orchestration_from_context(ctx, db.clone(), event_bus.clone())
            .complete_return(tenant_id, auth.user_id, id, command)
            .await
            .map_err(|error| {
                post_order_graphql_error(tenant_id, id, "complete_order_return", error)
            })?;

        Ok(item.into())
    }

    async fn cancel_order_return(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: CancelOrderReturnInputObject,
    ) -> Result<GqlOrderReturn> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Cancel order return")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let item = OrderService::new(db.clone(), event_bus.clone())
            .cancel_return(
                tenant_id,
                id,
                crate::dto::CancelOrderReturnInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(item.into())
    }
}

fn require_current_actor(auth: &AuthContext, requested_user_id: Uuid) -> Result<()> {
    if requested_user_id != auth.user_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Mutation actor must match the authenticated user",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::require_current_actor;
    use rustok_api::AuthContext;
    use uuid::Uuid;

    fn auth(user_id: Uuid) -> AuthContext {
        AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: Vec::new(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn caller_cannot_spoof_order_mutation_actor() {
        let user_id = Uuid::new_v4();
        assert!(require_current_actor(&auth(user_id), user_id).is_ok());
        assert!(require_current_actor(&auth(user_id), Uuid::new_v4()).is_err());
    }
}

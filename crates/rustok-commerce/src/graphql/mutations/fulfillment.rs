use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::{graphql::require_module_enabled, AuthContext, TenantContext};
use rustok_order::OrderService;
use std::str::FromStr;
use uuid::Uuid;

use crate::graphql_runtime::post_order_orchestration_from_context;
use crate::ExchangeDifferenceRefundInput;

use super::super::{require_commerce_permission, types::*, MODULE_SLUG};
use super::helpers::*;

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
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        ensure_storefront_order_access(db, event_bus, tenant_id, ctx, order_id).await?;

        let item = OrderService::new(db.clone(), event_bus.clone())
            .create_return(tenant_id, order_id, build_create_order_return_input(input)?)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

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
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .mark_paid(
                tenant_id,
                user_id,
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
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .ship_order(tenant_id, user_id, id, input.tracking_number, input.carrier)
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
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .deliver_order(tenant_id, user_id, id, input.delivered_signature)
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
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order = OrderService::new(db.clone(), event_bus.clone())
            .cancel_order(tenant_id, user_id, id, input.reason)
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
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let auth = ctx.data::<AuthContext>()?;
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

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order_service = OrderService::new(db.clone(), event_bus.clone());
        let orchestration_service =
            post_order_orchestration_from_context(ctx, db.clone(), event_bus.clone());

        // Fetch the order change to inspect its change_type
        let order_change = order_service.get_order_change(tenant_id, id).await?;

        let result = match order_change.change_type.as_str() {
            "exchange" => {
                let difference_refund = if let Some(diff) = input.difference_refund {
                    let amount = Decimal::from_str(&diff.amount).map_err(|e| {
                        FieldError::new(format!("invalid difference refund amount: {e}"))
                    })?;
                    let metadata = parse_optional_metadata(diff.metadata.as_deref())?;
                    Some(ExchangeDifferenceRefundInput {
                        amount,
                        reason: diff.reason,
                        metadata,
                    })
                } else {
                    None
                };
                let metadata = parse_optional_metadata(input.metadata.as_deref())?;
                orchestration_service
                    .apply_exchange_order_change(
                        tenant_id,
                        order_change.order_id,
                        id,
                        difference_refund,
                        metadata,
                    )
                    .await
                    .map_err(|err| FieldError::new(err.to_string()))?
                    .order_change
            }
            "claim" => {
                let metadata = parse_optional_metadata(input.metadata.as_deref())?;
                orchestration_service
                    .apply_claim_order_change(tenant_id, id, metadata)
                    .await
                    .map_err(|err| FieldError::new(err.to_string()))?
                    .order_change
            }
            _ => {
                order_service
                    .apply_order_change(
                        tenant_id,
                        id,
                        crate::dto::ApplyOrderChangeInput {
                            metadata: parse_optional_metadata(input.metadata.as_deref())?,
                        },
                    )
                    .await?
            }
        };

        Ok(result.into())
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
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

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

        if input.refund.is_some() {
            require_commerce_permission(
                ctx,
                &[Permission::PAYMENTS_UPDATE],
                "Permission denied: payments:update required",
            )?;
        }

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order_service = OrderService::new(db.clone(), event_bus.clone());
        let mut complete_input = crate::dto::CompleteOrderReturnInput {
            resolution_type: input.resolution_type,
            refund_id: input.refund_id,
            order_change_id: input.order_change_id,
            metadata: parse_optional_metadata(input.metadata.as_deref())?,
        };

        if let Some(refund_input) = input.refund {
            complete_input = build_refund_resolution_return_completion(
                ctx,
                db,
                &order_service,
                tenant_id,
                id,
                complete_input,
                refund_input,
            )
            .await?;
        }

        if let Some(exchange_input) = input.exchange {
            complete_input = build_exchange_resolution_return_completion(
                &order_service,
                tenant_id,
                auth.user_id,
                id,
                complete_input,
                exchange_input,
            )
            .await?;
        }

        if let Some(claim_input) = input.claim {
            complete_input = build_claim_resolution_return_completion(
                &order_service,
                tenant_id,
                auth.user_id,
                id,
                complete_input,
                claim_input,
            )
            .await?;
        }

        let item = order_service
            .complete_return(tenant_id, id, complete_input)
            .await?;

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

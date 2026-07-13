use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext,
};
use rustok_order::OrderService;
use std::str::FromStr;
use uuid::Uuid;

use crate::graphql_runtime::post_order_orchestration_from_context;
use crate::ExchangeDifferenceRefundInput;

use super::super::{
    current_tenant_scope, require_commerce_permission, types::*, MODULE_SLUG,
};
use super::helpers::*;
use super::provider_return_helpers::build_provider_refund_resolution_return_completion;

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

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let order_service = OrderService::new(db.clone(), event_bus.clone());
        let orchestration_service =
            post_order_orchestration_from_context(ctx, db.clone(), event_bus.clone());

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Complete order return")?;

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
            complete_input = build_provider_refund_resolution_return_completion(
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
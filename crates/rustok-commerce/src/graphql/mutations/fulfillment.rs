use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::{graphql::require_module_enabled, AuthContext, TenantContext};
use rustok_core::Permission;
use uuid::Uuid;

use crate::{
    ExchangeDifferenceRefundInput, FulfillmentOrchestrationService, FulfillmentService,
    OrderService, PaymentService, PostOrderOrchestrationService,
};

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
            PostOrderOrchestrationService::new(db.clone(), event_bus.clone());

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
        let decision = PostOrderOrchestrationService::new(db.clone(), event_bus.clone())
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
        let collection = PaymentService::new(db.clone())
            .authorize_collection(
                tenant_id,
                id,
                crate::dto::AuthorizePaymentInput {
                    provider_id: input.provider_id,
                    provider_payment_id: input.provider_payment_id,
                    amount: parse_optional_decimal(input.amount.as_deref())?,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

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
        let collection = PaymentService::new(db.clone())
            .capture_collection(
                tenant_id,
                id,
                crate::dto::CapturePaymentInput {
                    amount: parse_optional_decimal(input.amount.as_deref())?,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

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
        let collection = crate::PaymentOrchestrationService::new(db.clone())
            .cancel_collection(
                tenant_id,
                id,
                crate::dto::CancelPaymentInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(collection.into())
    }

    async fn create_refund(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        payment_collection_id: Uuid,
        input: CreateRefundInputObject,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let refund = crate::PaymentOrchestrationService::new(db.clone())
            .create_refund(
                tenant_id,
                payment_collection_id,
                crate::dto::CreateRefundInput {
                    amount: parse_decimal(&input.amount)?,
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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
        let refund = PaymentService::new(db.clone())
            .complete_refund(
                tenant_id,
                id,
                crate::dto::CompleteRefundInput {
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

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
        let refund = PaymentService::new(db.clone())
            .cancel_refund(
                tenant_id,
                id,
                crate::dto::CancelRefundInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

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
        let fulfillment = FulfillmentOrchestrationService::new(db.clone())
            .create_manual_fulfillment(
                tenant_id,
                crate::dto::CreateFulfillmentInput {
                    order_id: input.order_id,
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
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

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
        let fulfillment = FulfillmentService::new(db.clone())
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
            .await?;

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
        let fulfillment = FulfillmentService::new(db.clone())
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
            .await?;

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
        let fulfillment = FulfillmentService::new(db.clone())
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
            .await?;

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
        let fulfillment = FulfillmentService::new(db.clone())
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
            .await?;

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
        let fulfillment = FulfillmentService::new(db.clone())
            .cancel_fulfillment(
                tenant_id,
                id,
                crate::dto::CancelFulfillmentInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(fulfillment.into())
    }
}

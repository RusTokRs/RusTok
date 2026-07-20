use async_graphql::{Context, Object, Result};
use rustok_api::{Permission, graphql::require_module_enabled};
use uuid::Uuid;

use crate::graphql_runtime::{
    fulfillment_orchestration_from_context, payment_orchestration_from_context,
};

use super::super::{MODULE_SLUG, require_commerce_permission, types::*};
use super::helpers::*;

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
        let collection = payment_orchestration_from_context(ctx, db.clone())
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
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
        let collection = payment_orchestration_from_context(ctx, db.clone())
            .capture_collection(
                tenant_id,
                id,
                crate::dto::CapturePaymentInput {
                    amount: parse_optional_decimal(input.amount.as_deref())?,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
        let collection = payment_orchestration_from_context(ctx, db.clone())
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
        let refund = payment_orchestration_from_context(ctx, db.clone())
            .create_refund_idempotent(
                tenant_id,
                payment_collection_id,
                idempotency_key,
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
        let refund = payment_orchestration_from_context(ctx, db.clone())
            .complete_refund(
                tenant_id,
                id,
                crate::dto::CompleteRefundInput {
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
        let refund = payment_orchestration_from_context(ctx, db.clone())
            .cancel_refund(
                tenant_id,
                id,
                crate::dto::CancelRefundInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
        let fulfillment = fulfillment_orchestration_from_context(ctx, db.clone())
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(fulfillment.into())
    }
}

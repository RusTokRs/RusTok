use async_graphql::{Context, Object, Result};
use rust_decimal::Decimal;
use rustok_api::graphql::require_module_enabled;
use rustok_core::Permission;
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::{CatalogService, FulfillmentService, OrderService, PaymentService};

use super::{require_commerce_permission, types::*, MODULE_SLUG};

#[derive(Default)]
pub struct CommerceMutation;

#[Object]
impl CommerceMutation {
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
            .ship_order(
                tenant_id,
                user_id,
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
        let collection = PaymentService::new(db.clone())
            .cancel_collection(
                tenant_id,
                id,
                crate::dto::CancelPaymentInput {
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(collection.into())
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

    async fn create_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        input: CreateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_CREATE],
            "Permission denied: products:create required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let domain_input = convert_create_product_input(input)?;
        let product = catalog
            .create_product(tenant_id, user_id, domain_input)
            .await?;

        Ok(product.into())
    }

    async fn update_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: UpdateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let domain_input = crate::dto::UpdateProductInput {
            translations: input.translations.map(|translations| {
                translations
                    .into_iter()
                    .map(|translation| crate::dto::ProductTranslationInput {
                        locale: translation.locale,
                        title: translation.title,
                        handle: translation.handle,
                        description: translation.description,
                        meta_title: translation.meta_title,
                        meta_description: translation.meta_description,
                    })
                    .collect()
            }),
            vendor: input.vendor,
            product_type: input.product_type,
            metadata: None,
            status: input.status.map(Into::into),
        };

        let product = catalog
            .update_product(tenant_id, user_id, id, domain_input)
            .await?;

        Ok(product.into())
    }

    async fn publish_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let product = catalog.publish_product(tenant_id, user_id, id).await?;

        Ok(product.into())
    }

    async fn delete_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_DELETE],
            "Permission denied: products:delete required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        catalog.delete_product(tenant_id, user_id, id).await?;

        Ok(true)
    }
}

fn convert_create_product_input(
    input: CreateProductInput,
) -> Result<crate::dto::CreateProductInput> {
    let translations = input
        .translations
        .into_iter()
        .map(|translation| crate::dto::ProductTranslationInput {
            locale: translation.locale,
            title: translation.title,
            handle: translation.handle,
            description: translation.description,
            meta_title: translation.meta_title,
            meta_description: translation.meta_description,
        })
        .collect();

    let options = input
        .options
        .unwrap_or_default()
        .into_iter()
        .map(|option| crate::dto::ProductOptionInput {
            name: option.name,
            values: option.values,
        })
        .collect();

    let variants = input
        .variants
        .into_iter()
        .map(|variant| {
            let prices = variant
                .prices
                .into_iter()
                .map(|price| {
                    let amount = parse_decimal(&price.amount)?;
                    let compare_at_amount = match price.compare_at_amount {
                        Some(value) => Some(parse_decimal(&value)?),
                        None => None,
                    };

                    Ok(crate::dto::PriceInput {
                        currency_code: price.currency_code,
                        amount,
                        compare_at_amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(crate::dto::CreateVariantInput {
                sku: variant.sku,
                barcode: variant.barcode,
                option1: variant.option1,
                option2: variant.option2,
                option3: variant.option3,
                prices,
                inventory_quantity: variant.inventory_quantity.unwrap_or(0),
                inventory_policy: variant
                    .inventory_policy
                    .unwrap_or_else(|| "deny".to_string()),
                weight: None,
                weight_unit: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(crate::dto::CreateProductInput {
        translations,
        options,
        variants,
        vendor: input.vendor,
        product_type: input.product_type,
        metadata: serde_json::Value::Object(Default::default()),
        publish: input.publish.unwrap_or(false),
    })
}

fn parse_decimal(value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|_| async_graphql::Error::new("Invalid decimal value"))
}

fn parse_optional_decimal(value: Option<&str>) -> Result<Option<Decimal>> {
    value.map(parse_decimal).transpose()
}

fn parse_optional_metadata(value: Option<&str>) -> Result<Value> {
    match value.map(str::trim) {
        None | Some("") => Ok(Value::Object(Default::default())),
        Some(value) => serde_json::from_str(value)
            .map_err(|_| async_graphql::Error::new("Invalid JSON metadata payload")),
    }
}

use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext, RequestContext, TenantContext,
};
use rustok_core::{locale_tags_match, Permission};
use rustok_inventory::check_variant_availability_for_public_channel;
use rustok_pricing::PriceResolutionContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    entities::{price_list, product, product_translation, product_variant, variant_translation},
    storefront_channel::{is_metadata_visible_for_public_channel, normalize_public_channel_slug},
    storefront_shipping::{
        effective_shipping_profile_slug, enrich_cart_delivery_groups,
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
    CartService, CatalogService, CheckoutService, CreateReturnDecisionInput, CustomerService,
    ExchangeDifferenceRefundInput, FulfillmentOrchestrationService, FulfillmentService,
    OrderService, PaymentService, PostOrderOrchestrationService,
    PricingService, ReturnClaimDecisionInput, ReturnDecisionInput, ReturnExchangeDecisionInput,
    ReturnRefundDecisionInput, ShippingProfileService, StoreContextService,
};

use super::helpers::*;
use super::super::{require_commerce_permission, types::*, MODULE_SLUG};

#[derive(Default)]
pub struct CommerceCatalogMutation;

#[Object]
impl CommerceCatalogMutation {
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
        validate_product_shipping_profile_input(
            db,
            tenant_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;
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
        validate_product_shipping_profile_input(
            db,
            tenant_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;
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
            seller_id: input.seller_id,
            vendor: input.vendor,
            product_type: input.product_type,
            shipping_profile_slug: input.shipping_profile_slug,
            tags: input.tags,
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

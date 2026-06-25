use async_graphql::{Context, Object, Result};
use rustok_api::{graphql::require_module_enabled, TenantContext};
use rustok_core::Permission;
use uuid::Uuid;

use crate::{CartService, PricingService};

use super::super::{require_commerce_permission, types::*, MODULE_SLUG};
use super::helpers::*;

#[derive(Default)]
pub struct CommercePricingMutation;

#[Object]
impl CommercePricingMutation {
    async fn preview_admin_cart_promotion(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AdminCartPromotionInput,
    ) -> Result<GqlCartPromotionPreview> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let service = CartService::new(db.clone());
        let line_item_id = validate_admin_cart_promotion_target(input.scope, input.line_item_id)?;
        let preview = match input.kind {
            GqlAdminCartPromotionKind::PercentageDiscount => {
                let discount_percent = parse_required_promotion_decimal(
                    input.discount_percent.as_deref(),
                    "discount_percent",
                )?;
                ensure_no_unused_promotion_amount(input.amount.as_deref(), "amount")?;
                match input.scope {
                    GqlAdminCartPromotionScope::Shipping => {
                        service
                            .preview_percentage_shipping_promotion(
                                tenant_id,
                                cart_id,
                                input.source_id.as_str(),
                                discount_percent,
                            )
                            .await
                    }
                    GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::LineItem => {
                        service
                            .preview_percentage_promotion(
                                tenant_id,
                                cart_id,
                                line_item_id,
                                input.source_id.as_str(),
                                discount_percent,
                            )
                            .await
                    }
                }
            }
            GqlAdminCartPromotionKind::FixedDiscount => {
                let amount = parse_required_promotion_decimal(input.amount.as_deref(), "amount")?;
                ensure_no_unused_promotion_amount(
                    input.discount_percent.as_deref(),
                    "discount_percent",
                )?;
                match input.scope {
                    GqlAdminCartPromotionScope::Shipping => {
                        service
                            .preview_fixed_shipping_promotion(
                                tenant_id,
                                cart_id,
                                input.source_id.as_str(),
                                amount,
                            )
                            .await
                    }
                    GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::LineItem => {
                        service
                            .preview_fixed_promotion(
                                tenant_id,
                                cart_id,
                                line_item_id,
                                input.source_id.as_str(),
                                amount,
                            )
                            .await
                    }
                }
            }
        }
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(map_cart_promotion_preview(input.scope, preview))
    }

    async fn apply_admin_cart_promotion(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AdminCartPromotionInput,
    ) -> Result<GqlCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_UPDATE],
            "Permission denied: orders:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let service = CartService::new(db.clone());
        let line_item_id = validate_admin_cart_promotion_target(input.scope, input.line_item_id)?;
        let metadata = parse_optional_metadata(input.metadata.as_deref())?;
        let cart = match input.kind {
            GqlAdminCartPromotionKind::PercentageDiscount => {
                let discount_percent = parse_required_promotion_decimal(
                    input.discount_percent.as_deref(),
                    "discount_percent",
                )?;
                ensure_no_unused_promotion_amount(input.amount.as_deref(), "amount")?;
                match input.scope {
                    GqlAdminCartPromotionScope::Shipping => {
                        service
                            .apply_percentage_shipping_promotion(
                                tenant_id,
                                cart_id,
                                input.source_id.as_str(),
                                discount_percent,
                                metadata,
                            )
                            .await
                    }
                    GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::LineItem => {
                        service
                            .apply_percentage_promotion(
                                tenant_id,
                                cart_id,
                                line_item_id,
                                input.source_id.as_str(),
                                discount_percent,
                                metadata,
                            )
                            .await
                    }
                }
            }
            GqlAdminCartPromotionKind::FixedDiscount => {
                let amount = parse_required_promotion_decimal(input.amount.as_deref(), "amount")?;
                ensure_no_unused_promotion_amount(
                    input.discount_percent.as_deref(),
                    "discount_percent",
                )?;
                match input.scope {
                    GqlAdminCartPromotionScope::Shipping => {
                        service
                            .apply_fixed_shipping_promotion(
                                tenant_id,
                                cart_id,
                                input.source_id.as_str(),
                                amount,
                                metadata,
                            )
                            .await
                    }
                    GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::LineItem => {
                        service
                            .apply_fixed_promotion(
                                tenant_id,
                                cart_id,
                                line_item_id,
                                input.source_id.as_str(),
                                amount,
                                metadata,
                            )
                            .await
                    }
                }
            }
        }
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(cart.into())
    }

    async fn update_admin_pricing_variant_price(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        variant_id: Uuid,
        input: UpdateAdminPricingVariantPriceInput,
    ) -> Result<GqlPricingPrice> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = PricingService::new(db.clone(), event_bus.clone());
        let currency_code = parse_pricing_currency_code(&input.currency_code)?;
        let amount = parse_decimal(&input.amount)?;
        let compare_at_amount = parse_optional_decimal(input.compare_at_amount.as_deref())?;
        let channel_slug = normalize_pricing_channel_slug(input.channel_slug.as_deref());

        if let Some(price_list_id) = input.price_list_id {
            service
                .set_price_list_tier_with_channel(
                    tenant_id,
                    auth.user_id,
                    variant_id,
                    price_list_id,
                    currency_code.as_str(),
                    amount,
                    compare_at_amount,
                    input.channel_id,
                    channel_slug.clone(),
                    input.min_quantity,
                    input.max_quantity,
                )
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        } else {
            service
                .set_price_tier_with_channel(
                    tenant_id,
                    auth.user_id,
                    variant_id,
                    currency_code.as_str(),
                    amount,
                    compare_at_amount,
                    input.channel_id,
                    channel_slug.clone(),
                    input.min_quantity,
                    input.max_quantity,
                )
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        }

        let price = load_pricing_price_row(
            &service,
            variant_id,
            &currency_code,
            input.price_list_id,
            input.channel_id,
            channel_slug.as_deref(),
            input.min_quantity,
            input.max_quantity,
        )
        .await?;

        Ok(price)
    }

    async fn preview_admin_pricing_variant_discount(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        variant_id: Uuid,
        input: AdminPricingVariantDiscountInput,
    ) -> Result<GqlPricingAdjustmentPreview> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = PricingService::new(db.clone(), event_bus.clone());
        let currency_code = parse_pricing_currency_code(&input.currency_code)?;
        let discount_percent = parse_decimal(&input.discount_percent)?;
        let channel_slug = normalize_pricing_channel_slug(input.channel_slug.as_deref());

        let preview = if let Some(price_list_id) = input.price_list_id {
            service
                .preview_price_list_percentage_discount_with_channel(
                    tenant_id,
                    variant_id,
                    price_list_id,
                    currency_code.as_str(),
                    discount_percent,
                    input.channel_id,
                    channel_slug,
                )
                .await
        } else {
            service
                .preview_percentage_discount_with_channel(
                    variant_id,
                    currency_code.as_str(),
                    discount_percent,
                    input.channel_id,
                    channel_slug,
                )
                .await
        }
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(preview.into())
    }

    async fn apply_admin_pricing_variant_discount(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        variant_id: Uuid,
        input: AdminPricingVariantDiscountInput,
    ) -> Result<GqlPricingAdjustmentPreview> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = PricingService::new(db.clone(), event_bus.clone());
        let currency_code = parse_pricing_currency_code(&input.currency_code)?;
        let discount_percent = parse_decimal(&input.discount_percent)?;
        let channel_slug = normalize_pricing_channel_slug(input.channel_slug.as_deref());

        let preview = if let Some(price_list_id) = input.price_list_id {
            service
                .apply_price_list_percentage_discount_with_channel(
                    tenant_id,
                    auth.user_id,
                    variant_id,
                    price_list_id,
                    currency_code.as_str(),
                    discount_percent,
                    input.channel_id,
                    channel_slug,
                )
                .await
        } else {
            service
                .apply_percentage_discount_with_channel(
                    tenant_id,
                    auth.user_id,
                    variant_id,
                    currency_code.as_str(),
                    discount_percent,
                    input.channel_id,
                    channel_slug,
                )
                .await
        }
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(preview.into())
    }

    async fn update_admin_pricing_price_list_rule(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        price_list_id: Uuid,
        input: UpdateAdminPricingPriceListRuleInput,
    ) -> Result<GqlActivePriceListOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        validate_active_price_list_for_rule_update(db, tenant_id, price_list_id).await?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = PricingService::new(db.clone(), event_bus.clone());
        let adjustment_percent = parse_optional_decimal(input.adjustment_percent.as_deref())?;

        service
            .set_price_list_percentage_rule(
                tenant_id,
                auth.user_id,
                price_list_id,
                adjustment_percent,
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        load_active_price_list_option(
            &service,
            tenant_id,
            price_list_id,
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await
    }

    async fn update_admin_pricing_price_list_scope(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        price_list_id: Uuid,
        input: UpdateAdminPricingPriceListScopeInput,
    ) -> Result<GqlActivePriceListOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = PricingService::new(db.clone(), event_bus.clone());

        let option = service
            .set_price_list_scope(
                tenant_id,
                auth.user_id,
                price_list_id,
                input.channel_id,
                normalize_pricing_channel_slug(input.channel_slug.as_deref()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(option.into())
    }
}

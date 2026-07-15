use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use rustok_api::{graphql::require_module_enabled, PortActor, PortContext, TenantContext};
use uuid::Uuid;

use rustok_cart::{
    in_process_cart_promotion_port, CartPromotionKindRequest, CartPromotionRequest,
    CartPromotionScopeRequest,
};
use rustok_pricing::{
    in_process_pricing_read_port, in_process_pricing_write_port, ApplyVariantDiscountRequest,
    PreviewVariantDiscountRequest, PricingService, SetPriceListScopeRequest,
};

use super::super::{require_commerce_permission, types::*, MODULE_SLUG};
use super::helpers::*;

fn cart_promotion_port_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    operation: &str,
    is_write: bool,
) -> PortContext {
    let correlation_id = format!("admin-cart-promotion:{operation}:{cart_id}");
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.graphql-admin"),
        "en",
        correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    if is_write {
        context.with_idempotency_key(correlation_id)
    } else {
        context
    }
}

fn pricing_preview_port_context(
    tenant_id: Uuid,
    user_id: Uuid,
    variant_id: Uuid,
    channel_slug: Option<&str>,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("admin-pricing-preview:{variant_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    channel_slug
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

fn pricing_write_port_context(
    tenant_id: Uuid,
    user_id: Uuid,
    operation: &str,
    aggregate_id: Uuid,
) -> PortContext {
    let correlation_id = format!("admin-pricing:{operation}:{aggregate_id}");
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2))
    .with_idempotency_key(correlation_id)
}

fn admin_cart_promotion_request(
    cart_id: Uuid,
    input: &AdminCartPromotionInput,
    line_item_id: Option<Uuid>,
    metadata: serde_json::Value,
) -> Result<CartPromotionRequest> {
    let (kind, amount) = match &input.kind {
        GqlAdminCartPromotionKind::PercentageDiscount => {
            ensure_no_unused_promotion_amount(input.amount.as_deref(), "amount")?;
            (
                CartPromotionKindRequest::PercentageDiscount,
                parse_required_promotion_decimal(
                    input.discount_percent.as_deref(),
                    "discount_percent",
                )?,
            )
        }
        GqlAdminCartPromotionKind::FixedDiscount => {
            ensure_no_unused_promotion_amount(
                input.discount_percent.as_deref(),
                "discount_percent",
            )?;
            (
                CartPromotionKindRequest::FixedDiscount,
                parse_required_promotion_decimal(input.amount.as_deref(), "amount")?,
            )
        }
    };
    let scope = match &input.scope {
        GqlAdminCartPromotionScope::Cart => CartPromotionScopeRequest::Cart,
        GqlAdminCartPromotionScope::LineItem => CartPromotionScopeRequest::LineItem,
        GqlAdminCartPromotionScope::Shipping => CartPromotionScopeRequest::Shipping,
    };
    Ok(CartPromotionRequest {
        cart_id,
        line_item_id,
        scope,
        kind,
        source_id: input.source_id.clone(),
        amount,
        metadata,
    })
}

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
        let line_item_id = validate_admin_cart_promotion_target(input.scope, input.line_item_id)?;
        let request =
            admin_cart_promotion_request(cart_id, &input, line_item_id, serde_json::Value::Null)?;
        let preview = in_process_cart_promotion_port(db.clone())
            .read_cart_promotion_preview(
                cart_promotion_port_context(tenant_id, cart_id, "preview", false),
                request,
            )
            .await
            .map_err(cart_port_error)?;

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
        let line_item_id = validate_admin_cart_promotion_target(input.scope, input.line_item_id)?;
        let metadata = parse_optional_metadata(input.metadata.as_deref())?;
        let request = admin_cart_promotion_request(cart_id, &input, line_item_id, metadata)?;
        let cart = in_process_cart_promotion_port(db.clone())
            .apply_cart_promotion(
                cart_promotion_port_context(tenant_id, cart_id, "apply", true),
                request,
            )
            .await
            .map_err(cart_port_error)?;

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
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let currency_code = parse_pricing_currency_code(&input.currency_code)?;
        let discount_percent = parse_decimal(&input.discount_percent)?;
        let channel_slug = normalize_pricing_channel_slug(input.channel_slug.as_deref());

        let preview = in_process_pricing_read_port(db.clone(), event_bus.clone())
            .preview_variant_discount(
                pricing_preview_port_context(
                    tenant_id,
                    auth.user_id,
                    variant_id,
                    channel_slug.as_deref(),
                ),
                PreviewVariantDiscountRequest {
                    variant_id,
                    price_list_id: input.price_list_id,
                    currency_code,
                    discount_percent,
                    channel_id: input.channel_id,
                    channel_slug,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.message))?;

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
        let currency_code = parse_pricing_currency_code(&input.currency_code)?;
        let discount_percent = parse_decimal(&input.discount_percent)?;
        let channel_slug = normalize_pricing_channel_slug(input.channel_slug.as_deref());

        let preview = in_process_pricing_write_port(db.clone(), event_bus.clone())
            .apply_variant_discount(
                pricing_write_port_context(
                    tenant_id,
                    auth.user_id,
                    "apply-variant-discount",
                    variant_id,
                ),
                ApplyVariantDiscountRequest {
                    variant_id,
                    price_list_id: input.price_list_id,
                    currency_code,
                    discount_percent,
                    channel_id: input.channel_id,
                    channel_slug,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.message))?;

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
        let option = in_process_pricing_write_port(db.clone(), event_bus.clone())
            .set_price_list_scope(
                pricing_write_port_context(
                    tenant_id,
                    auth.user_id,
                    "set-price-list-scope",
                    price_list_id,
                ),
                SetPriceListScopeRequest {
                    price_list_id,
                    channel_id: input.channel_id,
                    channel_slug: normalize_pricing_channel_slug(input.channel_slug.as_deref()),
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.message))?;

        Ok(option.into())
    }
}

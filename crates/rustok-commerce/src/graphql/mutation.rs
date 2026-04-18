use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext, RequestContext, TenantContext,
};
use rustok_core::{locale_tags_match, Permission};
use rustok_pricing::PriceResolutionContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    entities::{price_list, product, product_translation, product_variant, variant_translation},
    storefront_channel::{
        is_metadata_visible_for_public_channel,
        load_available_inventory_for_variant_in_public_channel, normalize_public_channel_slug,
    },
    storefront_shipping::{
        effective_shipping_profile_slug, enrich_cart_delivery_groups,
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
    CartService, CatalogService, CheckoutService, CustomerService, FulfillmentOrchestrationService,
    FulfillmentService, OrderService, PaymentService, PricingService, ShippingProfileService,
    StoreContextService,
};

use super::{require_commerce_permission, types::*, MODULE_SLUG};

#[derive(Default)]
pub struct CommerceMutation;

#[Object]
impl CommerceMutation {
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

    async fn create_storefront_cart(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontCartInput,
    ) -> Result<GqlStoreCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let context = StoreContextService::new(db.clone())
            .resolve_context(
                tenant_id,
                crate::dto::ResolveStoreContextInput {
                    region_id: input.region_id,
                    country_code: input.country_code.clone(),
                    locale: input
                        .locale
                        .clone()
                        .or_else(|| Some(request_context.locale.clone())),
                    currency_code: input.currency_code.clone(),
                },
            )
            .await?;
        let currency_code = context
            .currency_code
            .clone()
            .or(input.currency_code.clone())
            .ok_or_else(|| {
                async_graphql::Error::new(
                    "currency_code is required unless it can be resolved from region/country",
                )
            })?;

        let cart = CartService::new(db.clone())
            .create_cart_with_channel(
                tenant_id,
                crate::dto::CreateCartInput {
                    customer_id,
                    email: input.email,
                    region_id: context.region.as_ref().map(|region| region.id),
                    country_code: input.country_code,
                    locale_code: Some(context.locale.clone()),
                    selected_shipping_option_id: None,
                    currency_code,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
                request_context.channel_id,
                request_context.channel_slug.clone(),
            )
            .await?;
        let cart = enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?;

        Ok(GqlStoreCart {
            cart: cart.into(),
            context: context.into(),
        })
    }

    async fn add_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        input: AddStorefrontCartLineItemInput,
    ) -> Result<GqlCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, cart_id).await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let pricing_service = PricingService::new(db.clone(), event_bus.clone());
        let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, ctx);
        let pricing_context = build_storefront_pricing_context(
            &cart,
            request_context,
            public_channel_slug.as_deref(),
            input.quantity,
        );
        let resolved_input = resolve_storefront_line_item_input(
            db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            &cart.currency_code,
            cart.locale_code
                .as_deref()
                .unwrap_or(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
            public_channel_slug.as_deref(),
            input,
        )
        .await?;

        let updated = cart_service
            .add_line_item_with_pricing_adjustment(
                tenant_id,
                cart_id,
                resolved_input.add_line_item,
                resolved_input.pricing_adjustment,
            )
            .await?;
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }

    async fn update_storefront_cart_context(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        input: UpdateStorefrontCartContextInput,
    ) -> Result<GqlStoreCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, cart_id).await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;

        let region_was_explicit = !input.region_id.is_undefined();
        let email = maybe_undefined_or_existing(input.email, cart.email.clone());
        let requested_region_id = maybe_undefined_or_existing(input.region_id, cart.region_id);
        let requested_country_code = match input.country_code {
            async_graphql::MaybeUndefined::Value(value) => Some(value),
            async_graphql::MaybeUndefined::Null => None,
            async_graphql::MaybeUndefined::Undefined if region_was_explicit => None,
            async_graphql::MaybeUndefined::Undefined => cart.country_code.clone(),
        };
        let requested_locale = maybe_undefined_or_existing(input.locale, cart.locale_code.clone())
            .or_else(|| Some(request_context.locale.clone()));
        let requested_shipping_option_id = maybe_undefined_or_existing(
            input.selected_shipping_option_id,
            cart.selected_shipping_option_id,
        );
        let requested_shipping_selections = match input.shipping_selections {
            async_graphql::MaybeUndefined::Value(items) => Some(
                items
                    .into_iter()
                    .map(|item| crate::dto::CartShippingSelectionInput {
                        shipping_profile_slug: item.shipping_profile_slug,
                        seller_id: item.seller_id,
                        seller_scope: item.seller_scope,
                        selected_shipping_option_id: item.selected_shipping_option_id,
                    })
                    .collect::<Vec<_>>(),
            ),
            async_graphql::MaybeUndefined::Null => Some(Vec::new()),
            async_graphql::MaybeUndefined::Undefined => None,
        };

        let context = StoreContextService::new(db.clone())
            .resolve_context(
                tenant_id,
                crate::dto::ResolveStoreContextInput {
                    region_id: requested_region_id,
                    country_code: requested_country_code.clone(),
                    locale: requested_locale,
                    currency_code: Some(cart.currency_code.clone()),
                },
            )
            .await?;
        validate_selected_shipping_option(
            db,
            tenant_id,
            &cart,
            requested_shipping_option_id,
            requested_shipping_selections.as_deref(),
            &cart.currency_code,
            storefront_public_channel_slug_for_cart(&cart, ctx).as_deref(),
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await?;

        let updated = cart_service
            .update_context(
                tenant_id,
                cart_id,
                crate::dto::UpdateCartContextInput {
                    email,
                    region_id: context.region.as_ref().map(|region| region.id),
                    country_code: requested_country_code,
                    locale_code: Some(context.locale.clone()),
                    selected_shipping_option_id: requested_shipping_option_id,
                    shipping_selections: Some(
                        requested_shipping_selections
                            .unwrap_or_else(|| current_shipping_selections(&cart)),
                    ),
                },
            )
            .await?;
        let updated = reprice_storefront_cart_line_items(
            db,
            tenant_id,
            request_context,
            &event_bus,
            &cart_service,
            updated,
        )
        .await?;
        let updated = enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?;

        Ok(GqlStoreCart {
            cart: updated.into(),
            context: context.into(),
        })
    }

    async fn update_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        line_id: Uuid,
        input: UpdateStorefrontCartLineItemInput,
    ) -> Result<GqlCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, cart_id).await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, ctx);
        if let Some(existing_line_item) = cart.line_items.iter().find(|item| item.id == line_id) {
            if let Some(variant_id) = existing_line_item.variant_id {
                validate_storefront_line_item_quantity(
                    db,
                    tenant_id,
                    variant_id,
                    input.quantity,
                    public_channel_slug.as_deref(),
                )
                .await?;
            }
        }
        let updated = if let Some(variant_id) = cart
            .line_items
            .iter()
            .find(|item| item.id == line_id)
            .and_then(|item| item.variant_id)
        {
            let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
            let pricing_service = PricingService::new(db.clone(), event_bus.clone());
            let pricing_context = build_storefront_pricing_context(
                &cart,
                request_context,
                public_channel_slug.as_deref(),
                input.quantity,
            );
            let resolved_price = pricing_service
                .resolve_variant_price(tenant_id, variant_id, pricing_context)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?
                .ok_or_else(|| {
                    async_graphql::Error::new(format!(
                        "No storefront price for variant {} in currency {}",
                        variant_id, cart.currency_code
                    ))
                })?;

            let pricing_update =
                storefront_cart_pricing_update(line_id, input.quantity, &resolved_price);
            cart_service
                .update_line_item_pricing(
                    tenant_id,
                    cart_id,
                    line_id,
                    input.quantity,
                    pricing_update.unit_price,
                    pricing_update.pricing_adjustment,
                )
                .await?
        } else {
            cart_service
                .update_line_item_quantity(tenant_id, cart_id, line_id, input.quantity)
                .await?
        };
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }

    async fn remove_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        line_id: Uuid,
    ) -> Result<GqlCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, cart_id).await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let updated = cart_service
            .remove_line_item(tenant_id, cart_id, line_id)
            .await?;
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            ctx.data::<RequestContext>()?,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }

    async fn create_storefront_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontPaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, input.cart_id).await?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let cart = reprice_storefront_cart_line_items(
            db,
            tenant_id,
            request_context,
            event_bus,
            &cart_service,
            cart,
        )
        .await?;
        let context = crate::StoreContextService::new(db.clone())
            .resolve_context(
                tenant_id,
                crate::dto::ResolveStoreContextInput {
                    region_id: cart.region_id,
                    country_code: cart.country_code.clone(),
                    locale: cart
                        .locale_code
                        .clone()
                        .or_else(|| Some(request_context.locale.clone())),
                    currency_code: Some(cart.currency_code.clone()),
                },
            )
            .await?;

        let service = PaymentService::new(db.clone());
        if let Some(existing) = service
            .find_reusable_collection_by_cart(tenant_id, cart.id)
            .await?
        {
            return Ok(existing.into());
        }

        let collection = service
            .create_collection(
                tenant_id,
                crate::dto::CreatePaymentCollectionInput {
                    cart_id: Some(cart.id),
                    order_id: None,
                    customer_id: cart.customer_id,
                    currency_code: cart.currency_code.clone(),
                    amount: cart.total_amount,
                    metadata: merge_graphql_metadata(
                        parse_optional_metadata(input.metadata.as_deref())?,
                        cart_context_metadata(&cart, &context),
                    ),
                },
            )
            .await?;

        Ok(collection.into())
    }

    async fn complete_storefront_checkout(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CompleteStorefrontCheckoutInput,
    ) -> Result<GqlCompleteCheckout> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let cart_service = CartService::new(db.clone());
        let mut cart = cart_service.get_cart(tenant_id, input.cart_id).await?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        if input.shipping_option_id.is_some()
            || input.shipping_selections.is_some()
            || input.region_id.is_some()
            || input.country_code.is_some()
            || input.locale.is_some()
        {
            let requested_region_id = input.region_id.or(cart.region_id);
            let requested_country_code = input
                .country_code
                .clone()
                .or_else(|| cart.country_code.clone());
            let requested_locale = input
                .locale
                .clone()
                .or_else(|| cart.locale_code.clone())
                .or_else(|| Some(request_context.locale.clone()));
            let requested_shipping_option_id = input
                .shipping_option_id
                .or(cart.selected_shipping_option_id);
            let requested_shipping_selections = input
                .shipping_selections
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| crate::dto::CartShippingSelectionInput {
                            shipping_profile_slug: item.shipping_profile_slug.clone(),
                            seller_id: item.seller_id.clone(),
                            seller_scope: item.seller_scope.clone(),
                            selected_shipping_option_id: item.selected_shipping_option_id,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| current_shipping_selections(&cart));

            let context = StoreContextService::new(db.clone())
                .resolve_context(
                    tenant_id,
                    crate::dto::ResolveStoreContextInput {
                        region_id: requested_region_id,
                        country_code: requested_country_code.clone(),
                        locale: requested_locale,
                        currency_code: Some(cart.currency_code.clone()),
                    },
                )
                .await?;
            validate_selected_shipping_option(
                db,
                tenant_id,
                &cart,
                requested_shipping_option_id,
                Some(requested_shipping_selections.as_slice()),
                &cart.currency_code,
                storefront_public_channel_slug_for_cart(&cart, ctx).as_deref(),
                Some(request_context.locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await?;

            cart = cart_service
                .update_context(
                    tenant_id,
                    cart.id,
                    crate::dto::UpdateCartContextInput {
                        email: cart.email.clone(),
                        region_id: context.region.as_ref().map(|region| region.id),
                        country_code: requested_country_code,
                        locale_code: Some(context.locale.clone()),
                        selected_shipping_option_id: requested_shipping_option_id,
                        shipping_selections: Some(requested_shipping_selections),
                    },
                )
                .await?;
        }
        let _ = reprice_storefront_cart_line_items(
            db,
            tenant_id,
            request_context,
            event_bus,
            &cart_service,
            cart,
        )
        .await?;
        let actor_id = ctx
            .data_opt::<AuthContext>()
            .map(|auth| auth.user_id)
            .unwrap_or_else(Uuid::nil);

        let response = CheckoutService::new(db.clone(), event_bus.clone())
            .complete_checkout(
                tenant_id,
                actor_id,
                crate::dto::CompleteCheckoutInput {
                    cart_id: input.cart_id,
                    shipping_option_id: input.shipping_option_id,
                    shipping_selections: input.shipping_selections.map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::dto::CartShippingSelectionInput {
                                shipping_profile_slug: item.shipping_profile_slug,
                                seller_id: item.seller_id,
                                seller_scope: item.seller_scope,
                                selected_shipping_option_id: item.selected_shipping_option_id,
                            })
                            .collect()
                    }),
                    region_id: input.region_id,
                    country_code: input.country_code,
                    locale: input.locale,
                    create_fulfillment: input.create_fulfillment.unwrap_or(true),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(response.into())
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
        let refund = PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                payment_collection_id,
                crate::dto::CreateRefundInput {
                    amount: parse_decimal(&input.amount)?,
                    reason: input.reason,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

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

    async fn create_shipping_option(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateShippingOptionInputObject,
    ) -> Result<GqlShippingOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_CREATE],
            "Permission denied: fulfillments:create required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        validate_shipping_option_profile_inputs(
            db,
            tenant_id,
            input.allowed_shipping_profile_slugs.as_ref(),
        )
        .await?;
        let option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                crate::dto::CreateShippingOptionInput {
                    translations: input
                        .translations
                        .into_iter()
                        .map(|translation| crate::dto::ShippingOptionTranslationInput {
                            locale: translation.locale,
                            name: translation.name,
                        })
                        .collect(),
                    currency_code: input.currency_code,
                    amount: parse_decimal(&input.amount)?,
                    provider_id: input.provider_id,
                    allowed_shipping_profile_slugs: input.allowed_shipping_profile_slugs,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(option.into())
    }

    async fn update_shipping_option(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateShippingOptionInputObject,
    ) -> Result<GqlShippingOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        validate_shipping_option_profile_inputs(
            db,
            tenant_id,
            input.allowed_shipping_profile_slugs.as_ref(),
        )
        .await?;
        let option = FulfillmentService::new(db.clone())
            .update_shipping_option(
                tenant_id,
                id,
                crate::dto::UpdateShippingOptionInput {
                    translations: input.translations.map(|translations| {
                        translations
                            .into_iter()
                            .map(|translation| crate::dto::ShippingOptionTranslationInput {
                                locale: translation.locale,
                                name: translation.name,
                            })
                            .collect()
                    }),
                    currency_code: input.currency_code,
                    amount: parse_optional_decimal(input.amount.as_deref())?,
                    provider_id: input.provider_id,
                    allowed_shipping_profile_slugs: input.allowed_shipping_profile_slugs,
                    metadata: input
                        .metadata
                        .as_deref()
                        .map(|value| {
                            serde_json::from_str(value).map_err(|_| {
                                async_graphql::Error::new("Invalid JSON metadata payload")
                            })
                        })
                        .transpose()?,
                },
            )
            .await?;

        Ok(option.into())
    }

    async fn create_shipping_profile(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateShippingProfileInputObject,
    ) -> Result<GqlShippingProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_CREATE],
            "Permission denied: fulfillments:create required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let profile = ShippingProfileService::new(db.clone())
            .create_shipping_profile(
                tenant_id,
                crate::dto::CreateShippingProfileInput {
                    slug: input.slug,
                    translations: input
                        .translations
                        .into_iter()
                        .map(|translation| crate::dto::ShippingProfileTranslationInput {
                            locale: translation.locale,
                            name: translation.name,
                            description: translation.description,
                        })
                        .collect(),
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await?;

        Ok(profile.into())
    }

    async fn update_shipping_profile(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateShippingProfileInputObject,
    ) -> Result<GqlShippingProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let profile = ShippingProfileService::new(db.clone())
            .update_shipping_profile(
                tenant_id,
                id,
                crate::dto::UpdateShippingProfileInput {
                    slug: input.slug,
                    translations: input.translations.map(|translations| {
                        translations
                            .into_iter()
                            .map(|translation| crate::dto::ShippingProfileTranslationInput {
                                locale: translation.locale,
                                name: translation.name,
                                description: translation.description,
                            })
                            .collect()
                    }),
                    metadata: if input.metadata.is_some() {
                        Some(parse_optional_metadata(input.metadata.as_deref())?)
                    } else {
                        None
                    },
                },
            )
            .await?;

        Ok(profile.into())
    }

    async fn deactivate_shipping_profile(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<GqlShippingProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let profile = ShippingProfileService::new(db.clone())
            .deactivate_shipping_profile(tenant_id, id)
            .await?;

        Ok(profile.into())
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

    async fn reactivate_shipping_profile(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<GqlShippingProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let profile = ShippingProfileService::new(db.clone())
            .reactivate_shipping_profile(tenant_id, id)
            .await?;

        Ok(profile.into())
    }

    async fn deactivate_shipping_option(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<GqlShippingOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let option = FulfillmentService::new(db.clone())
            .deactivate_shipping_option(tenant_id, id)
            .await?;

        Ok(option.into())
    }

    async fn reactivate_shipping_option(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<GqlShippingOption> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_UPDATE],
            "Permission denied: fulfillments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let option = FulfillmentService::new(db.clone())
            .reactivate_shipping_option(tenant_id, id)
            .await?;

        Ok(option.into())
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
            translations: option
                .translations
                .into_iter()
                .map(|translation| crate::dto::ProductOptionTranslationInput {
                    locale: translation.locale,
                    name: translation.name,
                    values: translation.values,
                })
                .collect(),
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
                        channel_id: price.channel_id,
                        channel_slug: price.channel_slug,
                        amount,
                        compare_at_amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(crate::dto::CreateVariantInput {
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
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
        seller_id: input.seller_id,
        vendor: input.vendor,
        product_type: input.product_type,
        shipping_profile_slug: input.shipping_profile_slug,
        tags: input.tags.unwrap_or_default(),
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

fn parse_pricing_currency_code(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(async_graphql::Error::new(
            "currency_code must be a 3-letter code",
        ));
    }
    Ok(normalized)
}

fn validate_admin_cart_promotion_target(
    scope: GqlAdminCartPromotionScope,
    line_item_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    match scope {
        GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::Shipping => {
            if line_item_id.is_some() {
                return Err(async_graphql::Error::new(
                    "line_item_id is allowed only for line_item scope",
                ));
            }
            Ok(None)
        }
        GqlAdminCartPromotionScope::LineItem => line_item_id.map(Some).ok_or_else(|| {
            async_graphql::Error::new("line_item_id is required for line_item scope")
        }),
    }
}

fn parse_required_promotion_decimal(value: Option<&str>, field: &str) -> Result<Decimal> {
    let Some(value) = value else {
        return Err(async_graphql::Error::new(format!(
            "{field} is required for the selected promotion kind"
        )));
    };
    parse_decimal(value)
}

fn ensure_no_unused_promotion_amount(value: Option<&str>, field: &str) -> Result<()> {
    if value.is_some() {
        return Err(async_graphql::Error::new(format!(
            "{field} must be omitted for the selected promotion kind"
        )));
    }
    Ok(())
}

fn map_cart_promotion_preview(
    scope: GqlAdminCartPromotionScope,
    preview: rustok_cart::services::cart::CartPromotionPreview,
) -> GqlCartPromotionPreview {
    GqlCartPromotionPreview {
        kind: match preview.kind {
            rustok_cart::services::cart::CartPromotionKind::PercentageDiscount => {
                "percentage_discount".to_string()
            }
            rustok_cart::services::cart::CartPromotionKind::FixedDiscount => {
                "fixed_discount".to_string()
            }
        },
        scope: match scope {
            GqlAdminCartPromotionScope::Cart => "cart".to_string(),
            GqlAdminCartPromotionScope::LineItem => "line_item".to_string(),
            GqlAdminCartPromotionScope::Shipping => "shipping".to_string(),
        },
        line_item_id: preview.line_item_id,
        currency_code: preview.currency_code,
        base_amount: preview.base_amount.to_string(),
        adjustment_amount: preview.adjustment_amount.to_string(),
        adjusted_amount: preview.adjusted_amount.to_string(),
    }
}

fn normalize_pricing_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn parse_optional_metadata(value: Option<&str>) -> Result<Value> {
    match value.map(str::trim) {
        None | Some("") => Ok(Value::Object(Default::default())),
        Some(value) => serde_json::from_str(value)
            .map_err(|_| async_graphql::Error::new("Invalid JSON metadata payload")),
    }
}

async fn resolve_optional_storefront_customer_id(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match CustomerService::new(db.clone())
        .get_customer_by_user(tenant_id, auth.user_id)
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::error::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(err) => Err(async_graphql::Error::new(err.to_string())),
    }
}

fn ensure_storefront_cart_access(
    cart: &crate::dto::CartResponse,
    customer_id: Option<Uuid>,
) -> Result<()> {
    if let Some(expected_customer_id) = cart.customer_id {
        if customer_id.is_none() {
            return Err(<FieldError as GraphQLError>::unauthenticated());
        }
        if customer_id != Some(expected_customer_id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Cart belongs to another customer",
            ));
        }
    }

    Ok(())
}

fn merge_graphql_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn map_price_row_to_gql_price(price: crate::entities::price::Model) -> GqlPricingPrice {
    let on_sale = price
        .compare_at_amount
        .filter(|compare_at| *compare_at > Decimal::ZERO)
        .map(|compare_at| compare_at > price.amount)
        .unwrap_or(false);
    let discount_percent = price.compare_at_amount.and_then(|compare_at_amount| {
        if compare_at_amount <= Decimal::ZERO || compare_at_amount <= price.amount {
            return None;
        }

        Some(
            (((compare_at_amount - price.amount) / compare_at_amount) * Decimal::from(100))
                .round_dp(2)
                .normalize()
                .to_string(),
        )
    });

    GqlPricingPrice {
        currency_code: price.currency_code,
        amount: price.amount.normalize().to_string(),
        compare_at_amount: price
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        discount_percent,
        on_sale,
        price_list_id: price.price_list_id,
        channel_id: price.channel_id,
        channel_slug: price.channel_slug,
        min_quantity: price.min_quantity,
        max_quantity: price.max_quantity,
    }
}

async fn load_pricing_price_row(
    service: &PricingService,
    variant_id: Uuid,
    currency_code: &str,
    price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> Result<GqlPricingPrice> {
    let normalized_channel_slug = normalize_pricing_channel_slug(channel_slug);
    let prices = service
        .get_variant_prices(variant_id)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    let price = prices
        .into_iter()
        .find(|price| {
            price.currency_code.eq_ignore_ascii_case(currency_code)
                && price.price_list_id == price_list_id
                && price.channel_id == channel_id
                && normalize_pricing_channel_slug(price.channel_slug.as_deref())
                    == normalized_channel_slug
                && price.min_quantity == min_quantity
                && price.max_quantity == max_quantity
        })
        .ok_or_else(|| async_graphql::Error::new("Updated pricing row was not found"))?;

    Ok(map_price_row_to_gql_price(price))
}

async fn load_active_price_list_option(
    service: &PricingService,
    tenant_id: Uuid,
    price_list_id: Uuid,
    requested_locale: &str,
    tenant_default_locale: &str,
) -> Result<GqlActivePriceListOption> {
    let option = service
        .list_active_price_lists(
            tenant_id,
            Some(requested_locale),
            Some(tenant_default_locale),
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?
        .into_iter()
        .find(|item| item.id == price_list_id)
        .ok_or_else(|| {
            async_graphql::Error::new("price_list_id must reference an active price list")
        })?;

    Ok(option.into())
}

async fn validate_active_price_list_for_rule_update(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> Result<()> {
    let price_list = price_list::Entity::find_by_id(price_list_id)
        .filter(price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| async_graphql::Error::new("price_list_id was not found"))?;

    if !price_list.status.eq_ignore_ascii_case("active") {
        return Err(async_graphql::Error::new(
            "price_list_id must reference an active price list",
        ));
    }

    let now = chrono::Utc::now();
    if price_list
        .starts_at
        .map(|item| item.with_timezone(&chrono::Utc) > now)
        .unwrap_or(false)
    {
        return Err(async_graphql::Error::new("price_list_id is not active yet"));
    }

    if price_list
        .ends_at
        .map(|item| item.with_timezone(&chrono::Utc) < now)
        .unwrap_or(false)
    {
        return Err(async_graphql::Error::new(
            "price_list_id is already expired",
        ));
    }

    Ok(())
}

fn cart_context_metadata(
    cart: &crate::dto::CartResponse,
    context: &crate::dto::StoreContextResponse,
) -> Value {
    serde_json::json!({
        "cart_context": {
            "region_id": context.region.as_ref().map(|region| region.id),
            "country_code": cart.country_code.clone(),
            "locale": context.locale.clone(),
            "currency_code": cart.currency_code.clone(),
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "shipping_selections": current_shipping_selections(cart),
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

async fn enrich_storefront_cart(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| normalize_public_channel_slug(request_context.channel_slug.as_deref()));
    enrich_cart_delivery_groups(
        db,
        tenant_id,
        cart,
        public_channel_slug.as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await
    .map_err(|err| async_graphql::Error::new(err.to_string()))
}

fn request_public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| {
            normalize_public_channel_slug(request_context.channel_slug.as_deref())
        })
}

fn storefront_public_channel_slug_for_cart(
    cart: &crate::dto::CartResponse,
    ctx: &Context<'_>,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| request_public_channel_slug(ctx))
}

async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    cart: &crate::dto::CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    let selections = if let Some(shipping_selections) = shipping_selections {
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
        if cart.delivery_groups.len() > 1 {
            return Err(async_graphql::Error::new(
                "selectedShippingOptionId can only be used for carts with a single delivery group",
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: group.seller_scope.clone(),
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        current_shipping_selections(cart)
    };

    for selection in selections {
        let Some(selected_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let option = FulfillmentService::new(db.clone())
            .get_shipping_option(
                tenant_id,
                selected_shipping_option_id,
                requested_locale,
                tenant_default_locale,
            )
            .await?;
        if !option.currency_code.eq_ignore_ascii_case(currency_code) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} uses currency {}, expected {}",
                option.id, option.currency_code, currency_code
            )));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} is not available for the current channel",
                option.id
            )));
        }
        let required_shipping_profiles =
            std::collections::BTreeSet::from([normalize_shipping_profile_slug(
                selection.shipping_profile_slug.as_str(),
            )
            .unwrap_or_else(|| "default".to_string())]);
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} is not compatible with shipping profile {}",
                option.id, selection.shipping_profile_slug
            )));
        }
    }

    Ok(())
}

fn current_shipping_selections(
    cart: &crate::dto::CartResponse,
) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: group.seller_scope.clone(),
            selected_shipping_option_id: group.selected_shipping_option_id,
        })
        .collect()
}

async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(())
}

async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(())
}

fn maybe_undefined_or_existing<T>(
    value: async_graphql::MaybeUndefined<T>,
    current: Option<T>,
) -> Option<T> {
    match value {
        async_graphql::MaybeUndefined::Value(value) => Some(value),
        async_graphql::MaybeUndefined::Null => None,
        async_graphql::MaybeUndefined::Undefined => current,
    }
}

async fn resolve_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_service: &PricingService,
    pricing_context: &PriceResolutionContext,
    currency_code: &str,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: AddStorefrontCartLineItemInput,
) -> Result<ResolvedStorefrontLineItemInput> {
    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Variant not found"))?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Product not found"))?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(async_graphql::Error::new("Product not found"));
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await?;

    let resolved_price = pricing_service
        .resolve_variant_price(tenant_id, variant.id, pricing_context.clone())
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?
        .ok_or_else(|| {
            async_graphql::Error::new(format!(
                "No storefront price for variant {} in currency {}",
                variant.id, currency_code
            ))
        })?;
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(input.quantity, &resolved_price);
    validate_storefront_variant_inventory(
        db,
        tenant_id,
        &variant,
        input.quantity,
        public_channel_slug,
    )
    .await?;

    let base_title = pick_product_translation(&product_translation_models, locale, default_locale)
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match pick_variant_translation(&variant_translation_models, locale, default_locale)
        .and_then(|translation| translation.title.clone())
    {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(ResolvedStorefrontLineItemInput {
        add_line_item: crate::dto::AddCartLineItemInput {
            product_id: Some(product_model.id),
            variant_id: Some(variant.id),
            shipping_profile_slug: Some(effective_shipping_profile_slug(
                product_model.shipping_profile_slug.as_deref(),
                &product_model.metadata,
                variant.shipping_profile_slug.as_deref(),
            )),
            sku: variant.sku.clone(),
            title,
            quantity: input.quantity,
            unit_price: base_unit_price,
            metadata: merge_graphql_metadata(
                parse_optional_metadata(input.metadata.as_deref())?,
                seller_snapshot_metadata(product_model.seller_id.as_deref()),
            ),
        },
        pricing_adjustment,
    })
}

struct ResolvedStorefrontLineItemInput {
    add_line_item: crate::dto::AddCartLineItemInput,
    pricing_adjustment: Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
}

fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

fn pick_variant_translation<'a>(
    translations: &'a [variant_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a variant_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

fn resolve_commerce_graphql_locale(
    ctx: &Context<'_>,
    requested: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    requested
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            ctx.data_opt::<RequestContext>()
                .map(|request| request.locale.clone())
        })
        .unwrap_or_else(|| tenant_default_locale.to_string())
}

fn build_storefront_pricing_context(
    cart: &crate::dto::CartResponse,
    request_context: &RequestContext,
    public_channel_slug: Option<&str>,
    quantity: i32,
) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: cart.currency_code.to_ascii_uppercase(),
        region_id: cart.region_id,
        price_list_id: None,
        channel_id: cart.channel_id.or(request_context.channel_id),
        channel_slug: public_channel_slug.map(|slug| slug.to_string()),
        quantity: Some(quantity),
    }
}

async fn reprice_storefront_cart_line_items(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    event_bus: &rustok_outbox::TransactionalEventBus,
    cart_service: &CartService,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| normalize_public_channel_slug(request_context.channel_slug.as_deref()));
    let pricing_service = PricingService::new(db.clone(), event_bus.clone());
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context = build_storefront_pricing_context(
            &cart,
            request_context,
            public_channel_slug.as_deref(),
            line_item.quantity,
        );
        let resolved_price = pricing_service
            .resolve_variant_price(tenant_id, variant_id, pricing_context)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?
            .ok_or_else(|| {
                async_graphql::Error::new(format!(
                    "No storefront price for variant {} in currency {}",
                    variant_id, cart.currency_code
                ))
            })?;
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        cart_service
            .reprice_line_items(tenant_id, cart.id, updates)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(quantity, resolved_price);

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

fn storefront_cart_pricing_snapshot(
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> (
    Decimal,
    Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
) {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    (base_unit_price, pricing_adjustment)
}

fn normalize_graphql_seller_scope(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_graphql_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn seller_snapshot_metadata(seller_id: Option<&str>) -> Value {
    let seller_id = normalize_graphql_seller_id(seller_id);
    let seller_scope = seller_id
        .as_deref()
        .and_then(|value| normalize_graphql_seller_scope(Some(value)));

    serde_json::json!({
        "seller": {
            "id": seller_id,
            "scope": seller_scope,
        }
    })
}

async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    let Some(variant) = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    else {
        return Ok(());
    };

    validate_storefront_variant_inventory(
        db,
        tenant_id,
        &variant,
        requested_quantity,
        public_channel_slug,
    )
    .await
}

async fn validate_storefront_variant_inventory(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    if variant.inventory_policy == "continue" {
        return Ok(());
    }

    let available_inventory = load_available_inventory_for_variant_in_public_channel(
        db,
        tenant_id,
        variant.id,
        public_channel_slug,
    )
    .await
    .map_err(|err| async_graphql::Error::new(err.to_string()))?;
    if available_inventory < requested_quantity {
        return Err(async_graphql::Error::new(format!(
            "Variant {} does not have enough available inventory for the current channel",
            variant.id
        )));
    }

    Ok(())
}

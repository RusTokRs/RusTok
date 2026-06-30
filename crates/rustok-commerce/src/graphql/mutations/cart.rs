use async_graphql::{Context, Object, Result};
use rustok_api::{graphql::require_module_enabled, AuthContext, RequestContext, TenantContext};
use uuid::Uuid;

use crate::StoreContextService;
use rustok_cart::CartService;
use rustok_pricing::PricingService;

use super::super::{types::*, MODULE_SLUG};
use super::helpers::*;

#[derive(Default)]
pub struct CommerceCartMutation;

#[Object]
impl CommerceCartMutation {
    async fn create_storefront_cart(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontCartInput,
    ) -> Result<GqlStoreCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let context = StoreContextService::new(
            db.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
        )
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
        super::super::require_storefront_channel_enabled(ctx).await?;

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
        let inventory_service =
            rustok_inventory::InventoryService::new(db.clone(), event_bus.clone());
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
            &inventory_service,
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
        super::super::require_storefront_channel_enabled(ctx).await?;

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
                        seller_scope: None,
                        selected_shipping_option_id: item.selected_shipping_option_id,
                    })
                    .collect::<Vec<_>>(),
            ),
            async_graphql::MaybeUndefined::Null => Some(Vec::new()),
            async_graphql::MaybeUndefined::Undefined => None,
        };

        let context = StoreContextService::new(
            db.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
        )
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
            event_bus,
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
        super::super::require_storefront_channel_enabled(ctx).await?;

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
        let inventory_service =
            rustok_inventory::InventoryService::new(db.clone(), event_bus.clone());
        let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, ctx);
        if let Some(existing_line_item) = cart.line_items.iter().find(|item| item.id == line_id) {
            if let Some(variant_id) = existing_line_item.variant_id {
                validate_storefront_line_item_quantity(
                    &inventory_service,
                    db,
                    tenant_id,
                    variant_id,
                    input.quantity,
                    public_channel_slug.as_deref(),
                    cart.locale_code
                        .as_deref()
                        .unwrap_or(request_context.locale.as_str()),
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
        super::super::require_storefront_channel_enabled(ctx).await?;

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
}

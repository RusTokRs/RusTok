use async_graphql::{Context, Object, Result};
use rustok_api::{AuthContext, RequestContext, TenantContext, graphql::require_module_enabled};
use uuid::Uuid;

use crate::StoreContextService;
use rustok_cart::{
    CartStorefrontAddLineItemRequest, CartStorefrontContextUpdateRequest,
    CartStorefrontCreateRequest, CartStorefrontLineItemPricingRequest,
    CartStorefrontLineItemQuantityRequest, CartStorefrontReadRequest,
    CartStorefrontRemoveLineItemRequest, in_process_cart_storefront_port,
};
use rustok_pricing::{ResolveProductPriceRequest, in_process_pricing_read_port};

use super::super::{MODULE_SLUG, current_tenant_scope, types::*};
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
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Storefront cart creation")?;
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

        let cart = in_process_cart_storefront_port(db.clone())
            .create_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    tenant_id,
                    "create",
                    true,
                ),
                CartStorefrontCreateRequest {
                    input: crate::dto::CreateCartInput {
                        customer_id,
                        email: input.email,
                        region_id: context.region.as_ref().map(|region| region.id),
                        country_code: input.country_code,
                        locale_code: Some(context.locale.clone()),
                        selected_shipping_option_id: None,
                        currency_code,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                    channel_id: request_context.channel_id,
                    channel_slug: request_context.channel_slug.clone(),
                },
            )
            .await
            .map_err(cart_port_error)?;
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
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Add storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
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
            pricing_read_port.as_ref(),
            storefront_pricing_port_context(tenant_id, request_context, cart_id, input.variant_id),
            &pricing_context,
            cart.locale_code
                .as_deref()
                .unwrap_or(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
            public_channel_slug.as_deref(),
            input,
        )
        .await?;

        let updated = cart_storefront_port
            .add_storefront_line_item(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "add-line-item",
                    true,
                ),
                CartStorefrontAddLineItemRequest {
                    cart_id,
                    input: resolved_input.add_line_item,
                    pricing_adjustment: resolved_input.pricing_adjustment,
                },
            )
            .await
            .map_err(cart_port_error)?;
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
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Update storefront cart context")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
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

        let updated = cart_storefront_port
            .update_storefront_context(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "update-context",
                    true,
                ),
                CartStorefrontContextUpdateRequest {
                    cart_id,
                    input: crate::dto::UpdateCartContextInput {
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
                },
            )
            .await
            .map_err(cart_port_error)?;
        let updated = reprice_storefront_cart_line_items(
            db,
            tenant_id,
            request_context,
            event_bus,
            cart_storefront_port.as_ref(),
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
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Update storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
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
        let updated = if let Some((variant_id, product_id)) = cart
            .line_items
            .iter()
            .find(|item| item.id == line_id)
            .and_then(|item| {
                item.variant_id
                    .map(|variant_id| (variant_id, item.product_id))
            }) {
            let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
            let pricing_context = build_storefront_pricing_context(
                &cart,
                request_context,
                public_channel_slug.as_deref(),
                input.quantity,
            );
            let resolved_price: rustok_pricing::ResolvedPrice = pricing_read_port
                .resolve_product_price(
                    storefront_pricing_port_context(tenant_id, request_context, cart_id, line_id),
                    ResolveProductPriceRequest {
                        product_id,
                        variant_id,
                        region_id: pricing_context.region_id,
                        channel_id: pricing_context.channel_id,
                        channel_slug: pricing_context.channel_slug,
                        price_list_id: pricing_context.price_list_id,
                        quantity: pricing_context.quantity,
                        currency_code: pricing_context.currency_code,
                    },
                )
                .await
                .map_err(cart_port_error)?
                .into();

            let pricing_update =
                storefront_cart_pricing_update(line_id, input.quantity, &resolved_price);
            cart_storefront_port
                .update_storefront_line_item_pricing(
                    storefront_cart_port_context(
                        tenant_id,
                        request_context,
                        ctx.data_opt::<AuthContext>(),
                        cart_id,
                        "update-line-item",
                        true,
                    ),
                    CartStorefrontLineItemPricingRequest {
                        cart_id,
                        line_item_id: line_id,
                        quantity: input.quantity,
                        unit_price: pricing_update.unit_price,
                        pricing_adjustment: pricing_update.pricing_adjustment,
                    },
                )
                .await
                .map_err(cart_port_error)?
        } else {
            cart_storefront_port
                .update_storefront_line_item_quantity(
                    storefront_cart_port_context(
                        tenant_id,
                        request_context,
                        ctx.data_opt::<AuthContext>(),
                        cart_id,
                        "update-line-item",
                        true,
                    ),
                    CartStorefrontLineItemQuantityRequest {
                        cart_id,
                        line_item_id: line_id,
                        quantity: input.quantity,
                    },
                )
                .await
                .map_err(cart_port_error)?
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
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Remove storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let request_context = ctx.data::<RequestContext>()?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let updated = cart_storefront_port
            .remove_storefront_line_item(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "remove-line-item",
                    true,
                ),
                CartStorefrontRemoveLineItemRequest {
                    cart_id,
                    line_item_id: line_id,
                },
            )
            .await
            .map_err(cart_port_error)?;
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
}

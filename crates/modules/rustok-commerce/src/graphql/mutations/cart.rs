use async_graphql::{Context, Object, Result};
use rustok_api::{AuthContext, RequestContext, TenantContext, graphql::require_module_enabled};
use uuid::Uuid;

use crate::StoreContextService;
use rustok_cart::{
    CartStorefrontCreateRequest, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    in_process_cart_storefront_port,
};
use rustok_pricing::in_process_pricing_read_port;

use super::super::{MODULE_SLUG, current_tenant_scope, types::*};
use super::helpers::*;
use super::typed_shipping_option_helper::validate_selected_shipping_option;

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
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let product_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let runtime = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
            product_runtime.read_port(),
            in_process_pricing_read_port(
                db.clone(),
                event_bus.clone(),
                crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                    db.clone(),
                    event_bus.clone(),
                ),
            ),
            in_process_cart_storefront_port(db.clone()),
            rustok_inventory::in_process_inventory_reservation_port(db.clone(), event_bus.clone()),
        );
        let updated = runtime
            .add_line_item(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "add-line-item",
                    true,
                ),
                customer_id,
                tenant.default_locale.as_str(),
                crate::storefront_cart_runtime::StorefrontCartAddCommand {
                    cart_id,
                    variant_id: input.variant_id,
                    quantity: input.quantity,
                    expected_context: input.expected_context.map(Into::into),
                    expected_locale: input.expected_locale,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
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
        let context_input = crate::dto::UpdateCartContextInput {
            email,
            region_id: context.region.as_ref().map(|region| region.id),
            country_code: requested_country_code,
            locale_code: Some(context.locale.clone()),
            selected_shipping_option_id: requested_shipping_option_id,
            shipping_selections: requested_shipping_selections,
        };
        let target = cart.project_context(&context_input).map_err(|_| {
            cart_port_error(rustok_api::PortError::validation(
                "commerce.cart_context_invalid",
                "Cart context is invalid",
            ))
        })?;
        let write_context = storefront_cart_port_context(
            tenant_id,
            request_context,
            ctx.data_opt::<AuthContext>(),
            cart_id,
            "update-context",
            true,
        );
        let public_channel_slug =
            crate::storefront_channel::resolve_cart_channel(db, &write_context, &target)
                .await
                .map_err(cart_port_error)?;
        validate_selected_shipping_option(
            db,
            tenant_id,
            &target,
            context_input.selected_shipping_option_id,
            context_input.shipping_selections.as_deref(),
            &target.currency_code,
            public_channel_slug.as_deref(),
            target.locale_code.as_deref(),
            Some(tenant.default_locale.as_str()),
        )
        .await?;
        let pricing = in_process_pricing_read_port(
            db.clone(),
            event_bus.clone(),
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            ),
        );
        let command = crate::prepare_storefront_cart_context(
            pricing.as_ref(),
            write_context.clone(),
            &cart,
            context_input,
        )
        .await
        .map_err(super::typed_reprice_helper::cart_context_preparation_error)?;
        let updated = cart_storefront_port
            .update_storefront_context(write_context, command)
            .await
            .map_err(cart_port_error)?;
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
        let product_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let workflow = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
            product_runtime.read_port(),
            in_process_pricing_read_port(db.clone(), event_bus.clone(), product_runtime),
            cart_storefront_port,
            rustok_inventory::in_process_inventory_reservation_port(db.clone(), event_bus.clone()),
        );
        let updated = workflow
            .update_line_item_quantity(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "update-line-item",
                    true,
                ),
                customer_id,
                tenant.default_locale.as_str(),
                cart,
                line_id,
                input.quantity,
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
                    pricing_context: rustok_cart::CartPricingContextGuard::capture_removal(
                        &cart, line_id,
                    )
                    .map_err(cart_port_error)?,
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

use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, graphql::require_module_enabled};
use rustok_cart::{
    CartStorefrontReadRequest, PrepareCartCheckoutSnapshotRequest,
    bind_in_process_atomic_cart_checkout_with_pricing, in_process_cart_storefront_port,
};
use rustok_payment::PaymentService;
use uuid::Uuid;

use crate::{CheckoutService, ShippingProfileService};
use rustok_fulfillment::FulfillmentService;

use super::super::{MODULE_SLUG, current_tenant_scope, require_commerce_permission, types::*};
use super::helpers::*;

#[derive(Default)]
pub struct CommerceCheckoutMutation;

#[Object]
impl CommerceCheckoutMutation {
    async fn create_storefront_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontPaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id =
            current_tenant_scope(ctx, tenant_id, "Storefront payment collection creation")?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    input.cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest {
                    cart_id: input.cart_id,
                },
            )
            .await
            .map_err(cart_port_error)?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let cart = reprice_storefront_cart_line_items(
            db,
            tenant_id,
            request_context,
            event_bus,
            cart_storefront_port.as_ref(),
            cart,
        )
        .await?;
        let context = crate::StoreContextService::new(
            db.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
        )
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
        idempotency_key: String,
        input: CompleteStorefrontCheckoutInput,
    ) -> Result<GqlCompleteCheckout> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Storefront checkout")?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    input.cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest {
                    cart_id: input.cart_id,
                },
            )
            .await
            .map_err(cart_port_error)?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let actor_id = ctx
            .data_opt::<AuthContext>()
            .map(|auth| auth.user_id)
            .unwrap_or_else(Uuid::nil);

        let checkout_input = crate::dto::CompleteCheckoutInput {
            cart_id: input.cart_id,
            shipping_option_id: input.shipping_option_id,
            shipping_selections: input.shipping_selections.map(|items| {
                items
                    .into_iter()
                    .map(|item| crate::dto::CartShippingSelectionInput {
                        shipping_profile_slug: item.shipping_profile_slug,
                        seller_id: item.seller_id,
                        seller_scope: None,
                        selected_shipping_option_id: item.selected_shipping_option_id,
                    })
                    .collect()
            }),
            region_id: input.region_id,
            country_code: input.country_code,
            locale: input.locale,
            create_fulfillment: input.create_fulfillment.unwrap_or(true),
            metadata: parse_optional_metadata(input.metadata.as_deref())?,
        };
        let pricing_resolver = std::sync::Arc::new(crate::StorefrontCheckoutPricingResolver::new(
            db.clone(),
            event_bus.clone(),
            request_context.channel_id,
            request_context.channel_slug.clone(),
        ));
        let atomic_cart = bind_in_process_atomic_cart_checkout_with_pricing(
            db.clone(),
            PrepareCartCheckoutSnapshotRequest {
                cart_id: checkout_input.cart_id,
                region_id: checkout_input.region_id,
                country_code: checkout_input.country_code.clone(),
                locale_code: checkout_input.locale.clone(),
                selected_shipping_option_id: checkout_input.shipping_option_id,
                shipping_selections: checkout_input.shipping_selections.clone(),
            },
            pricing_resolver,
        );
        let checkout = CheckoutService::new(
            db.clone(),
            event_bus.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
            atomic_cart.port,
            std::sync::Arc::new(rustok_inventory::InventoryService::new(
                db.clone(),
                event_bus.clone(),
            )),
            std::sync::Arc::new(rustok_product::CatalogService::new(
                db.clone(),
                event_bus.clone(),
            )),
        );
        let response = crate::JournaledCheckoutService::new(checkout, db.clone())
            .with_atomic_cart_checkout_handle(atomic_cart.handle)
            .complete_checkout(tenant_id, actor_id, idempotency_key, checkout_input)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(response.into())
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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping option creation")?;

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping option update")?;

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping profile creation")?;

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping profile update")?;

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
        let tenant_id =
            current_tenant_scope(ctx, Some(tenant_id), "Shipping profile deactivation")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let profile = ShippingProfileService::new(db.clone())
            .deactivate_shipping_profile(tenant_id, id)
            .await?;

        Ok(profile.into())
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
        let tenant_id =
            current_tenant_scope(ctx, Some(tenant_id), "Shipping profile reactivation")?;

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping option deactivation")?;

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
        let tenant_id = current_tenant_scope(ctx, Some(tenant_id), "Shipping option reactivation")?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let option = FulfillmentService::new(db.clone())
            .reactivate_shipping_option(tenant_id, id)
            .await?;

        Ok(option.into())
    }
}

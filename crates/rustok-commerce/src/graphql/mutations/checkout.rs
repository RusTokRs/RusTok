use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, graphql::require_module_enabled};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_payment::{PaymentError, PaymentService};
use uuid::Uuid;

use crate::ShippingProfileService;
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
        .await
        .map_err(|error| {
            tracing::error!(
                error = ?error,
                tenant_id = %tenant_id,
                cart_id = %cart.id,
                operation = "resolve_store_context",
                "storefront payment collection context resolution failed"
            );
            async_graphql::Error::new("Store context is temporarily unavailable").extend_with(
                |_, extensions| {
                    extensions.set("code", "store_context_unavailable");
                    extensions.set("retryable", true);
                },
            )
        })?;

        let service = PaymentService::new(db.clone());
        if let Some(existing) = service
            .find_reusable_collection_by_cart(tenant_id, cart.id)
            .await
            .map_err(|error| {
                payment_collection_graphql_error(
                    tenant_id,
                    cart.id,
                    "find_reusable_collection_by_cart",
                    error,
                )
            })?
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
            .await
            .map_err(|error| {
                payment_collection_graphql_error(tenant_id, cart.id, "create_collection", error)
            })?;

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
        let runtime = crate::storefront_checkout_runtime::StorefrontCheckoutRuntime::new(
            db.clone(),
            event_bus.clone(),
        );
        let response = crate::services::storefront_staged_checkout_runtime::complete_storefront_checkout_input(
            &runtime,
            crate::graphql_runtime::payment_provider_registry_from_context(ctx),
            tenant_id,
            request_context,
            ctx.data_opt::<AuthContext>().cloned(),
            idempotency_key,
            checkout_input,
        )
        .await
        .map_err(storefront_checkout_graphql_error)?;

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

fn storefront_checkout_graphql_error(
    error: crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> async_graphql::Error {
    let code = error.public_code();
    let message = error.public_message();
    let retryable = error.retryable();
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn payment_collection_graphql_error(
    tenant_id: Uuid,
    cart_id: Uuid,
    operation: &'static str,
    error: PaymentError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        cart_id = %cart_id,
        operation,
        "storefront payment collection GraphQL operation failed"
    );
    let (code, message, retryable, reconciliation_required) = match error {
        PaymentError::Validation(_) => (
            "payment_request_invalid",
            "Payment collection request is invalid",
            false,
            false,
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            "payment_resource_not_found",
            "Payment resource was not found",
            false,
            false,
        ),
        PaymentError::InvalidTransition { .. } => (
            "payment_state_conflict",
            "Payment lifecycle conflicts with the requested operation",
            false,
            false,
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::ProviderConfiguration { .. } => (
            "payment_temporarily_unavailable",
            "Payment service is temporarily unavailable",
            true,
            false,
        ),
        PaymentError::ProviderRejected { .. } => (
            "payment_provider_rejected",
            "Payment provider rejected the requested operation",
            false,
            false,
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => (
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
            false,
            true,
        ),
        PaymentError::Database(_) => (
            "payment_storage_unavailable",
            "Payment service is temporarily unavailable",
            true,
            false,
        ),
    };
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
        extensions.set("reconciliation_required", reconciliation_required);
    })
}

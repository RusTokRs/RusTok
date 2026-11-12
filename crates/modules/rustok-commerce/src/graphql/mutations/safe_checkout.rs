mod checkout_boundary {
    use async_graphql::{Error, ErrorExtensions};
    use rustok_api::{PortContext, PortError, PortErrorKind};

    use crate::CommerceError;

    const CHECKOUT_ERROR_BOUNDARY: &str = "commerce_graphql_checkout";

    struct CheckoutServiceDiagnosticError;

    impl std::fmt::Debug for CheckoutServiceDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    #[derive(Clone)]
    pub(crate) enum BoundaryError {
        Graphql(Error),
        Public {
            message: String,
            code: &'static str,
            retryable: bool,
        },
    }

    impl From<Error> for BoundaryError {
        fn from(error: Error) -> Self {
            Self::Graphql(error)
        }
    }

    fn public_graphql_error(
        message: impl Into<String>,
        code: &'static str,
        retryable: bool,
    ) -> Error {
        Error::new(message).extend_with(|_, extensions| {
            extensions.set("code", code);
            extensions.set("retryable", retryable);
        })
    }

    fn commerce_error_envelope(
        error: &CommerceError,
    ) -> (String, &'static str, bool, &'static str) {
        match error {
            CommerceError::Validation(_) => (
                "Shipping profile request is invalid".to_string(),
                "SHIPPING_PROFILE_REQUEST_INVALID",
                false,
                "validation",
            ),
            CommerceError::InvalidPrice(_)
            | CommerceError::InvalidOptionCombination
            | CommerceError::NoVariants => (
                "Shipping profile request is invalid".to_string(),
                "SHIPPING_PROFILE_REQUEST_INVALID",
                false,
                "validation",
            ),
            CommerceError::ShippingProfileNotFound(_) => (
                "Shipping profile was not found".to_string(),
                "SHIPPING_PROFILE_NOT_FOUND",
                false,
                "not_found",
            ),
            CommerceError::DuplicateShippingProfileSlug(_) => (
                "Shipping profile conflicts with the current state".to_string(),
                "SHIPPING_PROFILE_STATE_CONFLICT",
                false,
                "conflict",
            ),
            CommerceError::Database(_) => (
                "Shipping profile service is temporarily unavailable".to_string(),
                "SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
            CommerceError::ProductNotFound(_)
            | CommerceError::VariantNotFound(_)
            | CommerceError::DuplicateHandle { .. }
            | CommerceError::DuplicateSku(_)
            | CommerceError::InsufficientInventory { .. }
            | CommerceError::CannotDeletePublished
            | CommerceError::Rich(_)
            | CommerceError::Core(_) => (
                "Shipping profile operation could not be completed safely".to_string(),
                "SHIPPING_PROFILE_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    fn shipping_option_port_error_envelope(
        error: &PortError,
    ) -> (String, &'static str, bool, &'static str) {
        match &error.kind {
            PortErrorKind::Validation => (
                "Shipping option request is invalid".to_string(),
                "SHIPPING_OPTION_REQUEST_INVALID",
                false,
                "validation",
            ),
            PortErrorKind::NotFound if error.code == "fulfillment.shipping_option_not_found" => (
                "Shipping option was not found".to_string(),
                "SHIPPING_OPTION_NOT_FOUND",
                false,
                "not_found",
            ),
            PortErrorKind::Conflict
                if error.code == "fulfillment.shipping_option_translation_revision_conflict" =>
            {
                (
                    "Shipping option translations changed; reload before saving".to_string(),
                    "SHIPPING_OPTION_TRANSLATION_CONFLICT",
                    false,
                    "translation_revision_conflict",
                )
            }
            PortErrorKind::Conflict => (
                "Shipping option operation conflicts with the current state".to_string(),
                "SHIPPING_OPTION_STATE_CONFLICT",
                false,
                "conflict",
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Shipping option service is temporarily unavailable".to_string(),
                "SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE",
                true,
                "temporarily_unavailable",
            ),
            PortErrorKind::NotFound
            | PortErrorKind::Forbidden
            | PortErrorKind::InvariantViolation => (
                "Shipping option operation could not be completed safely".to_string(),
                "SHIPPING_OPTION_OPERATION_FAILED",
                false,
                "unexpected_owner_error",
            ),
        }
    }

    impl From<CommerceError> for BoundaryError {
        fn from(error: CommerceError) -> Self {
            let (message, code, retryable, error_kind) = commerce_error_envelope(&error);
            let diagnostic_error = CheckoutServiceDiagnosticError;
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_commerce",
                error_kind,
                public_code = code,
                retryable,
                boundary = CHECKOUT_ERROR_BOUNDARY,
                "commerce GraphQL checkout shipping profile operation failed"
            );
            Self::Public {
                message,
                code,
                retryable,
            }
        }
    }

    pub(crate) fn shipping_option_port_error(
        context: &PortContext,
        owner_operation: &'static str,
        error: PortError,
    ) -> BoundaryError {
        let (message, code, retryable, error_kind) = shipping_option_port_error_envelope(&error);
        let diagnostic_error = CheckoutServiceDiagnosticError;
        tracing::error!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment.shipping_option_admin_command",
            owner_operation,
            correlation_id = %context.correlation_id,
            tenant_id_present = !context.tenant_id.is_empty(),
            actor_id_present = !context.actor.id.is_empty(),
            channel_present = context.channel.is_some(),
            locale_length = context.locale.chars().count(),
            deadline_ms = ?context.deadline_ms,
            owner_error_kind = ?error.kind,
            owner_code_length = error.code.chars().count(),
            error_kind,
            public_code = code,
            retryable,
            boundary = CHECKOUT_ERROR_BOUNDARY,
            "commerce GraphQL checkout shipping option owner command failed"
        );
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }

    impl From<BoundaryError> for Error {
        fn from(error: BoundaryError) -> Self {
            match error {
                BoundaryError::Graphql(error) => error,
                BoundaryError::Public {
                    message,
                    code,
                    retryable,
                } => public_graphql_error(message, code, retryable),
            }
        }
    }
}

mod async_graphql_shim {
    pub use async_graphql::{Context, Error, ErrorExtensions, Object};

    pub type Result<T> = std::result::Result<T, super::checkout_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;

use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::Permission;
use rustok_api::{
    AuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    graphql::require_module_enabled,
};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_fulfillment::{
    CreateAdminShippingOptionRequest, DeactivateAdminShippingOptionRequest,
    ReactivateAdminShippingOptionRequest, UpdateAdminShippingOptionRequest,
};
use rustok_payment::{
    PaymentCollectionCreateOrReuseRequest, ReusablePaymentCollectionByCartRequest,
};
use uuid::Uuid;

use crate::ShippingProfileService;

use super::super::{MODULE_SLUG, current_tenant_scope, require_commerce_permission, types::*};
use super::helpers::*;

fn shipping_option_command_context(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    shipping_option_id: Option<Uuid>,
    operation: &'static str,
) -> Result<PortContext> {
    let auth = ctx.data::<AuthContext>()?;
    let request = ctx.data_opt::<RequestContext>();
    let locale = request
        .map(|request| request.locale.as_str())
        .filter(|locale| !locale.trim().is_empty())
        .unwrap_or("und");
    let resource_id = shipping_option_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("commerce-graphql-shipping-option-command:{operation}:{resource_id}"),
    )
    .with_idempotency_key(Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    Ok(
        match request.and_then(|request| request.channel_slug.as_deref()) {
            Some(channel) => context.with_channel(channel),
            None => context,
        },
    )
}

fn storefront_payment_collection_actor(auth: Option<&AuthContext>) -> PortActor {
    auth.map(|auth| PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| {
            PortActor::service("rustok-commerce.graphql-storefront-payment-collection")
        })
}

fn storefront_payment_collection_locale(request: &RequestContext) -> &str {
    if request.locale.trim().is_empty() {
        "und"
    } else {
        request.locale.as_str()
    }
}

fn storefront_payment_collection_read_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request: &RequestContext,
    auth: Option<&AuthContext>,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        storefront_payment_collection_actor(auth),
        storefront_payment_collection_locale(request),
        format!("commerce-graphql-storefront-payment-collection-read:{cart_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn storefront_payment_collection_command_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request: &RequestContext,
    auth: Option<&AuthContext>,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        storefront_payment_collection_actor(auth),
        storefront_payment_collection_locale(request),
        format!("commerce-graphql-storefront-payment-collection:{cart_id}"),
    )
    .with_idempotency_key(format!("graphql-storefront-payment-collection:{cart_id}"))
    .with_deadline(std::time::Duration::from_secs(2));
    match request.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

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
        let store_context = crate::StoreContextService::new(
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

        let read_context = storefront_payment_collection_read_context(
            tenant_id,
            cart.id,
            request_context,
            ctx.data_opt::<AuthContext>(),
        );
        if let Some(existing) =
            crate::graphql_runtime::payment_read_runtime_for_current_graphql_scope(db.clone())
                .cart_read_port()
                .find_reusable_collection_by_cart(
                    read_context.clone(),
                    ReusablePaymentCollectionByCartRequest { cart_id: cart.id },
                )
                .await
                .map_err(|error| {
                    payment_collection_owner_graphql_error(
                        &read_context,
                        cart.id,
                        "find_reusable_collection_by_cart",
                        error,
                    )
                })?
        {
            return Ok(existing.into());
        }

        let command_context = storefront_payment_collection_command_context(
            tenant_id,
            cart.id,
            request_context,
            ctx.data_opt::<AuthContext>(),
        );
        let collection =
            crate::graphql_runtime::payment_command_runtime_from_context(ctx, db.clone())
                .collection_create_or_reuse_port()
                .create_or_reuse_collection(
                    command_context.clone(),
                    PaymentCollectionCreateOrReuseRequest {
                        cart_id: Some(cart.id),
                        order_id: None,
                        customer_id: cart.customer_id,
                        currency_code: cart.currency_code.clone(),
                        amount: cart.total_amount,
                        metadata: merge_graphql_metadata(
                            parse_optional_metadata(input.metadata.as_deref())?,
                            cart_context_metadata(&cart, &store_context),
                        ),
                    },
                )
                .await
                .map_err(|error| {
                    payment_collection_owner_graphql_error(
                        &command_context,
                        cart.id,
                        "create_or_reuse_collection",
                        error,
                    )
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
        )
        .with_product_catalog_read_runtime(
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            ),
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
        let request = CreateAdminShippingOptionRequest {
            input: crate::dto::CreateShippingOptionInput {
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
        };
        let command_context =
            shipping_option_command_context(ctx, tenant_id, None, "create_shipping_option")?;
        let option = crate::graphql_runtime::shipping_option_admin_command_runtime_from_context(
            ctx,
            db.clone(),
        )
        .command_port()
        .create_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            checkout_boundary::shipping_option_port_error(
                &command_context,
                "create_admin_shipping_option",
                error,
            )
        })?;

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
        let request = UpdateAdminShippingOptionRequest {
            shipping_option_id: id,
            input: crate::dto::UpdateShippingOptionInput {
                translations: input.translations.map(|translations| {
                    translations
                        .into_iter()
                        .map(|translation| crate::dto::ShippingOptionTranslationInput {
                            locale: translation.locale,
                            name: translation.name,
                        })
                        .collect()
                }),
                expected_translation_revision: input.expected_translation_revision,
                currency_code: input.currency_code,
                amount: parse_optional_decimal(input.amount.as_deref())?,
                provider_id: input.provider_id,
                allowed_shipping_profile_slugs: input.allowed_shipping_profile_slugs,
                metadata: input
                    .metadata
                    .as_deref()
                    .map(|value| {
                        serde_json::from_str(value)
                            .map_err(|_| async_graphql::Error::new("Invalid JSON metadata payload"))
                    })
                    .transpose()?,
            },
        };
        let command_context =
            shipping_option_command_context(ctx, tenant_id, Some(id), "update_shipping_option")?;
        let option = crate::graphql_runtime::shipping_option_admin_command_runtime_from_context(
            ctx,
            db.clone(),
        )
        .command_port()
        .update_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            checkout_boundary::shipping_option_port_error(
                &command_context,
                "update_admin_shipping_option",
                error,
            )
        })?;

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
        let request = DeactivateAdminShippingOptionRequest {
            shipping_option_id: id,
        };
        let command_context = shipping_option_command_context(
            ctx,
            tenant_id,
            Some(id),
            "deactivate_shipping_option",
        )?;
        let option = crate::graphql_runtime::shipping_option_admin_command_runtime_from_context(
            ctx,
            db.clone(),
        )
        .command_port()
        .deactivate_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            checkout_boundary::shipping_option_port_error(
                &command_context,
                "deactivate_admin_shipping_option",
                error,
            )
        })?;

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
        let request = ReactivateAdminShippingOptionRequest {
            shipping_option_id: id,
        };
        let command_context = shipping_option_command_context(
            ctx,
            tenant_id,
            Some(id),
            "reactivate_shipping_option",
        )?;
        let option = crate::graphql_runtime::shipping_option_admin_command_runtime_from_context(
            ctx,
            db.clone(),
        )
        .command_port()
        .reactivate_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            checkout_boundary::shipping_option_port_error(
                &command_context,
                "reactivate_admin_shipping_option",
                error,
            )
        })?;

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

fn payment_collection_port_graphql_policy(
    error: &PortError,
) -> (&'static str, &'static str, bool, bool, &'static str) {
    match &error.kind {
        PortErrorKind::Validation => (
            "payment_request_invalid",
            "Payment collection request is invalid",
            false,
            false,
            "validation",
        ),
        PortErrorKind::NotFound => (
            "payment_resource_not_found",
            "Payment resource was not found",
            false,
            false,
            "not_found",
        ),
        PortErrorKind::Conflict if error.code == "payment.provider_rejected" => (
            "payment_provider_rejected",
            "Payment provider rejected the requested operation",
            false,
            false,
            "provider_rejected",
        ),
        PortErrorKind::Conflict if error.code == "payment.provider_outcome_unknown" => (
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
            false,
            true,
            "provider_outcome_unknown",
        ),
        PortErrorKind::Conflict => (
            "payment_state_conflict",
            "Payment lifecycle conflicts with the requested operation",
            false,
            false,
            "state_conflict",
        ),
        PortErrorKind::Unavailable
            if error.code == "payment.database_unavailable"
                || error.code == "payment.cart_read_unavailable" =>
        {
            (
                "payment_storage_unavailable",
                "Payment service is temporarily unavailable",
                true,
                false,
                "database",
            )
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "payment_temporarily_unavailable",
            "Payment service is temporarily unavailable",
            true,
            false,
            "temporarily_unavailable",
        ),
        PortErrorKind::InvariantViolation if error.code == "payment.provider_invalid_response" => (
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
            false,
            true,
            "provider_invalid_response",
        ),
        PortErrorKind::InvariantViolation if error.code == "payment.provider_not_configured" => (
            "payment_temporarily_unavailable",
            "Payment service is temporarily unavailable",
            true,
            false,
            "provider_configuration",
        ),
        PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (
            "payment_operation_failed",
            "Payment operation could not be completed safely",
            false,
            false,
            "owner_operation_failed",
        ),
    }
}

fn payment_collection_owner_graphql_error(
    context: &PortContext,
    cart_id: Uuid,
    operation: &'static str,
    error: PortError,
) -> async_graphql::Error {
    let (code, message, retryable, reconciliation_required, error_kind) =
        payment_collection_port_graphql_policy(&error);
    tracing::error!(
        owner = "rustok_payment.payment_collection_port",
        operation,
        correlation_id = %context.correlation_id,
        tenant_id_present = !context.tenant_id.is_empty(),
        actor_kind = ?context.actor.kind,
        actor_id_present = !context.actor.id.is_empty(),
        cart_id_non_nil = !cart_id.is_nil(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        error_kind,
        public_code = code,
        retryable,
        reconciliation_required,
        boundary = "commerce_graphql_storefront_payment_collection",
        "commerce GraphQL storefront payment collection owner call failed"
    );
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
        extensions.set("reconciliation_required", reconciliation_required);
    })
}

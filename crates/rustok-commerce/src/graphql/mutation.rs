use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext, RequestContext, TenantContext,
};
use rustok_core::Permission;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    entities::{price, product, product_translation, product_variant, variant_translation},
    CartService, CatalogService, CheckoutService, CustomerService, FulfillmentService,
    OrderService, PaymentService, StoreContextService,
};

use super::{require_commerce_permission, types::*, MODULE_SLUG};

#[derive(Default)]
pub struct CommerceMutation;

#[Object]
impl CommerceMutation {
    async fn create_storefront_cart(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontCartInput,
    ) -> Result<GqlStoreCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;

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
            .create_cart(
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
        let resolved_input = resolve_storefront_line_item_input(
            db,
            tenant_id,
            &cart.currency_code,
            cart.locale_code
                .as_deref()
                .unwrap_or(request_context.locale.as_str()),
            input,
        )
        .await?;

        let updated = cart_service
            .add_line_item(tenant_id, cart_id, resolved_input)
            .await?;
        Ok(updated.into())
    }

    async fn update_storefront_cart_context(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        input: UpdateStorefrontCartContextInput,
    ) -> Result<GqlStoreCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;

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
            requested_shipping_option_id,
            &cart.currency_code,
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
                },
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
            .update_line_item_quantity(tenant_id, cart_id, line_id, input.quantity)
            .await?;
        Ok(updated.into())
    }

    async fn remove_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        line_id: Uuid,
    ) -> Result<GqlCart> {
        require_module_enabled(ctx, MODULE_SLUG).await?;

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
        Ok(updated.into())
    }

    async fn create_storefront_payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontPaymentCollectionInput,
    ) -> Result<GqlPaymentCollection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, input.cart_id).await?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
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

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let cart_service = CartService::new(db.clone());
        let cart = cart_service.get_cart(tenant_id, input.cart_id).await?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        ensure_storefront_cart_access(&cart, customer_id)?;
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
            vendor: input.vendor,
            product_type: input.product_type,
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
            name: option.name,
            values: option.values,
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
                        amount,
                        compare_at_amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(crate::dto::CreateVariantInput {
                sku: variant.sku,
                barcode: variant.barcode,
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
        vendor: input.vendor,
        product_type: input.product_type,
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
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    selected_shipping_option_id: Option<Uuid>,
    currency_code: &str,
) -> Result<()> {
    let Some(selected_shipping_option_id) = selected_shipping_option_id else {
        return Ok(());
    };

    let option = FulfillmentService::new(db.clone())
        .get_shipping_option(tenant_id, selected_shipping_option_id)
        .await?;
    if !option.currency_code.eq_ignore_ascii_case(currency_code) {
        return Err(async_graphql::Error::new(format!(
            "Shipping option {} uses currency {}, expected {}",
            option.id, option.currency_code, currency_code
        )));
    }

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
    currency_code: &str,
    locale: &str,
    input: AddStorefrontCartLineItemInput,
) -> Result<crate::dto::AddCartLineItemInput> {
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

    let product_translation_model = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .filter(product_translation::Column::Locale.eq(locale))
        .one(db)
        .await?;
    let fallback_translation_model = if product_translation_model.is_none() {
        product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.eq(product_model.id))
            .order_by_asc(product_translation::Column::Locale)
            .one(db)
            .await?
    } else {
        None
    };

    let variant_translation_model = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .filter(variant_translation::Column::Locale.eq(locale))
        .one(db)
        .await?;

    let selected_price = price::Entity::find()
        .filter(price::Column::VariantId.eq(variant.id))
        .filter(price::Column::CurrencyCode.eq(currency_code.to_ascii_uppercase()))
        .order_by_asc(price::Column::RegionId)
        .one(db)
        .await?
        .ok_or_else(|| {
            async_graphql::Error::new(format!(
                "No storefront price for variant {} in currency {}",
                variant.id, currency_code
            ))
        })?;

    let base_title = product_translation_model
        .as_ref()
        .or(fallback_translation_model.as_ref())
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match variant_translation_model.and_then(|translation| translation.title) {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(crate::dto::AddCartLineItemInput {
        product_id: Some(product_model.id),
        variant_id: Some(variant.id),
        sku: variant.sku.clone(),
        title,
        quantity: input.quantity,
        unit_price: selected_price.amount,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

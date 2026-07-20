use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::Permission;
use rustok_api::locale_tags_match;
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_fulfillment::FulfillmentService;
use rustok_order::OrderService;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentService;
use rustok_pricing::{
    ActivePriceListProjectionRequest, AdminProductPricingProjectionRequest, PricingReadPort,
    ResolveProductPriceRequest, StorefrontProductPricingProjectionRequest,
    in_process_pricing_read_port,
};
use rustok_product::services::catalog::helpers::product_channel_visibility_condition;
use rustok_product::{CatalogService, ProductCatalogSchemaService};
use rustok_region::{RegionListRequest, RegionReadPort, RegionService};
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    CommerceError, ShippingProfileService, StoreContextService,
    search::product_translation_title_search_condition,
    storefront_channel::{
        apply_public_channel_inventory_to_product, is_metadata_visible_for_public_channel,
        normalize_public_channel_slug, public_channel_slug_from_request,
    },
    storefront_shipping::{
        enrich_cart_delivery_groups, is_shipping_option_compatible_with_profiles,
        load_cart_shipping_profile_slugs, product_shipping_profile_slug,
    },
};
use rustok_product::entities::{product, product_translation};

use super::{
    MODULE_SLUG, PRODUCT_MODULE_SLUG, map_product_service_error, product_query_tenant,
    require_commerce_permission, types::*,
};

#[derive(Default)]
pub struct CommerceQuery;

fn first_non_empty(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

#[Object]
impl CommerceQuery {
    /// Pricing-authoritative admin product detail.
    ///
    /// Use this root when the caller needs raw scoped price rows or effective
    /// prices for an explicit currency/region/price-list/channel/quantity context.
    #[allow(clippy::too_many_arguments)]
    async fn admin_pricing_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
        currency_code: Option<String>,
        region_id: Option<Uuid>,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        quantity: Option<i32>,
    ) -> Result<Option<GqlPricingProductDetail>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let request_context = ctx.data_opt::<RequestContext>();
        let requested_locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());
        let selected_channel_id =
            channel_id.or_else(|| request_context.and_then(|item| item.channel_id));
        let selected_channel_slug = normalize_pricing_channel_slug(channel_slug.as_deref())
            .or_else(|| {
                request_context
                    .and_then(|item| normalize_pricing_channel_slug(item.channel_slug.as_deref()))
            });
        let resolution_context = build_pricing_resolution_context(
            currency_code,
            region_id,
            price_list_id,
            selected_channel_id,
            selected_channel_slug.clone(),
            quantity,
        )?;
        let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
        let detail = match pricing_read_port
            .read_admin_product_pricing_projection(
                pricing_admin_product_port_context(
                    tenant_id,
                    ctx.data_opt::<AuthContext>(),
                    requested_locale.as_str(),
                    resolution_context.as_ref(),
                    id,
                ),
                AdminProductPricingProjectionRequest {
                    product_id: id,
                    fallback_locale: Some(tenant.default_locale.clone()),
                    selected_price_list_id: resolution_context
                        .as_ref()
                        .and_then(|context| context.price_list_id),
                },
            )
            .await
        {
            Ok(detail) => Some(detail),
            Err(error) if error.kind == rustok_api::PortErrorKind::NotFound => None,
            Err(error) => return Err(async_graphql::Error::new(error.message)),
        };

        match detail {
            Some(detail) => {
                let mut detail = GqlPricingProductDetail::from(detail);
                if let Some(context) = resolution_context.as_ref() {
                    attach_effective_prices(
                        pricing_read_port.as_ref(),
                        tenant_id,
                        requested_locale.as_str(),
                        &mut detail,
                        context,
                    )
                    .await?;
                }
                Ok(Some(detail))
            }
            None => Ok(None),
        }
    }

    async fn storefront_pricing_channels(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<GqlPricingChannelOption>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if tenant_id.is_some_and(|requested_tenant_id| requested_tenant_id != tenant.id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Storefront catalog reads must use the current tenant",
            ));
        }
        let tenant_id = tenant.id;
        let channel_service = rustok_channel::ChannelService::new(db.clone());
        let (channels, _) = channel_service
            .list_channels(tenant_id, 1, 250)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(channels.into_iter().map(Into::into).collect())
    }

    async fn storefront_active_price_lists(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> Result<Vec<GqlActivePriceListOption>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if tenant_id.is_some_and(|requested_tenant_id| requested_tenant_id != tenant.id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Storefront catalog reads must use the current tenant",
            ));
        }
        let tenant_id = tenant.id;
        let request_context = ctx.data_opt::<RequestContext>();
        let selected_channel_id =
            channel_id.or_else(|| request_context.and_then(|item| item.channel_id));
        let selected_channel_slug = normalize_public_channel_slug(channel_slug.as_deref())
            .or_else(|| request_context.and_then(public_channel_slug_from_request));
        let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
        let items = pricing_read_port
            .list_active_price_list_projections(
                pricing_active_lists_port_context(
                    tenant_id,
                    request_context,
                    tenant.default_locale.as_str(),
                    selected_channel_slug.as_deref(),
                ),
                ActivePriceListProjectionRequest {
                    channel_id: selected_channel_id,
                    channel_slug: selected_channel_slug,
                    fallback_locale: Some(tenant.default_locale.clone()),
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.message))?;

        Ok(items.into_iter().map(Into::into).collect())
    }

    /// Pricing-authoritative published product detail for storefront consumers.
    ///
    /// Use this root when the caller needs effective prices for an explicit
    /// currency/region/price-list/channel/quantity context.
    #[allow(clippy::too_many_arguments)]
    async fn storefront_pricing_product(
        &self,
        ctx: &Context<'_>,
        handle: String,
        locale: Option<String>,
        currency_code: Option<String>,
        region_id: Option<Uuid>,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        quantity: Option<i32>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPricingProductDetail>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if tenant_id.is_some_and(|requested_tenant_id| requested_tenant_id != tenant.id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Storefront catalog reads must use the current tenant",
            ));
        }
        let tenant_id = tenant.id;
        let request_context = ctx.data_opt::<RequestContext>();
        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.map(|item| item.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let selected_channel_id =
            channel_id.or_else(|| request_context.and_then(|item| item.channel_id));
        let selected_channel_slug = normalize_pricing_channel_slug(channel_slug.as_deref())
            .or_else(|| request_public_channel_slug(ctx));
        let requested_handle = handle.trim().to_string();
        let resolution_context = build_pricing_resolution_context(
            currency_code,
            region_id,
            price_list_id,
            selected_channel_id,
            selected_channel_slug.clone(),
            quantity,
        )?;
        let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
        let detail = pricing_read_port
            .read_storefront_product_pricing_projection(
                pricing_storefront_product_port_context(
                    tenant_id,
                    requested_locale.as_str(),
                    selected_channel_slug.as_deref(),
                    requested_handle.as_str(),
                ),
                StorefrontProductPricingProjectionRequest {
                    handle: requested_handle,
                    fallback_locale: Some(tenant.default_locale.clone()),
                    public_channel_slug: selected_channel_slug.clone(),
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.message))?;

        match detail {
            Some(detail) => {
                let mut detail = GqlPricingProductDetail::from(detail);
                if let Some(context) = resolution_context.as_ref() {
                    attach_effective_prices(
                        pricing_read_port.as_ref(),
                        tenant_id,
                        requested_locale.as_str(),
                        &mut detail,
                        context,
                    )
                    .await?;
                }
                Ok(Some(detail))
            }
            None => Ok(None),
        }
    }

    async fn storefront_regions(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Vec<GqlRegion>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let requested_locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());
        let region_service = RegionService::new(db.clone());
        let regions = region_service
            .list_regions_for_tenant(
                rustok_api::PortContext::new(
                    tenant_id.to_string(),
                    rustok_api::PortActor::service("commerce.graphql-regions"),
                    requested_locale.as_str(),
                    format!("graphql-regions:{tenant_id}"),
                )
                .with_deadline(std::time::Duration::from_secs(3)),
                RegionListRequest {
                    requested_locale: Some(requested_locale),
                    tenant_default_locale: Some(tenant.default_locale.clone()),
                },
            )
            .await
            .map_err(|error| {
                async_graphql::Error::new(format!("{}: {}", error.code, error.message))
            })?;

        Ok(regions
            .into_iter()
            .map(|projection| projection.region.into())
            .collect())
    }

    async fn storefront_shipping_options(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        filter: Option<StorefrontContextFilter>,
    ) -> Result<Vec<GqlShippingOption>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let filter = filter.unwrap_or(StorefrontContextFilter {
            cart_id: None,
            region_id: None,
            country_code: None,
            locale: None,
            currency_code: None,
        });
        let (context, public_channel_slug, required_shipping_profiles) =
            if let Some(cart_id) = filter.cart_id {
                let cart = in_process_cart_storefront_port(db.clone())
                    .read_storefront_cart(
                        graphql_cart_port_context(tenant_id, cart_id),
                        CartStorefrontReadRequest { cart_id },
                    )
                    .await
                    .map_err(|error| async_graphql::Error::new(error.message))?;
                ensure_storefront_cart_access(&cart, customer_id)?;
                let required_shipping_profiles =
                    load_cart_shipping_profile_slugs(db, tenant_id, &cart).await?;
                (
                    resolve_storefront_context(
                        db,
                        ctx,
                        tenant_id,
                        cart.region_id,
                        cart.country_code.clone(),
                        cart.locale_code.clone(),
                        Some(cart.currency_code.clone()),
                    )
                    .await?,
                    storefront_public_channel_slug_for_cart(&cart, ctx),
                    required_shipping_profiles,
                )
            } else {
                (
                    resolve_storefront_context(
                        db,
                        ctx,
                        tenant_id,
                        filter.region_id,
                        filter.country_code,
                        filter.locale,
                        filter.currency_code,
                    )
                    .await?,
                    request_public_channel_slug(ctx),
                    Default::default(),
                )
            };

        let mut options = FulfillmentService::new(db.clone())
            .list_shipping_options(
                tenant_id,
                Some(context.locale.as_str()),
                Some(context.default_locale.as_str()),
            )
            .await?;
        if let Some(currency_code) = context.currency_code.as_deref() {
            options.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
        }
        options.retain(|option| {
            is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug.as_deref())
                && is_shipping_option_compatible_with_profiles(option, &required_shipping_profiles)
        });

        Ok(options.into_iter().map(Into::into).collect())
    }

    async fn storefront_me(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCustomer> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let customer = in_process_customer_read_port(db.clone())
            .read_customer_projection_by_user(
                graphql_customer_port_context(tenant_id, auth.user_id),
                CustomerUserProjectionRequest {
                    user_id: auth.user_id,
                },
            )
            .await
            .map_err(|error| match error.code.as_str() {
                "customer.customer_by_user_not_found" => {
                    <FieldError as GraphQLError>::unauthenticated()
                }
                _ => async_graphql::Error::new(error.message),
            })?;

        Ok(customer.into())
    }

    async fn storefront_returns(
        &self,
        ctx: &Context<'_>,
        order_id: Uuid,
        tenant_id: Option<Uuid>,
        filter: Option<StorefrontReturnsFilter>,
    ) -> Result<GqlOrderReturnList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());

        let filter = filter.unwrap_or(StorefrontReturnsFilter {
            status: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);

        let Some(_order) = load_storefront_customer_order(
            db,
            event_bus,
            tenant,
            ctx,
            tenant_id,
            order_id,
            locale.as_str(),
        )
        .await?
        else {
            return Ok(GqlOrderReturnList {
                items: Vec::new(),
                total: 0,
                page,
                per_page,
                has_next: false,
            });
        };

        let (items, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_returns(
                tenant_id,
                crate::dto::ListOrderReturnsInput {
                    page,
                    per_page,
                    order_id: Some(order_id),
                    status: filter.status,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlOrderReturnList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn storefront_refunds(
        &self,
        ctx: &Context<'_>,
        order_id: Uuid,
        tenant_id: Option<Uuid>,
        filter: Option<StorefrontRefundsFilter>,
    ) -> Result<GqlRefundList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());

        let filter = filter.unwrap_or(StorefrontRefundsFilter {
            status: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);

        let Some(_order) = load_storefront_customer_order(
            db,
            event_bus,
            tenant,
            ctx,
            tenant_id,
            order_id,
            locale.as_str(),
        )
        .await?
        else {
            return Ok(GqlRefundList {
                items: Vec::new(),
                total: 0,
                page,
                per_page,
                has_next: false,
            });
        };

        let (items, total) = PaymentService::new(db.clone())
            .list_refunds(
                tenant_id,
                crate::dto::ListRefundsInput {
                    page,
                    per_page,
                    payment_collection_id: None,
                    order_id: Some(order_id),
                    status: filter.status,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlRefundList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn storefront_order_changes(
        &self,
        ctx: &Context<'_>,
        order_id: Uuid,
        tenant_id: Option<Uuid>,
        filter: Option<StorefrontOrderChangesFilter>,
    ) -> Result<GqlOrderChangeList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());

        let filter = filter.unwrap_or(StorefrontOrderChangesFilter {
            status: None,
            change_type: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);

        let Some(_order) = load_storefront_customer_order(
            db,
            event_bus,
            tenant,
            ctx,
            tenant_id,
            order_id,
            locale.as_str(),
        )
        .await?
        else {
            return Ok(GqlOrderChangeList {
                items: Vec::new(),
                total: 0,
                page,
                per_page,
                has_next: false,
            });
        };

        let (items, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_order_changes(
                tenant_id,
                crate::dto::ListOrderChangesInput {
                    page,
                    per_page,
                    order_id: Some(order_id),
                    status: filter.status,
                    change_type: filter.change_type,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlOrderChangeList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn storefront_order(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlOrder>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());

        let order = load_storefront_customer_order(
            db,
            event_bus,
            tenant,
            ctx,
            tenant_id,
            id,
            locale.as_str(),
        )
        .await?;

        Ok(order.map(Into::into))
    }

    async fn storefront_cart(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlCart>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart = match in_process_cart_storefront_port(db.clone())
            .read_storefront_cart(
                graphql_cart_port_context(tenant_id, id),
                CartStorefrontReadRequest { cart_id: id },
            )
            .await
        {
            Ok(cart) => cart,
            Err(error) if error.code == "cart.cart_not_found" => return Ok(None),
            Err(error) => return Err(error.message.into()),
        };

        ensure_storefront_cart_access(&cart, customer_id)?;
        let request_context = ctx.data::<RequestContext>()?;
        let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, ctx)
            .or_else(|| public_channel_slug_from_request(request_context));
        let cart = enrich_cart_delivery_groups(
            db,
            tenant_id,
            cart,
            public_channel_slug.as_deref(),
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await?;
        Ok(Some(cart.into()))
    }

    async fn storefront_payment_collection(
        &self,
        ctx: &Context<'_>,
        cart_id: Uuid,
    ) -> Result<Option<GqlPaymentCollection>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant.id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart = match in_process_cart_storefront_port(db.clone())
            .read_storefront_cart(
                graphql_cart_port_context(tenant.id, cart_id),
                CartStorefrontReadRequest { cart_id },
            )
            .await
        {
            Ok(cart) => cart,
            Err(error) if error.code == "cart.cart_not_found" => return Ok(None),
            Err(error) => return Err(error.message.into()),
        };
        ensure_storefront_cart_access(&cart, customer_id)?;

        PaymentService::new(db.clone())
            .find_reusable_collection_by_cart(tenant.id, cart.id)
            .await
            .map(|collection| collection.map(Into::into))
            .map_err(|err| err.to_string().into())
    }

    async fn order(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlAdminOrderDetail>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());

        let order = match OrderService::new(db.clone(), event_bus.clone())
            .get_order_with_locale_fallback(
                tenant_id,
                id,
                locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(order) => order,
            Err(rustok_order::error::OrderError::OrderNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };
        let payment_collection = PaymentService::new(db.clone())
            .find_latest_collection_by_order(tenant_id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        let fulfillment = FulfillmentService::new(db.clone())
            .find_by_order(tenant_id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(Some(GqlAdminOrderDetail {
            order: order.into(),
            payment_collection: payment_collection.map(Into::into),
            fulfillment: fulfillment.map(Into::into),
        }))
    }

    async fn orders(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<OrdersFilter>,
    ) -> Result<GqlOrderList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_LIST],
            "Permission denied: orders:list required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        let filter = filter.unwrap_or(OrdersFilter {
            status: None,
            customer_id: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (orders, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_orders_with_locale_fallback(
                tenant_id,
                crate::dto::ListOrdersInput {
                    page,
                    per_page,
                    status: filter.status,
                    customer_id: filter.customer_id,
                },
                locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlOrderList {
            items: orders.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn order_change(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlOrderChange>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let item = match OrderService::new(db.clone(), event_bus.clone())
            .get_order_change(tenant_id, id)
            .await
        {
            Ok(item) => item,
            Err(rustok_order::error::OrderError::OrderChangeNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(item.into()))
    }

    async fn order_changes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<OrderChangesFilter>,
    ) -> Result<GqlOrderChangeList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let filter = filter.unwrap_or(OrderChangesFilter {
            order_id: None,
            status: None,
            change_type: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_order_changes(
                tenant_id,
                crate::dto::ListOrderChangesInput {
                    page,
                    per_page,
                    order_id: filter.order_id,
                    status: filter.status,
                    change_type: filter.change_type,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlOrderChangeList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn order_return(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlOrderReturn>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let item = match OrderService::new(db.clone(), event_bus.clone())
            .get_return(tenant_id, id)
            .await
        {
            Ok(item) => item,
            Err(rustok_order::error::OrderError::OrderReturnNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(item.into()))
    }

    async fn order_returns(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<OrderReturnsFilter>,
    ) -> Result<GqlOrderReturnList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let filter = filter.unwrap_or(OrderReturnsFilter {
            order_id: None,
            status: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_returns(
                tenant_id,
                crate::dto::ListOrderReturnsInput {
                    page,
                    per_page,
                    order_id: filter.order_id,
                    status: filter.status,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlOrderReturnList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn payment_collection(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlPaymentCollection>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ],
            "Permission denied: payments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let collection = match PaymentService::new(db.clone())
            .get_collection(tenant_id, id)
            .await
        {
            Ok(collection) => collection,
            Err(rustok_payment::error::PaymentError::PaymentCollectionNotFound(_)) => {
                return Ok(None);
            }
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(collection.into()))
    }

    async fn payment_collections(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<PaymentCollectionsFilter>,
    ) -> Result<GqlPaymentCollectionList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ],
            "Permission denied: payments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let filter = filter.unwrap_or(PaymentCollectionsFilter {
            status: None,
            order_id: None,
            cart_id: None,
            customer_id: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = PaymentService::new(db.clone())
            .list_collections(
                tenant_id,
                crate::dto::ListPaymentCollectionsInput {
                    page,
                    per_page,
                    status: filter.status,
                    order_id: filter.order_id,
                    cart_id: filter.cart_id,
                    customer_id: filter.customer_id,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlPaymentCollectionList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn refund(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlRefund>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ],
            "Permission denied: payments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let refund = match PaymentService::new(db.clone())
            .get_refund(tenant_id, id)
            .await
        {
            Ok(refund) => refund,
            Err(rustok_payment::error::PaymentError::RefundNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(refund.into()))
    }

    async fn refunds(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<RefundsFilter>,
    ) -> Result<GqlRefundList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ],
            "Permission denied: payments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let filter = filter.unwrap_or(RefundsFilter {
            payment_collection_id: None,
            order_id: None,
            status: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = PaymentService::new(db.clone())
            .list_refunds(
                tenant_id,
                crate::dto::ListRefundsInput {
                    page,
                    per_page,
                    payment_collection_id: filter.payment_collection_id,
                    order_id: filter.order_id,
                    status: filter.status,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlRefundList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn shipping_option(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlShippingOption>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        let option = match FulfillmentService::new(db.clone())
            .get_shipping_option(
                tenant_id,
                id,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(option) => option,
            Err(rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_)) => {
                return Ok(None);
            }
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(option.into()))
    }

    async fn shipping_profile(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlShippingProfile>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        let profile = match ShippingProfileService::new(db.clone())
            .get_shipping_profile(
                tenant_id,
                id,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(profile) => profile,
            Err(CommerceError::ShippingProfileNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(profile.into()))
    }

    async fn shipping_options(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<ShippingOptionsFilter>,
    ) -> Result<GqlShippingOptionList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        let filter = filter.unwrap_or(ShippingOptionsFilter {
            active: None,
            currency_code: None,
            provider_id: None,
            search: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let mut items = FulfillmentService::new(db.clone())
            .list_all_shipping_options(
                tenant_id,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        if let Some(active) = filter.active {
            items.retain(|option| option.active == active);
        }
        if let Some(currency_code) = filter.currency_code.as_deref() {
            items.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
        }
        if let Some(provider_id) = filter.provider_id.as_deref() {
            items.retain(|option| option.provider_id.eq_ignore_ascii_case(provider_id));
        }
        if let Some(search) = filter.search.as_deref() {
            let search = search.trim().to_ascii_lowercase();
            if !search.is_empty() {
                items.retain(|option| option.name.to_ascii_lowercase().contains(&search));
            }
        }
        let total = items.len() as u64;
        let offset = (page - 1) * per_page;
        let paged = items
            .into_iter()
            .skip(offset as usize)
            .take(per_page as usize)
            .map(Into::into)
            .collect();

        Ok(GqlShippingOptionList {
            items: paged,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn shipping_profiles(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<ShippingProfilesFilter>,
    ) -> Result<GqlShippingProfileList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_commerce_graphql_locale(ctx, None, tenant.default_locale.as_str());
        let filter = filter.unwrap_or(ShippingProfilesFilter {
            active: None,
            search: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = ShippingProfileService::new(db.clone())
            .list_shipping_profiles(
                tenant_id,
                crate::dto::ListShippingProfilesInput {
                    page,
                    per_page,
                    active: filter.active,
                    search: filter.search,
                    locale: Some(locale.clone()),
                },
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlShippingProfileList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn fulfillment(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GqlFulfillment>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let fulfillment = match FulfillmentService::new(db.clone())
            .get_fulfillment(tenant_id, id)
            .await
        {
            Ok(fulfillment) => fulfillment,
            Err(rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_)) => {
                return Ok(None);
            }
            Err(err) => return Err(err.to_string().into()),
        };

        Ok(Some(fulfillment.into()))
    }

    async fn fulfillments(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<FulfillmentsFilter>,
    ) -> Result<GqlFulfillmentList> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::FULFILLMENTS_READ],
            "Permission denied: fulfillments:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let filter = filter.unwrap_or(FulfillmentsFilter {
            status: None,
            order_id: None,
            customer_id: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (items, total) = FulfillmentService::new(db.clone())
            .list_fulfillments(
                tenant_id,
                crate::dto::ListFulfillmentsInput {
                    page,
                    per_page,
                    status: filter.status,
                    order_id: filter.order_id,
                    customer_id: filter.customer_id,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(GqlFulfillmentList {
            items: items.into_iter().map(Into::into).collect(),
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    /// Catalog-authoritative admin product detail.
    ///
    /// Variant `prices` here are compatibility snapshots for catalog/product
    /// consumers; pricing-authoritative reads live under `adminPricingProduct`.
    async fn product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlProduct>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        super::require_storefront_channel_enabled(ctx).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());

        let service = CatalogService::new(db.clone(), event_bus.clone());
        let product = match service
            .get_product_with_locale_fallback(
                tenant_id,
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(product) => product,
            Err(CommerceError::ProductNotFound(_)) => return Ok(None),
            Err(err) => return Err(map_product_service_error(err, "product_query")),
        };

        Ok(Some(
            localized_product_response(product, &locale, tenant.default_locale.as_str()).into(),
        ))
    }

    async fn products(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: Option<String>,
        filter: Option<ProductsFilter>,
    ) -> Result<GqlProductList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_LIST],
            "Permission denied: products:list required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());
        let filter = filter.unwrap_or(ProductsFilter {
            status: None,
            vendor: None,
            search: None,
            page: Some(1),
            per_page: Some(20),
        });
        let requested_limit = filter.per_page;
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = product::Entity::find().filter(product::Column::TenantId.eq(tenant_id));

        if let Some(status) = &filter.status {
            let status: rustok_product::entities::product::ProductStatus = (*status).into();
            query = query.filter(product::Column::Status.eq(status));
        }
        if let Some(vendor) = &filter.vendor {
            query = query.filter(product::Column::Vendor.eq(vendor));
        }
        if let Some(search) = &filter.search {
            query = query.filter(product_translation_title_search_condition(
                db.get_database_backend(),
                &locale,
                search,
            ));
        }

        let total = query.clone().count(db).await?;
        let products = query
            .order_by_desc(product::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(db)
            .await?;

        let items = load_product_list_items(
            db,
            event_bus,
            tenant_id,
            products,
            &locale,
            tenant.default_locale.as_str(),
            product_list_path("commerce.products"),
        )
        .await?;

        metrics::record_read_path_budget(
            "graphql",
            "commerce.products",
            requested_limit,
            per_page,
            items.len(),
        );

        Ok(GqlProductList {
            items,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    async fn product_attributes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: String,
    ) -> Result<GqlProductAttributeList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        let items = service
            .list_attributes(tenant_id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();

        Ok(GqlProductAttributeList {
            total: items.len() as u64,
            items,
        })
    }

    async fn catalog_categories(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: String,
    ) -> Result<GqlCatalogCategoryList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        let items = service
            .list_categories(tenant_id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();

        Ok(GqlCatalogCategoryList {
            total: items.len() as u64,
            items,
        })
    }

    async fn product_attribute_schemas(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: String,
    ) -> Result<GqlProductAttributeSchemaList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        let items = service
            .list_schemas(tenant_id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();

        Ok(GqlProductAttributeSchemaList {
            total: items.len() as u64,
            items,
        })
    }

    async fn product_effective_form(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        product_id: Option<Uuid>,
        category_id: Option<Uuid>,
        locale: String,
    ) -> Result<Option<GqlProductEffectiveForm>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        let locale = locale.trim();
        let form = match (product_id, category_id) {
            (Some(product_id), _) => service
                .load_effective_form_for_product(tenant_id, product_id)
                .await
                .map_err(|error| map_product_service_error(error, "product_query"))?,
            (None, Some(category_id)) => Some(
                service
                    .load_effective_form_for_category(tenant_id, category_id, &[])
                    .await
                    .map_err(|error| map_product_service_error(error, "product_query"))?,
            ),
            (None, None) => {
                return Err(async_graphql::Error::new(
                    "Either product_id or category_id is required",
                ));
            }
        };
        let Some(form) = form else {
            return Ok(None);
        };
        let group_labels = service
            .load_effective_form_group_labels(tenant_id, form.category_id, locale)
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?;

        let definitions = service
            .list_attributes(tenant_id, locale)
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .map(|attribute| (attribute.id, attribute))
            .collect::<HashMap<_, _>>();
        let effective_attribute_ids = form
            .attributes
            .iter()
            .map(|binding| binding.attribute_id)
            .collect::<Vec<_>>();
        let mut options_by_attribute = service
            .list_attribute_options(tenant_id, &effective_attribute_ids, locale)
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<GqlProductAttributeOption>>::new(),
                |mut map, option| {
                    map.entry(option.attribute_id)
                        .or_default()
                        .push(GqlProductAttributeOption {
                            id: option.id,
                            code: option.code,
                            label: option.label,
                            position: option.position,
                        });
                    map
                },
            );

        let attributes = form
            .attributes
            .into_iter()
            .map(|binding| {
                let definition = definitions.get(&binding.attribute_id).ok_or_else(|| {
                    async_graphql::Error::new(format!(
                        "attribute definition {} is missing",
                        binding.attribute_id
                    ))
                })?;
                Ok(GqlProductEffectiveFormAttribute {
                    attribute_id: binding.attribute_id,
                    code: definition.code.clone(),
                    label: definition.label.clone(),
                    value_type: definition.value_type.as_str().to_string(),
                    is_localized: definition.is_localized,
                    options: options_by_attribute
                        .remove(&binding.attribute_id)
                        .unwrap_or_default(),
                    group_label: binding
                        .group_code
                        .as_ref()
                        .and_then(|code| group_labels.get(code).cloned()),
                    group_code: binding.group_code,
                    is_required: binding.is_required,
                    is_disabled: binding.is_disabled,
                    position: binding.position,
                    source: effective_attribute_source_name(binding.source).to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(GqlProductEffectiveForm {
            category_id: form.category_id,
            attributes,
            detached_attribute_ids: form.detached_attribute_ids,
        }))
    }

    async fn product_attribute_values(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: String,
    ) -> Result<Vec<GqlProductAttributeValue>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .load_product_attribute_values(tenant_id, product_id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))
            .map(|items| items.into_iter().map(Into::into).collect())
    }

    async fn storefront_catalog_search_options(
        &self,
        ctx: &Context<'_>,
        locale: String,
    ) -> Result<GqlProductCatalogSearchOptions> {
        require_module_enabled(ctx, "product").await?;
        super::require_storefront_channel_enabled(ctx).await?;
        if locale.trim().is_empty() {
            return Err(async_graphql::Error::new("locale is required"));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        let category_options = service
            .list_categories(tenant.id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .map(|category| GqlProductCatalogSearchOption {
                value: category.id.to_string(),
                label: first_non_empty([category.path, category.name, category.code]),
            })
            .collect();
        let attribute_options = service
            .list_attributes(tenant.id, locale.trim())
            .await
            .map_err(|error| map_product_service_error(error, "product_query"))?
            .into_iter()
            .filter(|attribute| attribute.is_filterable || attribute.is_sortable)
            .map(|attribute| {
                let label = first_non_empty([attribute.label, attribute.code.clone()]);
                GqlProductCatalogSearchOption {
                    value: attribute.code.clone(),
                    label: format!("{label} ({})", attribute.code),
                }
            })
            .collect();

        Ok(GqlProductCatalogSearchOptions {
            category_options,
            attribute_options,
        })
    }

    /// Catalog-authoritative published product detail.
    ///
    /// Variant `prices` here are compatibility snapshots for catalog/product
    /// consumers; pricing-authoritative reads live under `storefrontPricingProduct`.
    async fn storefront_product(
        &self,
        ctx: &Context<'_>,
        id: Option<Uuid>,
        handle: Option<String>,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlProduct>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if tenant_id.is_some_and(|requested_tenant_id| requested_tenant_id != tenant.id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Storefront catalog reads must use the current tenant",
            ));
        }
        let tenant_id = tenant.id;
        let locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());

        let public_channel_slug = request_public_channel_slug(ctx);
        let product_id = match (id, handle.as_deref().map(str::trim)) {
            (Some(id), _) => Some(id),
            (None, Some(handle)) if !handle.is_empty() => {
                find_published_product_id_by_handle(
                    db,
                    tenant_id,
                    handle,
                    &locale,
                    tenant.default_locale.as_str(),
                    public_channel_slug.as_deref(),
                )
                .await?
            }
            _ => {
                return Err(async_graphql::Error::new(
                    "Either `id` or non-empty `handle` is required",
                ));
            }
        };

        let Some(product_id) = product_id else {
            return Ok(None);
        };

        let service = CatalogService::new(db.clone(), event_bus.clone());
        let mut product = match service
            .get_product_with_locale_fallback(
                tenant_id,
                product_id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(product) => product,
            Err(CommerceError::ProductNotFound(_)) => return Ok(None),
            Err(err) => return Err(map_product_service_error(err, "product_query")),
        };

        if product.status != rustok_product::entities::product::ProductStatus::Active
            || product.published_at.is_none()
            || !is_metadata_visible_for_public_channel(
                &product.metadata,
                public_channel_slug.as_deref(),
            )
        {
            return Ok(None);
        }

        apply_public_channel_inventory_to_product(
            db,
            tenant_id,
            &mut product,
            public_channel_slug.as_deref(),
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(Some(
            localized_product_response(product, &locale, tenant.default_locale.as_str()).into(),
        ))
    }

    async fn storefront_products(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
        filter: Option<StorefrontProductsFilter>,
    ) -> Result<GqlProductList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if tenant_id.is_some_and(|requested_tenant_id| requested_tenant_id != tenant.id) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Storefront catalog reads must use the current tenant",
            ));
        }
        let tenant_id = tenant.id;
        let locale =
            resolve_commerce_graphql_locale(ctx, locale.as_deref(), tenant.default_locale.as_str());
        let filter = filter.unwrap_or(StorefrontProductsFilter {
            vendor: None,
            product_type: None,
            search: None,
            page: Some(1),
            per_page: Some(12),
        });
        let requested_limit = filter.per_page;
        let page = filter.page.unwrap_or(1);
        let per_page = filter.per_page.unwrap_or(12);
        if page == 0 || per_page == 0 || per_page > 48 {
            return Err(async_graphql::Error::new(
                "page must be at least 1 and per_page must be between 1 and 48",
            ));
        }
        let offset = (page.saturating_sub(1)) * per_page;
        let public_channel_slug = request_public_channel_slug(ctx);

        let mut query = product::Entity::find()
            .filter(product::Column::TenantId.eq(tenant_id))
            .filter(
                product::Column::Status
                    .eq(rustok_product::entities::product::ProductStatus::Active),
            )
            .filter(product::Column::PublishedAt.is_not_null())
            .filter(product_channel_visibility_condition(
                db.get_database_backend(),
                public_channel_slug.as_deref(),
            ));

        if let Some(vendor) = &filter.vendor {
            query = query.filter(product::Column::Vendor.eq(vendor));
        }
        if let Some(product_type) = &filter.product_type {
            query = query.filter(product::Column::ProductType.eq(product_type));
        }
        if let Some(search) = &filter.search {
            query = query.filter(product_translation_title_search_condition(
                db.get_database_backend(),
                &locale,
                search,
            ));
        }

        let total = query.clone().count(db).await?;
        let products = query
            .order_by_desc(product::Column::PublishedAt)
            .order_by_desc(product::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(db)
            .await?;

        let items = load_product_list_items(
            db,
            event_bus,
            tenant_id,
            products,
            &locale,
            tenant.default_locale.as_str(),
            product_list_path("commerce.storefront_products"),
        )
        .await?;

        metrics::record_read_path_budget(
            "graphql",
            "commerce.storefront_products",
            requested_limit,
            per_page,
            items.len(),
        );

        Ok(GqlProductList {
            items,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }
}

async fn load_storefront_customer_order(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant: &TenantContext,
    ctx: &Context<'_>,
    tenant_id: Uuid,
    order_id: Uuid,
    locale: &str,
) -> Result<Option<rustok_order::dto::OrderResponse>> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let customer = in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            graphql_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
        .map_err(|error| match error.code.as_str() {
            "customer.customer_by_user_not_found" => {
                <FieldError as GraphQLError>::unauthenticated()
            }
            _ => async_graphql::Error::new(error.message),
        })?;

    let order = match OrderService::new(db.clone(), event_bus.clone())
        .get_order_with_locale_fallback(
            tenant_id,
            order_id,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
    {
        Ok(order) => order,
        Err(rustok_order::error::OrderError::OrderNotFound(_)) => return Ok(None),
        Err(err) => return Err(err.to_string().into()),
    };

    if order.customer_id != Some(customer.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Order does not belong to the current customer",
        ));
    }

    Ok(Some(order))
}

fn normalize_pricing_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn build_pricing_resolution_context(
    currency_code: Option<String>,
    region_id: Option<Uuid>,
    price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<rustok_pricing::PriceResolutionContext>> {
    let currency_code = match currency_code.map(|value| value.trim().to_ascii_uppercase()) {
        Some(value) if value.len() == 3 && value.chars().all(|ch| ch.is_ascii_alphabetic()) => {
            value
        }
        Some(_) => {
            return Err(async_graphql::Error::new(
                "currency_code must be a 3-letter code",
            ));
        }
        None if region_id.is_some() || price_list_id.is_some() || quantity.is_some() => {
            return Err(async_graphql::Error::new(
                "currency_code is required for pricing resolution context",
            ));
        }
        None => return Ok(None),
    };
    let quantity = match quantity {
        Some(value) if value < 1 => {
            return Err(async_graphql::Error::new("quantity must be at least 1"));
        }
        Some(value) => value,
        None => 1,
    };

    Ok(Some(rustok_pricing::PriceResolutionContext {
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug: normalize_pricing_channel_slug(channel_slug.as_deref()),
        quantity: Some(quantity),
    }))
}

async fn attach_effective_prices(
    pricing_read_port: &dyn PricingReadPort,
    tenant_id: Uuid,
    locale: &str,
    detail: &mut GqlPricingProductDetail,
    context: &rustok_pricing::PriceResolutionContext,
) -> Result<()> {
    for variant in &mut detail.variants {
        let effective_price = match pricing_read_port
            .resolve_product_price(
                pricing_query_port_context(tenant_id, locale, context, detail.id, variant.id),
                ResolveProductPriceRequest {
                    product_id: Some(detail.id),
                    variant_id: variant.id,
                    region_id: context.region_id,
                    channel_id: context.channel_id,
                    channel_slug: context.channel_slug.clone(),
                    price_list_id: context.price_list_id,
                    quantity: context.quantity,
                    currency_code: context.currency_code.clone(),
                },
            )
            .await
        {
            Ok(snapshot) => Some(rustok_pricing::ResolvedPrice::from(snapshot).into()),
            Err(error) if error.kind == rustok_api::PortErrorKind::NotFound => None,
            Err(error) => return Err(async_graphql::Error::new(error.message)),
        };
        variant.effective_price = effective_price;
    }

    Ok(())
}

fn pricing_query_port_context(
    tenant_id: Uuid,
    locale: &str,
    pricing_context: &rustok_pricing::PriceResolutionContext,
    product_id: Uuid,
    variant_id: Uuid,
) -> rustok_api::PortContext {
    let context = rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("rustok-commerce.pricing-query"),
        locale,
        format!("pricing-query:{product_id}:{variant_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    pricing_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

fn pricing_active_lists_port_context(
    tenant_id: Uuid,
    request_context: Option<&RequestContext>,
    default_locale: &str,
    channel_slug: Option<&str>,
) -> rustok_api::PortContext {
    let context = rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("rustok-commerce.pricing-query"),
        request_context
            .map(|context| context.locale.as_str())
            .unwrap_or(default_locale),
        "pricing-active-lists",
    )
    .with_deadline(std::time::Duration::from_secs(2));
    channel_slug
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

fn pricing_admin_product_port_context(
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
    locale: &str,
    pricing_context: Option<&rustok_pricing::PriceResolutionContext>,
    product_id: Uuid,
) -> rustok_api::PortContext {
    let actor = auth
        .map(|auth| rustok_api::PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| rustok_api::PortActor::service("rustok-commerce.pricing-query"));
    let context = rustok_api::PortContext::new(
        tenant_id.to_string(),
        actor,
        locale,
        format!("admin-pricing-product:{product_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    pricing_context
        .and_then(|context| context.channel_slug.as_deref())
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

fn pricing_storefront_product_port_context(
    tenant_id: Uuid,
    locale: &str,
    channel_slug: Option<&str>,
    handle: &str,
) -> rustok_api::PortContext {
    let context = rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("rustok-commerce.pricing-query"),
        locale,
        format!("storefront-pricing-product:{handle}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    channel_slug
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

async fn load_product_list_items(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    products: Vec<product::Model>,
    locale: &str,
    default_locale: &str,
    metric_path: &str,
) -> Result<Vec<GqlProductListItem>> {
    let product_ids = products
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let translations_started_at = std::time::Instant::now();
    let translations = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.is_in(product_ids))
            .all(db)
            .await?
    };
    metrics::record_read_path_query(
        "graphql",
        metric_path,
        "translations",
        translations_started_at.elapsed().as_secs_f64(),
        translations.len() as u64,
    );

    let mut translations_by_product: HashMap<Uuid, Vec<product_translation::Model>> =
        HashMap::new();
    for translation in translations {
        translations_by_product
            .entry(translation.product_id)
            .or_default()
            .push(translation);
    }
    let product_tags_started_at = std::time::Instant::now();
    let product_tags = CatalogService::new(db.clone(), event_bus.clone())
        .load_product_tag_map(tenant_id, &products, locale, Some(default_locale))
        .await
        .map_err(|error| map_product_service_error(error, "product_query"))?;
    metrics::record_read_path_query(
        "graphql",
        metric_path,
        "product_tags",
        product_tags_started_at.elapsed().as_secs_f64(),
        product_tags.len() as u64,
    );

    Ok(products
        .into_iter()
        .map(|product| {
            let translation = translations_by_product
                .get(&product.id)
                .and_then(|items| pick_translation(items, locale, default_locale));
            GqlProductListItem {
                id: product.id,
                status: product.status.into(),
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_else(|| "Untitled product".to_string()),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                seller_id: product.seller_id,
                vendor: product.vendor,
                product_type: product.product_type,
                shipping_profile_slug: Some(product_shipping_profile_slug(
                    product.shipping_profile_slug.as_deref(),
                    &product.metadata,
                )),
                tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
                created_at: product.created_at.to_rfc3339(),
                published_at: product.published_at.map(|value| value.to_rfc3339()),
            }
        })
        .collect())
}

fn localized_product_response(
    mut product: crate::dto::ProductResponse,
    locale: &str,
    default_locale: &str,
) -> crate::dto::ProductResponse {
    let selected_translation =
        pick_response_translation(&product.translations, locale, default_locale)
            .cloned()
            .into_iter()
            .collect::<Vec<_>>();
    if !selected_translation.is_empty() {
        product.translations = selected_translation;
    }
    product
}

fn pick_translation<'a>(
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

fn pick_response_translation<'a>(
    translations: &'a [crate::dto::ProductTranslationResponse],
    locale: &str,
    default_locale: &str,
) -> Option<&'a crate::dto::ProductTranslationResponse> {
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

async fn find_published_product_id_by_handle(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
) -> Result<Option<Uuid>> {
    if let Some(product_id) =
        find_published_product_id_for_locale(db, tenant_id, handle, locale, public_channel_slug)
            .await?
    {
        return Ok(Some(product_id));
    }

    if !locale_tags_match(default_locale, locale) {
        if let Some(product_id) = find_published_product_id_for_locale(
            db,
            tenant_id,
            handle,
            default_locale,
            public_channel_slug,
        )
        .await?
        {
            return Ok(Some(product_id));
        }
    }

    find_published_product_id_any_locale(db, tenant_id, handle, public_channel_slug).await
}

async fn find_published_product_id_for_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    public_channel_slug: Option<&str>,
) -> Result<Option<Uuid>> {
    let translations = product_translation::Entity::find()
        .filter(product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?
        .into_iter()
        .filter(|translation| locale_tags_match(&translation.locale, locale))
        .collect();

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

async fn find_published_product_id_any_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    public_channel_slug: Option<&str>,
) -> Result<Option<Uuid>> {
    let translations = product_translation::Entity::find()
        .filter(product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?;

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

async fn find_first_published_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    translations: Vec<product_translation::Model>,
    public_channel_slug: Option<&str>,
) -> Result<Option<Uuid>> {
    for translation in translations {
        let product = product::Entity::find_by_id(translation.product_id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .filter(
                product::Column::Status
                    .eq(rustok_product::entities::product::ProductStatus::Active),
            )
            .filter(product::Column::PublishedAt.is_not_null())
            .one(db)
            .await?;
        if product.as_ref().is_some_and(|product| {
            is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
        }) {
            return Ok(Some(translation.product_id));
        }
    }

    Ok(None)
}

fn product_list_path(path: &'static str) -> &'static str {
    path
}

fn effective_attribute_source_name(
    source: rustok_product::services::EffectiveAttributeSource,
) -> &'static str {
    match source {
        rustok_product::services::EffectiveAttributeSource::Schema => "schema",
        rustok_product::services::EffectiveAttributeSource::Inherited => "inherited",
        rustok_product::services::EffectiveAttributeSource::CloneSnapshot => "clone_snapshot",
        rustok_product::services::EffectiveAttributeSource::CategoryLocal => "category_local",
    }
}

fn request_public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(public_channel_slug_from_request)
}

fn storefront_public_channel_slug_for_cart(
    cart: &crate::dto::CartResponse,
    ctx: &Context<'_>,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| request_public_channel_slug(ctx))
}

async fn resolve_optional_storefront_customer_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            graphql_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(async_graphql::Error::new(error.message)),
    }
}

fn graphql_customer_port_context(tenant_id: Uuid, user_id: Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::user(user_id.to_string()),
        "en",
        format!("storefront-customer:{user_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn graphql_cart_port_context(tenant_id: Uuid, cart_id: Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("rustok-commerce.graphql"),
        "en",
        format!("storefront-cart:{cart_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
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

async fn resolve_storefront_context(
    db: &DatabaseConnection,
    ctx: &Context<'_>,
    tenant_id: Uuid,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<crate::dto::StoreContextResponse> {
    let tenant = ctx.data::<TenantContext>()?;
    let locale = Some(resolve_commerce_graphql_locale(
        ctx,
        locale.as_deref(),
        tenant.default_locale.as_str(),
    ));
    StoreContextService::new(
        db.clone(),
        std::sync::Arc::new(RegionService::new(db.clone())),
    )
    .resolve_context(
        tenant_id,
        crate::dto::ResolveStoreContextInput {
            region_id,
            country_code,
            locale,
            currency_code,
        },
    )
    .await
    .map_err(|err| async_graphql::Error::new(err.to_string()))
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

use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError},
    AuthContext, RequestContext, TenantContext,
};
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    entities::{product, product_translation},
    search::product_translation_title_search_condition,
    storefront_channel::{
        apply_public_channel_inventory_to_product, is_metadata_visible_for_public_channel,
        normalize_public_channel_slug, public_channel_slug_from_request,
    },
    storefront_shipping::{
        is_shipping_option_compatible_with_profiles, load_cart_shipping_profile_slugs,
        shipping_profile_slug_from_product_metadata,
    },
    CatalogService, CommerceError, CustomerService, FulfillmentService, OrderService,
    PaymentService, RegionService, ShippingProfileService, StoreContextService,
};

use super::{require_commerce_permission, types::*, MODULE_SLUG};

#[derive(Default)]
pub struct CommerceQuery;

#[Object]
impl CommerceQuery {
    async fn storefront_regions(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<GqlRegion>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let regions = RegionService::new(db.clone())
            .list_regions(tenant_id)
            .await?;

        Ok(regions.into_iter().map(Into::into).collect())
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
                let cart = crate::CartService::new(db.clone())
                    .get_cart(tenant_id, cart_id)
                    .await?;
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
            .list_shipping_options(tenant_id)
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
        let customer = CustomerService::new(db.clone())
            .get_customer_by_user(tenant_id, auth.user_id)
            .await
            .map_err(|err| match err {
                rustok_customer::error::CustomerError::CustomerByUserNotFound(_) => {
                    <FieldError as GraphQLError>::unauthenticated()
                }
                other => async_graphql::Error::new(other.to_string()),
            })?;

        Ok(customer.into())
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
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let customer = CustomerService::new(db.clone())
            .get_customer_by_user(tenant_id, auth.user_id)
            .await
            .map_err(|err| match err {
                rustok_customer::error::CustomerError::CustomerByUserNotFound(_) => {
                    <FieldError as GraphQLError>::unauthenticated()
                }
                other => async_graphql::Error::new(other.to_string()),
            })?;
        let order = match OrderService::new(db.clone(), event_bus.clone())
            .get_order(tenant_id, id)
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

        Ok(Some(order.into()))
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
        let cart = match crate::CartService::new(db.clone())
            .get_cart(tenant_id, id)
            .await
        {
            Ok(cart) => cart,
            Err(rustok_cart::error::CartError::CartNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        ensure_storefront_cart_access(&cart, customer_id)?;
        Ok(Some(cart.into()))
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

        let order = match OrderService::new(db.clone(), event_bus.clone())
            .get_order(tenant_id, id)
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
        let filter = filter.unwrap_or(OrdersFilter {
            status: None,
            customer_id: None,
            page: Some(1),
            per_page: Some(20),
        });
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let (orders, total) = OrderService::new(db.clone(), event_bus.clone())
            .list_orders(
                tenant_id,
                crate::dto::ListOrdersInput {
                    page,
                    per_page,
                    status: filter.status,
                    customer_id: filter.customer_id,
                },
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
                return Ok(None)
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
        let option = match FulfillmentService::new(db.clone())
            .get_shipping_option(tenant_id, id)
            .await
        {
            Ok(option) => option,
            Err(rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_)) => {
                return Ok(None)
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
        let profile = match ShippingProfileService::new(db.clone())
            .get_shipping_profile(tenant_id, id)
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
            .list_all_shipping_options(tenant_id)
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
                },
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
                return Ok(None)
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

    async fn product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlProduct>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_READ],
            "Permission denied: products:read required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = CatalogService::new(db.clone(), event_bus.clone());
        let product = match service.get_product(tenant_id, id).await {
            Ok(product) => product,
            Err(CommerceError::ProductNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
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
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_LIST],
            "Permission denied: products:list required",
        )?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = filter.unwrap_or(ProductsFilter {
            status: None,
            vendor: None,
            search: None,
            page: Some(1),
            per_page: Some(20),
        });
        let requested_limit = filter.per_page.map(|value| value.max(0) as u64);
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(20).clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = product::Entity::find().filter(product::Column::TenantId.eq(tenant_id));

        if let Some(status) = &filter.status {
            let status: crate::entities::product::ProductStatus = (*status).into();
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

    async fn storefront_product(
        &self,
        ctx: &Context<'_>,
        id: Option<Uuid>,
        handle: Option<String>,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlProduct>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

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
                ))
            }
        };

        let Some(product_id) = product_id else {
            return Ok(None);
        };

        let service = CatalogService::new(db.clone(), event_bus.clone());
        let mut product = match service.get_product(tenant_id, product_id).await {
            Ok(product) => product,
            Err(CommerceError::ProductNotFound(_)) => return Ok(None),
            Err(err) => return Err(err.to_string().into()),
        };

        if product.status != crate::entities::product::ProductStatus::Active
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
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = filter.unwrap_or(StorefrontProductsFilter {
            vendor: None,
            product_type: None,
            search: None,
            page: Some(1),
            per_page: Some(12),
        });
        let requested_limit = filter.per_page.map(|value| value.max(0) as u64);
        let page = filter.page.unwrap_or(1).max(1);
        let per_page = filter.per_page.unwrap_or(12).clamp(1, 48);
        let offset = (page.saturating_sub(1)) * per_page;
        let public_channel_slug = request_public_channel_slug(ctx);

        let mut query = product::Entity::find()
            .filter(product::Column::TenantId.eq(tenant_id))
            .filter(product::Column::Status.eq(crate::entities::product::ProductStatus::Active))
            .filter(product::Column::PublishedAt.is_not_null());

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

        let visible_products = query
            .order_by_desc(product::Column::PublishedAt)
            .order_by_desc(product::Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .filter(|product| {
                is_metadata_visible_for_public_channel(
                    &product.metadata,
                    public_channel_slug.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        let total = visible_products.len() as u64;
        let products = visible_products
            .into_iter()
            .skip(offset as usize)
            .take(per_page as usize)
            .collect::<Vec<_>>();

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
        .await?;
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
                vendor: product.vendor,
                product_type: product.product_type,
                shipping_profile_slug: Some(shipping_profile_slug_from_product_metadata(
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
        .find(|translation| translation.locale == locale)
        .or_else(|| {
            (default_locale != locale).then(|| {
                translations
                    .iter()
                    .find(|translation| translation.locale == default_locale)
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
        .find(|translation| translation.locale == locale)
        .or_else(|| {
            (default_locale != locale).then(|| {
                translations
                    .iter()
                    .find(|translation| translation.locale == default_locale)
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

    if default_locale != locale {
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
        .filter(product_translation::Column::Locale.eq(locale))
        .all(db)
        .await?;

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
            .filter(product::Column::Status.eq(crate::entities::product::ProductStatus::Active))
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

fn request_public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| public_channel_slug_from_request(request_context))
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

async fn resolve_storefront_context(
    db: &DatabaseConnection,
    ctx: &Context<'_>,
    tenant_id: Uuid,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<crate::dto::StoreContextResponse> {
    let locale = locale.or_else(|| Some(resolve_graphql_locale(ctx, None)));
    StoreContextService::new(db.clone())
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

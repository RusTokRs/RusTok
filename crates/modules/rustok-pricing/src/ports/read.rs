use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::dto::pricing::*;
use crate::ports::diagnostics::{
    log_pricing_port_entry, log_pricing_port_failure, parse_port_tenant_id,
    pricing_error_to_port_error, read_with_deadline, resolution_error_to_port_error,
    resolution_scope_error, validate_non_nil_uuid, PricingOwnerErrorFacts,
};

/// Transport-neutral owner boundary for pricing read projections.
#[async_trait]
pub trait PricingReadPort: Send + Sync {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError>;

    /// Quote a bounded SKU set in one shared context/snapshot. Missing prices are explicit
    /// items; a missing identity or unsafe resolution rejects the batch, never substitutes.
    async fn resolve_product_prices(
        &self,
        context: PortContext,
        request: ResolveProductPricesRequest,
    ) -> Result<ResolvedProductPricesSnapshot, PortError>;

    async fn list_storefront_product_pricing(
        &self,
        context: PortContext,
        request: StorefrontProductPricingListRequest,
    ) -> Result<crate::StorefrontPricingProductList, PortError>;

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError>;

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<ActivePriceListProjectionSnapshot>, PortError>;

    async fn read_admin_product_pricing_projection(
        &self,
        context: PortContext,
        request: AdminProductPricingProjectionRequest,
    ) -> Result<crate::AdminPricingProductDetail, PortError>;

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<crate::StorefrontPricingProductDetail>, PortError>;

    async fn preview_variant_discount(
        &self,
        context: PortContext,
        request: PreviewVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError>;
}

#[async_trait]
impl PricingReadPort for crate::PricingService {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        log_pricing_port_entry(&context, "resolve_product_price");
        let variant_id = request.variant_id;
        let mut batch = self
            .resolve_product_prices(
                context.clone(),
                ResolveProductPricesRequest {
                    product_id: request.product_id,
                    variant_ids: vec![variant_id],
                    pricing: crate::PriceResolutionContext {
                        currency_code: request.currency_code,
                        region_id: request.region_id,
                        channel_id: request.channel_id,
                        channel_slug: request.channel_slug,
                        price_list_id: request.price_list_id,
                        quantity: request.quantity,
                    },
                },
            )
            .await?;
        let line = batch.items.pop().ok_or_else(|| {
            PortError::invariant_violation(
                "pricing.resolution_incomplete",
                "SKU quote could not be completed safely",
            )
        })?;
        let resolved = line.price.ok_or_else(|| {
            PortError::new(
                PortErrorKind::NotFound,
                "pricing.price_not_found",
                "price was not found",
                false,
            )
        })?;
        Ok(ResolvedProductPriceSnapshot {
            source: resolved.source,
            context: batch.context,
            resolved_at: batch.resolved_at,
            // Always return the Product-owned association, even for variant-first input.
            product_id: line.product_id,
            variant_id: line.variant_id,
            currency_code: resolved.currency_code,
            amount: resolved.amount,
            compare_at_amount: resolved.compare_at_amount,
            discount_percent: resolved.discount_percent,
            on_sale: resolved.on_sale,
            region_id: resolved.region_id,
            min_quantity: resolved.min_quantity,
            max_quantity: resolved.max_quantity,
            price_list_id: resolved.price_list_id,
            channel_id: resolved.channel_id,
            channel_slug: resolved.channel_slug,
        })
    }

    async fn resolve_product_prices(
        &self,
        context: PortContext,
        request: ResolveProductPricesRequest,
    ) -> Result<ResolvedProductPricesSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        log_pricing_port_entry(&context, "resolve_product_prices");
        let result = read_with_deadline(
            &context,
            resolve_product_prices_in_context(self, &context, request),
        )
        .await;
        if let Err(error) = &result {
            let facts = PricingOwnerErrorFacts::empty("batch_resolution_refused");
            log_pricing_port_failure(
                &context,
                "resolve_product_prices",
                "pricing.batch_resolution_refused",
                &facts,
                matches!(
                    error.kind,
                    PortErrorKind::Unavailable
                        | PortErrorKind::Timeout
                        | PortErrorKind::InvariantViolation
                ),
            );
        }
        result
    }

    async fn list_storefront_product_pricing(
        &self,
        mut context: PortContext,
        mut request: StorefrontProductPricingListRequest,
    ) -> Result<crate::StorefrontPricingProductList, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        parse_port_tenant_id(&context, "list_storefront_product_pricing")?;
        if request.page == 0
            || request.per_page == 0
            || request.per_page > rustok_product::MAX_PUBLISHED_PRODUCTS_PER_PAGE
        {
            return Err(PortError::validation(
                "pricing.catalog_page_invalid",
                "pricing catalog page must contain between 1 and 48 Products",
            ));
        }
        log_pricing_port_entry(&context, "list_storefront_product_pricing");
        let budget_context = context.clone();
        read_with_deadline(&budget_context, async {
            let (id, slug) =
                canonical_channel_scope(self, &context, request.channel_id, request.channel_slug)
                    .await?;
            request.channel_id = id;
            request.channel_slug = slug.clone();
            context.channel = slug;
            crate::PricingService::list_storefront_product_pricing(self, context, request).await
        })
        .await
    }

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError> {
        let owner_operation = "read_price_list_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.price_list_id, "price_list_id")?;
        log_pricing_port_entry(&context, owner_operation);
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        let lists = self
            .list_active_price_lists(tenant_id, Some(locale), Some(locale))
            .await
            .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))?;
        let list = lists
            .into_iter()
            .find(|list| list.id == request.price_list_id)
            .ok_or_else(|| {
                let facts =
                    PricingOwnerErrorFacts::uuids("price_list_not_found", &[request.price_list_id]);
                log_pricing_port_failure(
                    &context,
                    owner_operation,
                    "pricing.price_list_not_found",
                    &facts,
                    false,
                );
                PortError::new(
                    PortErrorKind::NotFound,
                    "pricing.price_list_not_found",
                    "price list was not found",
                    false,
                )
            })?;

        Ok(PriceListProjectionSnapshot {
            price_list_id: list.id,
            title: list.name,
            currency_code: None,
            starts_at: None,
            ends_at: None,
        })
    }

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<ActivePriceListProjectionSnapshot>, PortError> {
        let owner_operation = "list_active_price_list_projections";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        if let Some(channel_id) = request.channel_id {
            validate_non_nil_uuid(channel_id, "channel_id")?;
        }
        log_pricing_port_entry(&context, owner_operation);
        let (channel_id, channel_slug) =
            canonical_channel_scope(self, &context, request.channel_id, request.channel_slug)
                .await?;
        let lists = self
            .list_active_price_lists_for_channel(
                tenant_id,
                channel_id,
                channel_slug.as_deref(),
                Some(context.locale.as_str()),
                request.fallback_locale.as_deref(),
            )
            .await
            .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))?;

        Ok(lists
            .into_iter()
            .map(|list| ActivePriceListProjectionSnapshot {
                price_list_id: list.id,
                title: list.name,
                list_type: list.list_type,
                channel_id: list.channel_id,
                channel_slug: list.channel_slug,
                rule_kind: list.rule_kind,
                adjustment_percent: list.adjustment_percent,
            })
            .collect())
    }

    async fn read_admin_product_pricing_projection(
        &self,
        context: PortContext,
        request: AdminProductPricingProjectionRequest,
    ) -> Result<crate::AdminPricingProductDetail, PortError> {
        let owner_operation = "read_admin_product_pricing_projection";
        context.require_policy(PortCallPolicy::read())?;
        parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.product_id, "product_id")?;
        if let Some(price_list_id) = request.selected_price_list_id {
            validate_non_nil_uuid(price_list_id, "selected_price_list_id")?;
        }
        log_pricing_port_entry(&context, owner_operation);
        self.get_admin_product_pricing_with_locale_fallback(
            context.clone(),
            request.product_id,
            request.fallback_locale.as_deref(),
            request.selected_price_list_id,
        )
        .await
    }

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<crate::StorefrontPricingProductDetail>, PortError> {
        let owner_operation = "read_storefront_product_pricing_projection";
        context.require_policy(PortCallPolicy::read())?;
        parse_port_tenant_id(&context, owner_operation)?;
        log_pricing_port_entry(&context, owner_operation);
        let (_, public_channel_slug) = canonical_channel_scope(
            self,
            &context,
            request.public_channel_id,
            request.public_channel_slug,
        )
        .await?;
        self.get_published_product_pricing_by_handle_with_locale_fallback(
            context.clone(),
            request.handle.trim(),
            request.fallback_locale.as_deref(),
            public_channel_slug.as_deref(),
        )
        .await
    }

    async fn preview_variant_discount(
        &self,
        context: PortContext,
        request: PreviewVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError> {
        let owner_operation = "preview_variant_discount";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(price_list_id) = request.price_list_id {
            validate_non_nil_uuid(price_list_id, "price_list_id")?;
        }
        if let Some(channel_id) = request.channel_id {
            validate_non_nil_uuid(channel_id, "channel_id")?;
        }
        self.require_embedded_variant_state()?;
        self.read_product_variant_identities(
            &context,
            &rustok_product::ProductVariantIdentitiesRequest {
                product_id: None,
                variant_ids: vec![request.variant_id],
            },
        )
        .await?;

        log_pricing_port_entry(&context, owner_operation);
        let preview = if let Some(price_list_id) = request.price_list_id {
            self.preview_price_list_percentage_discount_with_channel(
                tenant_id,
                request.variant_id,
                price_list_id,
                request.currency_code.as_str(),
                request.discount_percent,
                request.channel_id,
                request.channel_slug,
            )
            .await
        } else {
            self.preview_percentage_discount_with_channel(
                request.variant_id,
                request.currency_code.as_str(),
                request.discount_percent,
                request.channel_id,
                request.channel_slug,
            )
            .await
        };
        preview.map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }
}

pub(crate) async fn resolve_product_prices_in_context(
    service: &crate::PricingService,
    context: &PortContext,
    request: ResolveProductPricesRequest,
) -> Result<ResolvedProductPricesSnapshot, PortError> {
    let operation = "resolve_product_prices";
    let tenant_id = parse_port_tenant_id(context, operation)?;
    if request.variant_ids.is_empty() || request.variant_ids.len() > MAX_PRICE_RESOLUTION_VARIANTS {
        return Err(PortError::validation(
            "pricing.batch_size_invalid",
            "quote between 1 and 256 SKUs",
        ));
    }
    if request.variant_ids.iter().any(Uuid::is_nil)
        || request.product_id.is_some_and(|id| id.is_nil())
        || request
            .variant_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != request.variant_ids.len()
    {
        return Err(PortError::validation(
            "pricing.batch_identity_invalid",
            "request unique, non-nil Product/SKU identities",
        ));
    }
    let mut pricing = request
        .pricing
        .normalized()
        .map_err(|error| pricing_error_to_port_error(context, operation, error))?;
    let identity_request = rustok_product::ProductVariantIdentitiesRequest {
        product_id: request.product_id,
        variant_ids: request.variant_ids.clone(),
    };
    let identities = service
        .read_product_variant_identities(context, &identity_request)
        .await?;

    let (channel_id, channel_slug) =
        canonical_channel_scope(service, context, pricing.channel_id, pricing.channel_slug).await?;
    pricing.channel_id = channel_id;
    pricing.channel_slug = channel_slug;
    if let Some(region_id) = pricing.region_id {
        let projection = service
            .region_read_port()
            .read_region(
                context.clone(),
                rustok_region::RegionReadRequest {
                    selector: rustok_region::RegionReadSelector::Id(region_id),
                    requested_locale: Some(context.locale.clone()),
                    tenant_default_locale: None,
                },
            )
            .await
            .map_err(|error| {
                resolution_scope_error(
                    error,
                    "pricing.region_invalid",
                    "requested region is not available",
                )
            })?
            .ok_or_else(|| {
                PortError::validation(
                    "pricing.region_invalid",
                    "requested region is not available",
                )
            })?;
        if projection.region.currency_code != pricing.currency_code {
            return Err(PortError::validation(
                "pricing.region_currency_mismatch",
                "quote currency must match the requested region currency",
            ));
        }
    }
    let batch = service
        .resolve_price_snapshot(tenant_id, request.variant_ids.clone(), pricing)
        .await
        .map_err(|error| resolution_error_to_port_error(context, operation, error))?;
    let mut prices = batch.prices.into_iter().collect::<BTreeMap<_, _>>();
    let mut items = Vec::with_capacity(identities.len());
    for identity in identities {
        let price = prices.remove(&identity.variant_id).ok_or_else(|| {
            PortError::invariant_violation(
                "pricing.resolution_incomplete",
                "SKU quote could not be completed safely",
            )
        })?;
        items.push(ProductPriceResolutionSnapshot {
            product_id: identity.product_id,
            variant_id: identity.variant_id,
            price,
        });
    }
    Ok(ResolvedProductPricesSnapshot {
        context: batch.context,
        resolved_at: batch.resolved_at,
        items,
    })
}

pub(crate) async fn canonical_channel_scope(
    service: &crate::PricingService,
    context: &PortContext,
    mut channel_id: Option<Uuid>,
    mut channel_slug: Option<String>,
) -> Result<(Option<Uuid>, Option<String>), PortError> {
    if channel_id.is_some_and(|id| id.is_nil())
        || channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
    {
        return Err(PortError::validation(
            "pricing.channel_invalid",
            "channel selector is invalid",
        ));
    }
    channel_slug = channel_slug.map(|slug| slug.trim().to_ascii_lowercase());
    // Use a trusted context channel only when no explicit channel selector was supplied.
    // Never mix an explicit ID for one channel with a slug borrowed from another.
    let context_channel = context
        .channel
        .as_deref()
        .map(|slug| slug.trim().to_ascii_lowercase());
    if context_channel.as_deref().is_some_and(str::is_empty) {
        return Err(PortError::validation(
            "pricing.channel_context_invalid",
            "pricing context channel is empty",
        ));
    }
    if channel_id.is_none() && channel_slug.is_none() {
        channel_slug = context_channel.clone();
    }
    if channel_id.is_some() || channel_slug.is_some() {
        let projection = service
            .channel_read_port()
            .read_selected_channel(context.clone(), channel_id, channel_slug.clone())
            .await
            .map_err(|error| {
                let mismatch = error.code == "channel.selector_mismatch";
                resolution_scope_error(
                    error,
                    if mismatch {
                        "pricing.channel_mismatch"
                    } else {
                        "pricing.channel_invalid"
                    },
                    "channel ID, slug and pricing context must identify an available channel",
                )
            })?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "pricing.channel_projection_missing",
                    "the Channel owner omitted a selected channel",
                )
            })?;
        let channel = projection.detail.channel;
        let slug = channel.slug;
        if context_channel
            .as_ref()
            .is_some_and(|trusted| trusted != &slug)
        {
            return Err(PortError::validation(
                "pricing.channel_mismatch",
                "channel ID, slug and pricing context must identify the same channel",
            ));
        }
        channel_id = Some(channel.id);
        channel_slug = Some(slug);
    }
    Ok((channel_id, channel_slug))
}

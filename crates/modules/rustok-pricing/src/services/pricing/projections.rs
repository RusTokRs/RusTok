use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use std::collections::{BTreeSet, HashMap};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{PortContext, PortError, PortErrorKind, locale_tags_match};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::error::CommerceError;
use rustok_product::dto::{ProductResponse, ProductTranslationResponse};

use super::PricingService;
use super::resolution;
use super::types::*;
use crate::entities::price;

impl PricingService {
    #[instrument(skip(self, context))]
    #[allow(clippy::too_many_arguments)]
    pub async fn list_admin_product_pricing_with_locale_fallback(
        &self,
        context: PortContext,
        fallback_locale: Option<&str>,
        search: Option<&str>,
        status: Option<ProductStatus>,
        page: u64,
        per_page: u64,
    ) -> Result<AdminPricingProductList, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            crate::ports::parse_port_tenant_id(&context, "list_admin_product_pricing")?;
            let page = Ord::max(page, 1);
            let per_page = per_page.clamp(1, 100);
            let catalog = self.product_catalog_read_runtime.read_port();
            let products = catalog
                .list_legacy_admin_products(
                    context.clone(),
                    rustok_product::LegacyAdminProductsRequest {
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                        search: search.map(str::to_owned),
                        status: status.map(map_foundation_product_status),
                        vendor: None,
                        page,
                        per_page,
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            let ids = products
                .items
                .iter()
                .map(|item| item.id)
                .collect::<BTreeSet<_>>();
            if products.page != page
                || products.per_page != per_page
                || products.items.len() > per_page as usize
                || ids.len() != products.items.len()
                || ids.iter().any(Uuid::is_nil)
            {
                return Err(PortError::invariant_violation(
                    "pricing.catalog_page_invalid",
                    "The selected Product owner returned an inconsistent pricing inspection page",
                ));
            }
            let mut items = Vec::with_capacity(products.items.len());
            for item in products.items {
                let product = catalog
                    .read_product_projection(
                        context.clone(),
                        rustok_product::ProductProjectionRequest {
                            product_id: item.id,
                            locale: Some(context.locale.clone()),
                            fallback_locale: fallback_locale.map(str::to_owned),
                        },
                    )
                    .await
                    .map_err(pricing_catalog_port_error)?;
                validate_pricing_product_projection(&context, Some(item.id), &product)?;
                items.push(map_admin_list_item(
                    product,
                    &context.locale,
                    fallback_locale.unwrap_or(&context.locale),
                ));
            }
            Ok(AdminPricingProductList {
                items,
                total: products.total,
                page: products.page,
                per_page: products.per_page,
                has_next: products.has_next,
            })
        })
        .await
    }

    #[instrument(skip(self, context), fields(product_id = %product_id))]
    pub async fn get_admin_product_pricing_with_locale_fallback(
        &self,
        context: PortContext,
        product_id: Uuid,
        fallback_locale: Option<&str>,
        selected_price_list_id: Option<Uuid>,
    ) -> Result<AdminPricingProductDetail, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            let operation = "read_admin_product_pricing_projection";
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            let tenant_id = crate::ports::parse_port_tenant_id(&context, operation)?;
            if product_id.is_nil() || selected_price_list_id.is_some_and(|id| id.is_nil()) {
                return Err(PortError::validation(
                    "pricing.catalog_identity_invalid",
                    "Use non-nil Product and Price List identities",
                ));
            }
            let product = self
                .product_catalog_read_runtime
                .read_port()
                .read_product_projection(
                    context.clone(),
                    rustok_product::ProductProjectionRequest {
                        product_id,
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            validate_pricing_product_projection(&context, Some(product_id), &product)?;
            if let Some(price_list_id) = selected_price_list_id {
                let list = crate::entities::price_list::Entity::find_by_id(price_list_id)
                    .filter(crate::entities::price_list::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await
                    .map_err(|error| {
                        crate::ports::pricing_error_to_port_error(&context, operation, error.into())
                    })?;
                if list.is_none() {
                    return Err(PortError::not_found(
                        "pricing.price_list_not_found",
                        "The selected Price List is not available to this tenant",
                    ));
                }
            }
            let variant_ids = product
                .variants
                .iter()
                .map(|variant| variant.id)
                .collect::<Vec<_>>();
            let mut prices_by_variant = HashMap::<Uuid, Vec<price::Model>>::new();
            if !variant_ids.is_empty() {
                let mut query =
                    price::Entity::find().filter(price::Column::VariantId.is_in(variant_ids));
                if let Some(selected_price_list_id) = selected_price_list_id {
                    query = query.filter(
                        sea_orm::Condition::any()
                            .add(price::Column::PriceListId.is_null())
                            .add(price::Column::PriceListId.eq(selected_price_list_id)),
                    );
                } else {
                    query = query.filter(price::Column::PriceListId.is_null());
                }

                let prices = query
                    .limit((MAX_PUBLIC_PRICE_CATALOG_ROWS + 1) as u64)
                    .all(&self.db)
                    .await
                    .map_err(|error| {
                        crate::ports::pricing_error_to_port_error(&context, operation, error.into())
                    })?;
                if prices.len() > MAX_PUBLIC_PRICE_CATALOG_ROWS {
                    return Err(PortError::new(
                        rustok_api::PortErrorKind::Conflict,
                        "pricing.catalog_price_row_limit",
                        "Pricing inspection exceeds its row budget; narrow the selected Price List",
                        false,
                    ));
                }
                for price in prices {
                    prices_by_variant
                        .entry(price.variant_id)
                        .or_default()
                        .push(price);
                }
            }
            for prices in prices_by_variant.values_mut() {
                prices.sort_by(super::queries::price_specificity_cmp);
            }

            Ok(map_admin_detail(product, prices_by_variant))
        })
        .await
    }

    pub(crate) async fn list_storefront_product_pricing(
        &self,
        context: PortContext,
        request: crate::StorefrontProductPricingListRequest,
    ) -> Result<StorefrontPricingProductList, PortError> {
        let identity_port = self.variant_identity_read_port()?;
        let products = self
            .product_catalog_read_runtime
            .read_port()
            .list_legacy_storefront_products(
                context.clone(),
                rustok_product::LegacyStorefrontProductsRequest {
                    vendor: None,
                    product_type: None,
                    search: None,
                    locale: Some(context.locale.clone()),
                    fallback_locale: request.fallback_locale,
                    public_channel_slug: request.channel_slug,
                    page: request.page,
                    per_page: request.per_page,
                },
            )
            .await
            .map_err(pricing_catalog_port_error)?;
        let selected_product_ids = products
            .items
            .iter()
            .map(|product| product.id)
            .collect::<BTreeSet<_>>();
        if products.items.len() > request.per_page as usize
            || selected_product_ids.len() != products.items.len()
            || selected_product_ids.iter().any(Uuid::is_nil)
            || products.page != request.page
            || products.per_page != request.per_page
        {
            return Err(PortError::invariant_violation(
                "pricing.catalog_page_invalid",
                "The selected Product owner returned an inconsistent pricing inspection page",
            ));
        }
        let identity_request = rustok_product::ProductVariantIdentityListRequest {
            product_ids: products.items.iter().map(|product| product.id).collect(),
        };
        let variants = identity_port
            .list_product_variant_identities(context.clone(), identity_request.clone())
            .await
            .map_err(pricing_identity_port_error)?;
        rustok_product::validate_product_variant_identity_list_response(
            &context,
            &identity_request,
            &variants,
        )
        .map_err(|error| {
            PortError::new(
                error.kind,
                "pricing.variant_identity_invalid",
                "The selected Product owner returned inconsistent page SKU ownership",
                error.retryable,
            )
        })?;
        let mut variant_counts_by_product = HashMap::<Uuid, u64>::new();
        let mut variant_to_product = HashMap::<Uuid, Uuid>::new();
        for variant in variants {
            variant_to_product.insert(variant.variant_id, variant.product_id);
            *variant_counts_by_product
                .entry(variant.product_id)
                .or_insert(0) += 1;
        }
        let variant_ids = variant_to_product.keys().copied().collect::<Vec<_>>();
        let prices = if variant_ids.is_empty() {
            Vec::new()
        } else {
            price::Entity::find()
                .filter(price::Column::VariantId.is_in(variant_ids))
                .filter(crate::services::bootstrap::catalog_base_scope())
                .limit((MAX_PUBLIC_PRICE_CATALOG_ROWS + 1) as u64)
                .all(&self.db)
                .await
                .map_err(|error| {
                    crate::ports::pricing_error_to_port_error(
                        &context,
                        "list_storefront_product_pricing",
                        CommerceError::Database(error),
                    )
                })?
        };
        if prices.len() > MAX_PUBLIC_PRICE_CATALOG_ROWS {
            return Err(PortError::new(
                rustok_api::PortErrorKind::Conflict,
                "pricing.catalog_price_row_limit",
                "Pricing inspection page exceeds its row budget; reduce page size",
                false,
            ));
        }
        let mut currencies_by_product = HashMap::<Uuid, BTreeSet<String>>::new();
        let mut sale_variants_by_product = HashMap::<Uuid, BTreeSet<Uuid>>::new();
        for price in prices {
            resolution::validate_row(&price, &price.currency_code, None).map_err(|error| {
                crate::ports::resolution_error_to_port_error(
                    &context,
                    "list_storefront_product_pricing",
                    error,
                )
            })?;
            let product_id = variant_to_product
                .get(&price.variant_id)
                .copied()
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "pricing.catalog_identity_incomplete",
                        "Pricing catalog identity projection is incomplete",
                    )
                })?;
            currencies_by_product
                .entry(product_id)
                .or_default()
                .insert(price.currency_code);
            if price
                .compare_at_amount
                .map(|compare| compare > price.amount)
                .unwrap_or(false)
            {
                sale_variants_by_product
                    .entry(product_id)
                    .or_default()
                    .insert(price.variant_id);
            }
        }

        Ok(StorefrontPricingProductList {
            items: products
                .items
                .into_iter()
                .map(|product| StorefrontPricingProductListItem {
                    id: product.id,
                    title: product.title,
                    handle: product.handle,
                    seller_id: product.seller_id,
                    vendor: product.vendor,
                    product_type: product.product_type,
                    created_at: product.created_at,
                    published_at: product.published_at,
                    variant_count: variant_counts_by_product.remove(&product.id).unwrap_or(0),
                    sale_variant_count: sale_variants_by_product
                        .remove(&product.id)
                        .map(|variants| variants.len() as u64)
                        .unwrap_or(0),
                    currencies: currencies_by_product
                        .remove(&product.id)
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                })
                .collect(),
            total: products.total,
            page: products.page,
            per_page: products.per_page,
            has_next: products.has_next,
        })
    }

    #[instrument(skip(self, context))]
    pub async fn get_published_product_pricing_by_handle_with_locale_fallback(
        &self,
        context: PortContext,
        handle: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> Result<Option<StorefrontPricingProductDetail>, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            crate::ports::parse_port_tenant_id(
                &context,
                "read_storefront_product_pricing_projection",
            )?;
            let product = self
                .product_catalog_read_runtime
                .read_port()
                .read_storefront_product_projection(
                    context.clone(),
                    rustok_product::StorefrontProductProjectionRequest {
                        subject: rustok_product::StorefrontProductProjectionSubject::Handle {
                            handle: handle.to_owned(),
                        },
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                        public_channel_slug: public_channel_slug.map(str::to_owned),
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            if let Some(product) = product.as_ref() {
                validate_pricing_product_projection(&context, None, product)?;
                if !product
                    .translations
                    .iter()
                    .any(|translation| translation.handle == handle)
                {
                    return Err(PortError::invariant_violation(
                        "pricing.catalog_identity_invalid",
                        "The selected Product owner returned a different storefront link",
                    ));
                }
            }
            Ok(product.map(map_product_detail))
        })
        .await
    }
}

pub(crate) fn pricing_identity_port_error(error: PortError) -> PortError {
    let code = if error.code == "product.variant_product_mismatch" {
        "pricing.variant_product_mismatch"
    } else if error.kind == PortErrorKind::NotFound {
        "pricing.variant_not_found"
    } else {
        "pricing.variant_identity_unavailable"
    };
    PortError::new(
        error.kind,
        code,
        "SKU ownership could not be verified for this pricing request",
        error.retryable,
    )
}

pub(crate) fn pricing_catalog_port_error(error: PortError) -> PortError {
    let code = match error.kind {
        rustok_api::PortErrorKind::NotFound => "pricing.product_not_found",
        rustok_api::PortErrorKind::Validation => "pricing.catalog_request_invalid",
        _ => "pricing.catalog_unavailable",
    };
    PortError::new(
        error.kind,
        code,
        "The selected Product projection could not be established",
        error.retryable,
    )
}

pub(crate) fn validate_pricing_product_projection(
    context: &PortContext,
    expected_product_id: Option<Uuid>,
    product: &ProductResponse,
) -> Result<(), PortError> {
    let tenant_id = crate::ports::parse_port_tenant_id(context, "pricing_product_projection")?;
    let variants = product
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<BTreeSet<_>>();
    if product.tenant_id != tenant_id
        || product.id.is_nil()
        || expected_product_id.is_some_and(|id| id != product.id)
        || product.variants.len() > rustok_product::MAX_VARIANT_IDENTITY_BATCH
        || variants.len() != product.variants.len()
        || variants.iter().any(Uuid::is_nil)
        || product
            .variants
            .iter()
            .any(|variant| variant.product_id != product.id)
    {
        return Err(PortError::invariant_violation(
            "pricing.catalog_identity_invalid",
            "The selected Product owner returned an inconsistent Product/SKU projection",
        ));
    }
    Ok(())
}

pub(crate) fn map_product_detail(product: ProductResponse) -> StorefrontPricingProductDetail {
    StorefrontPricingProductDetail {
        id: product.id,
        status: map_product_status(product.status),
        seller_id: product.seller_id,
        vendor: product.vendor,
        product_type: product.product_type,
        published_at: product.published_at,
        translations: product
            .translations
            .into_iter()
            .map(|translation| StorefrontPricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: product
            .variants
            .into_iter()
            .map(|variant| StorefrontPricingVariant {
                id: variant.id,
                title: variant.title,
                sku: variant.sku,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| StorefrontPricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount,
                        compare_at_amount: price.compare_at_amount,
                        discount_percent: calculate_discount_percent(
                            price.amount,
                            price.compare_at_amount,
                        ),
                        on_sale: price.on_sale,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(crate) fn map_admin_list_item(
    product: ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> AdminPricingProductListItem {
    let translation =
        pick_product_translation(product.translations.as_slice(), locale, fallback_locale);

    AdminPricingProductListItem {
        id: product.id,
        status: map_product_status(product.status),
        seller_id: product.seller_id,
        title: translation
            .map(|translation| translation.title.clone())
            .unwrap_or_else(|| "Untitled".to_string()),
        handle: translation
            .map(|translation| translation.handle.clone())
            .unwrap_or_default(),
        vendor: product.vendor,
        product_type: product.product_type,
        shipping_profile_slug: product.shipping_profile_slug,
        tags: product.tags,
        created_at: product.created_at,
        published_at: product.published_at,
    }
}

pub(crate) fn map_admin_detail(
    product: ProductResponse,
    mut prices_by_variant: HashMap<Uuid, Vec<price::Model>>,
) -> AdminPricingProductDetail {
    AdminPricingProductDetail {
        id: product.id,
        status: map_product_status(product.status),
        seller_id: product.seller_id,
        vendor: product.vendor,
        product_type: product.product_type,
        shipping_profile_slug: product.shipping_profile_slug,
        created_at: product.created_at,
        updated_at: product.updated_at,
        published_at: product.published_at,
        translations: product
            .translations
            .into_iter()
            .map(|translation| AdminPricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: product
            .variants
            .into_iter()
            .map(|variant| AdminPricingVariant {
                id: variant.id,
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
                title: variant.title,
                combination_identity: variant.combination_identity,
                prices: prices_by_variant
                    .remove(&variant.id)
                    .map(|prices| {
                        prices
                            .into_iter()
                            .map(|price| AdminPricingPrice {
                                currency_code: price.currency_code,
                                amount: price.amount,
                                compare_at_amount: price.compare_at_amount,
                                discount_percent: calculate_discount_percent(
                                    price.amount,
                                    price.compare_at_amount,
                                ),
                                on_sale: is_sale_price(price.amount, price.compare_at_amount),
                                price_list_id: price.price_list_id,
                                channel_id: price.channel_id,
                                channel_slug: price.channel_slug,
                                min_quantity: price.min_quantity,
                                max_quantity: price.max_quantity,
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        variant
                            .prices
                            .into_iter()
                            .map(|price| AdminPricingPrice {
                                currency_code: price.currency_code,
                                amount: price.amount,
                                compare_at_amount: price.compare_at_amount,
                                discount_percent: calculate_discount_percent(
                                    price.amount,
                                    price.compare_at_amount,
                                ),
                                on_sale: price.on_sale,
                                price_list_id: None,
                                channel_id: None,
                                channel_slug: None,
                                min_quantity: None,
                                max_quantity: None,
                            })
                            .collect()
                    }),
            })
            .collect(),
    }
}

pub(crate) fn admin_pricing_price_from_model(price: price::Model) -> AdminPricingPrice {
    AdminPricingPrice {
        currency_code: price.currency_code,
        amount: price.amount,
        compare_at_amount: price.compare_at_amount,
        discount_percent: calculate_discount_percent(price.amount, price.compare_at_amount),
        on_sale: is_sale_price(price.amount, price.compare_at_amount),
        price_list_id: price.price_list_id,
        channel_id: price.channel_id,
        channel_slug: price.channel_slug,
        min_quantity: price.min_quantity,
        max_quantity: price.max_quantity,
    }
}

pub(crate) fn pick_product_translation<'a>(
    translations: &'a [ProductTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a ProductTranslationResponse> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, fallback_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn map_product_status(
    status: rustok_product::ProductStatus,
) -> ProductStatus {
    match status {
        rustok_product::ProductStatus::Draft => ProductStatus::Draft,
        rustok_product::ProductStatus::Active => ProductStatus::Active,
        rustok_product::ProductStatus::Archived => ProductStatus::Archived,
    }
}

pub(crate) fn map_foundation_product_status(
    status: ProductStatus,
) -> rustok_product::ProductStatus {
    match status {
        ProductStatus::Draft => rustok_product::ProductStatus::Draft,
        ProductStatus::Active => rustok_product::ProductStatus::Active,
        ProductStatus::Archived => rustok_product::ProductStatus::Archived,
    }
}

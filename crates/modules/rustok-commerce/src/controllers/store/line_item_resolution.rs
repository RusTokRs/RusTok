use rustok_api::{PortContext, PortError};
use rustok_inventory::{
    PublicChannelInventoryVariantProjectionInput, check_variant_availability_for_public_channel,
};
use rustok_pricing::ResolveProductPriceRequest;
use rustok_product::{
    ProductCatalogReadPort, ProductFulfillmentRequirement,
    StorefrontVariantProductProjectionRequest,
};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::controllers::store::{ResolvedStoreLineItemInput, StoreLineItemResolution};
use crate::{
    CommerceError, dto::AddCartLineItemInput, storefront_shipping::effective_shipping_profile_slug,
};

fn storefront_product_port_context(
    tenant_id: Uuid,
    locale: &str,
    public_channel_slug: Option<&str>,
    variant_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("rustok-commerce.storefront-line-item-product"),
        locale,
        format!("commerce-storefront-line-item:product:{variant_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match public_channel_slug {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

/// Bounded projection of a storefront line-item port failure.
///
/// The line-item boundary sits in front of the Product catalog read and the Pricing read, and each
/// of them fails with its own typed `PortError`. What they share is what this struct keeps: the
/// shape of the caller's port context, non-nil identity flags, the typed failure label, and the
/// owner code length. No tenant id, channel slug, variant id, product id, owner code, or owner
/// message is ever carried here.
struct StorefrontLineItemPortFacts {
    correlation_id_length: usize,
    tenant_id_length: usize,
    channel_present: bool,
    variant_id_non_nil: bool,
    product_id_non_nil: Option<bool>,
    owner_error_kind: &'static str,
    owner_code_length: usize,
    retryable: bool,
}

impl StorefrontLineItemPortFacts {
    fn new(
        error: &PortError,
        context: &PortContext,
        variant_id: Uuid,
        product_id: Option<Uuid>,
    ) -> Self {
        Self {
            correlation_id_length: context.correlation_id.chars().count(),
            tenant_id_length: context.tenant_id.chars().count(),
            channel_present: context.channel.is_some(),
            variant_id_non_nil: !variant_id.is_nil(),
            product_id_non_nil: product_id.map(|value| !value.is_nil()),
            owner_error_kind: storefront_line_item_port_error_kind(&error.kind),
            owner_code_length: error.code.chars().count(),
            retryable: error.retryable,
        }
    }
}

fn storefront_line_item_port_error_kind(kind: &rustok_api::PortErrorKind) -> &'static str {
    match kind {
        rustok_api::PortErrorKind::Validation => "validation",
        rustok_api::PortErrorKind::NotFound => "not_found",
        rustok_api::PortErrorKind::Conflict => "conflict",
        rustok_api::PortErrorKind::Forbidden => "forbidden",
        rustok_api::PortErrorKind::Unavailable => "unavailable",
        rustok_api::PortErrorKind::Timeout => "timeout",
        rustok_api::PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

/// Severity follows the public status, exactly like the other storefront mappers: transport and
/// invariant failures (503/504/500) are technical and log at `error`, while a rejected caller
/// request (400/403/404/409) logs at `warn` so expected input never produces an error storm.
fn is_technical_storefront_status(status: axum::http::StatusCode) -> bool {
    matches!(
        status,
        axum::http::StatusCode::SERVICE_UNAVAILABLE
            | axum::http::StatusCode::GATEWAY_TIMEOUT
            | axum::http::StatusCode::INTERNAL_SERVER_ERROR
    )
}

fn log_storefront_line_item_port_error(
    owner: &'static str,
    operation: &'static str,
    technical_event: &'static str,
    rejected_event: &'static str,
    facts: &StorefrontLineItemPortFacts,
    public_status: axum::http::StatusCode,
) {
    if is_technical_storefront_status(public_status) {
        tracing::error!(
            owner,
            operation,
            correlation_id_length = facts.correlation_id_length,
            tenant_id_length = facts.tenant_id_length,
            channel_present = facts.channel_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            product_id_non_nil = ?facts.product_id_non_nil,
            owner_error_kind = facts.owner_error_kind,
            owner_code_length = facts.owner_code_length,
            retryable = facts.retryable,
            public_status = %public_status,
            boundary = "commerce_storefront_line_item_http",
            technical_event
        );
    } else {
        tracing::warn!(
            owner,
            operation,
            correlation_id_length = facts.correlation_id_length,
            tenant_id_length = facts.tenant_id_length,
            channel_present = facts.channel_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            product_id_non_nil = ?facts.product_id_non_nil,
            owner_error_kind = facts.owner_error_kind,
            owner_code_length = facts.owner_code_length,
            retryable = facts.retryable,
            public_status = %public_status,
            boundary = "commerce_storefront_line_item_http",
            rejected_event
        );
    }
}

fn map_storefront_line_item_product_port_error(
    error: PortError,
    context: &PortContext,
    variant_id: Uuid,
) -> HttpError {
    let facts = StorefrontLineItemPortFacts::new(&error, context, variant_id, None);
    let public = port_error_to_http_error(error);
    log_storefront_line_item_port_error(
        "rustok_product",
        "read_variant_product_projection",
        "storefront line item Product owner read failed with bounded diagnostics",
        "storefront line item Product owner read was rejected with bounded diagnostics",
        &facts,
        public.status,
    );
    public
}

fn pick_product_translation_response<'a>(
    translations: &'a [crate::dto::ProductTranslationResponse],
    locale: &str,
    default_locale: &str,
) -> Option<&'a crate::dto::ProductTranslationResponse> {
    translations
        .iter()
        .find(|translation| rustok_api::locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!rustok_api::locale_tags_match(default_locale, locale)).then(|| {
                translations.iter().find(|translation| {
                    rustok_api::locale_tags_match(&translation.locale, default_locale)
                })
            })?
        })
        .or_else(|| translations.first())
}

fn pick_variant_translation_response<'a>(
    translations: &'a [crate::dto::VariantTranslationResponse],
    locale: &str,
    default_locale: &str,
) -> Option<&'a crate::dto::VariantTranslationResponse> {
    translations
        .iter()
        .find(|translation| rustok_api::locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!rustok_api::locale_tags_match(default_locale, locale)).then(|| {
                translations.iter().find(|translation| {
                    rustok_api::locale_tags_match(&translation.locale, default_locale)
                })
            })?
        })
        .or_else(|| translations.first())
}

fn map_storefront_line_item_pricing_error(
    error: PortError,
    context: &PortContext,
    variant_id: Uuid,
    product_id: Uuid,
) -> HttpError {
    let facts = StorefrontLineItemPortFacts::new(&error, context, variant_id, Some(product_id));
    let public = port_error_to_http_error(error);
    log_storefront_line_item_port_error(
        "rustok_pricing",
        "resolve_product_price",
        "storefront line item pricing resolution failed with bounded diagnostics",
        "storefront line item pricing resolution was rejected with bounded diagnostics",
        &facts,
        public.status,
    );
    public
}

fn map_storefront_line_item_inventory_error(
    error: CommerceError,
    operation: &'static str,
    tenant_id: Uuid,
    variant_id: Uuid,
    product_id: Uuid,
    public_channel_slug: Option<&str>,
    locale: Option<&str>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        CommerceError::Validation(_) => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_store_inventory_invalid",
            "Inventory request is invalid",
            "validation",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::ShippingProfileNotFound(_) => (
            axum::http::StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::InsufficientInventory { .. } => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_store_inventory_insufficient",
            "Requested quantity is not available",
            "insufficient_inventory",
        ),
        CommerceError::Database(_) => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_inventory_unavailable",
            "Inventory service is temporarily unavailable",
            "database",
        ),
        CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InvalidPrice(_)
        | CommerceError::InvalidOptionCombination
        | CommerceError::DuplicateShippingProfileSlug(_)
        | CommerceError::NoVariants
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_inventory_failed",
            "Inventory operation could not be completed safely",
            "unexpected_owner_error",
        ),
    };
    if is_technical_storefront_status(status) {
        tracing::error!(
            owner = "rustok_inventory.public_channel",
            operation,
            tenant_id_non_nil = !tenant_id.is_nil(),
            variant_id_non_nil = !variant_id.is_nil(),
            product_id_non_nil = !product_id.is_nil(),
            channel_present = public_channel_slug.is_some(),
            channel_length = public_channel_slug.map(str::len),
            locale_present = locale.is_some(),
            locale_length = locale.map(str::len),
            error_kind,
            owner_code_length = code.chars().count(),
            public_status = %status,
            boundary = "commerce_storefront_line_item_http",
            "storefront line item inventory operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_inventory.public_channel",
            operation,
            tenant_id_non_nil = !tenant_id.is_nil(),
            variant_id_non_nil = !variant_id.is_nil(),
            product_id_non_nil = !product_id.is_nil(),
            channel_present = public_channel_slug.is_some(),
            channel_length = public_channel_slug.map(str::len),
            locale_present = locale.is_some(),
            locale_length = locale.map(str::len),
            error_kind,
            owner_code_length = code.chars().count(),
            public_status = %status,
            boundary = "commerce_storefront_line_item_http",
            "storefront line item inventory operation was rejected with bounded diagnostics"
        );
    }
    HttpError::new(status, code, message)
}

pub(crate) async fn resolve_store_line_item_input(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    resolution: StoreLineItemResolution<'_>,
) -> HttpResult<ResolvedStoreLineItemInput> {
    let StoreLineItemResolution {
        product_catalog_read_port,
        pricing_read_port,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    } = resolution;

    let port_context =
        storefront_product_port_context(tenant_id, locale, public_channel_slug, input.variant_id);
    let product = product_catalog_read_port
        .read_storefront_variant_product_projection(
            port_context.clone(),
            StorefrontVariantProductProjectionRequest {
                variant_id: input.variant_id,
                locale: Some(locale.to_string()),
                fallback_locale: Some(default_locale.to_string()),
                public_channel_slug: public_channel_slug.map(str::to_owned),
            },
        )
        .await
        .map_err(|error| {
            map_storefront_line_item_product_port_error(error, &port_context, input.variant_id)
        })?
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    let variant = product
        .variants
        .iter()
        .find(|variant| variant.id == input.variant_id)
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    let pricing_port_context = crate::controllers::store::store_line_item_pricing_port_context(
        tenant_id,
        variant.id,
        locale,
        pricing_context,
    );
    let resolved_price: rustok_pricing::ResolvedPrice = pricing_read_port
        .resolve_product_price(
            pricing_port_context.clone(),
            ResolveProductPriceRequest {
                product_id: Some(product.id),
                variant_id: variant.id,
                region_id: pricing_context.region_id,
                channel_id: pricing_context.channel_id,
                channel_slug: pricing_context.channel_slug.clone(),
                price_list_id: pricing_context.price_list_id,
                quantity: pricing_context.quantity,
                currency_code: pricing_context.currency_code.clone(),
            },
        )
        .await
        .map_err(|error| {
            map_storefront_line_item_pricing_error(
                error,
                &pricing_port_context,
                variant.id,
                product.id,
            )
        })?
        .into();

    let (base_unit_price, pricing_adjustment) =
        crate::controllers::store::storefront_cart_pricing_snapshot(
            input.quantity,
            &resolved_price,
        );

    validate_store_variant_inventory(
        db,
        tenant_id,
        variant.id,
        &variant.inventory_policy,
        input.quantity,
        public_channel_slug,
        Some(locale),
        product.id,
    )
    .await?;

    let base_title =
        pick_product_translation_response(&product.translations, locale, default_locale)
            .map(|translation| translation.title.clone())
            .unwrap_or_else(|| {
                variant
                    .sku
                    .clone()
                    .unwrap_or_else(|| format!("Variant {}", variant.id))
            });

    let title =
        match pick_variant_translation_response(&variant.translations, locale, default_locale)
            .and_then(|translation| translation.title.clone())
        {
            Some(variant_title) if !variant_title.trim().is_empty() => {
                format!("{base_title} / {}", variant_title.trim())
            }
            _ => base_title,
        };

    let fulfillment_requirement = product.fulfillment_requirement;
    let shipping_profile_slug = match fulfillment_requirement {
        ProductFulfillmentRequirement::Digital => None,
        ProductFulfillmentRequirement::Physical => Some(effective_shipping_profile_slug(
            product.shipping_profile_slug.as_deref(),
            &product.metadata,
            variant.shipping_profile_slug.as_deref(),
        )),
    };

    Ok(ResolvedStoreLineItemInput {
        add_line_item: AddCartLineItemInput {
            product_id: Some(product.id),
            variant_id: Some(variant.id),
            fulfillment_requirement: match fulfillment_requirement {
                ProductFulfillmentRequirement::Digital => {
                    rustok_cart::CartLineFulfillmentRequirement::Digital
                }
                ProductFulfillmentRequirement::Physical => {
                    rustok_cart::CartLineFulfillmentRequirement::Physical
                }
            },
            shipping_profile_slug,
            sku: variant.sku.clone(),
            title,
            quantity: input.quantity,
            unit_price: base_unit_price,
            metadata: crate::controllers::store::merge_metadata(
                input.metadata,
                crate::controllers::store::seller_snapshot_metadata(product.seller_id.as_deref()),
            ),
        },
        pricing_adjustment,
    })
}

pub(crate) async fn validate_store_line_item_quantity(
    db: &DatabaseConnection,
    product_catalog_read_port: &dyn ProductCatalogReadPort,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> HttpResult<()> {
    let port_context =
        storefront_product_port_context(tenant_id, "en", public_channel_slug, variant_id);
    let product = product_catalog_read_port
        .read_storefront_variant_product_projection(
            port_context.clone(),
            StorefrontVariantProductProjectionRequest {
                variant_id,
                locale: Some("en".to_string()),
                fallback_locale: Some(rustok_api::PLATFORM_FALLBACK_LOCALE.to_string()),
                public_channel_slug: public_channel_slug.map(str::to_owned),
            },
        )
        .await
        .map_err(|error| {
            map_storefront_line_item_product_port_error(error, &port_context, variant_id)
        })?
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    let variant = product
        .variants
        .iter()
        .find(|variant| variant.id == variant_id)
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    validate_store_variant_inventory(
        db,
        tenant_id,
        variant.id,
        &variant.inventory_policy,
        requested_quantity,
        public_channel_slug,
        None,
        product.id,
    )
    .await
}

async fn validate_store_variant_inventory(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    inventory_policy: &str,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
    locale: Option<&str>,
    product_id: Uuid,
) -> HttpResult<()> {
    let available = check_variant_availability_for_public_channel(
        db,
        tenant_id,
        PublicChannelInventoryVariantProjectionInput {
            variant_id,
            inventory_policy,
        },
        requested_quantity,
        public_channel_slug,
    )
    .await
    .map_err(|error| {
        map_storefront_line_item_inventory_error(
            error,
            "check_variant_availability",
            tenant_id,
            variant_id,
            product_id,
            public_channel_slug,
            locale,
        )
    })?;
    if !available {
        return Err(HttpError::bad_request(
            "commerce_store_inventory_insufficient",
            "Requested quantity is not available",
        ));
    }

    Ok(())
}

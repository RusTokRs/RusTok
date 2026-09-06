use rustok_api::{PortContext, PortError};
use rustok_inventory::{
    PublicChannelInventoryVariantProjectionInput, check_variant_availability_for_public_channel,
};
use rustok_pricing::ResolveProductPriceRequest;
use rustok_product::entities::{
    product, product_translation, product_variant, variant_translation,
};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::controllers::store::{ResolvedStoreLineItemInput, StoreLineItemResolution};
use crate::{
    CommerceError, dto::AddCartLineItemInput,
    storefront_channel::is_metadata_visible_for_public_channel,
    storefront_shipping::effective_shipping_profile_slug,
};

fn map_storefront_line_item_database_error(
    error: DbErr,
    operation: &'static str,
    tenant_id: Uuid,
    variant_id: Option<Uuid>,
    product_id: Option<Uuid>,
    public_channel_slug: Option<&str>,
    locale: Option<&str>,
) -> HttpError {
    let status = axum::http::StatusCode::SERVICE_UNAVAILABLE;
    let code = "commerce_store_catalog_unavailable";
    tracing::error!(
        error = ?error,
        owner = "rustok_product.persistence",
        operation,
        tenant_id = %tenant_id,
        variant_id = ?variant_id,
        product_id = ?product_id,
        channel = ?public_channel_slug,
        locale = ?locale,
        error_kind = "database",
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_line_item_http",
        "storefront line item catalog read failed"
    );
    HttpError::new(status, code, "Store catalog is temporarily unavailable")
}

fn map_storefront_line_item_pricing_error(
    error: PortError,
    context: &PortContext,
    variant_id: Uuid,
    product_id: Uuid,
) -> HttpError {
    let public = port_error_to_http_error(error.clone());
    tracing::error!(
        error = ?error,
        owner = "rustok_pricing",
        operation = "resolve_product_price",
        tenant_id = %context.tenant_id,
        correlation_id = %context.correlation_id,
        channel = ?context.channel,
        variant_id = %variant_id,
        product_id = %product_id,
        error_kind = ?error.kind,
        retryable = error.retryable,
        public_code = %public.code,
        status = %public.status,
        boundary = "commerce_storefront_line_item_http",
        "storefront line item pricing resolution failed"
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
    tracing::error!(
        error = ?error,
        owner = "rustok_inventory.public_channel",
        operation,
        tenant_id = %tenant_id,
        variant_id = %variant_id,
        product_id = %product_id,
        channel = ?public_channel_slug,
        locale = ?locale,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_line_item_http",
        "storefront line item inventory operation failed"
    );
    HttpError::new(status, code, message)
}

pub(crate) async fn resolve_store_line_item_input(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    resolution: StoreLineItemResolution<'_>,
) -> HttpResult<ResolvedStoreLineItemInput> {
    let StoreLineItemResolution {
        pricing_read_port,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    } = resolution;

    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| {
            map_storefront_line_item_database_error(
                error,
                "load_variant",
                tenant_id,
                Some(input.variant_id),
                None,
                public_channel_slug,
                Some(locale),
            )
        })?
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| {
            map_storefront_line_item_database_error(
                error,
                "load_product",
                tenant_id,
                Some(variant.id),
                Some(variant.product_id),
                public_channel_slug,
                Some(locale),
            )
        })?
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ));
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await
        .map_err(|error| {
            map_storefront_line_item_database_error(
                error,
                "load_product_translations",
                tenant_id,
                Some(variant.id),
                Some(product_model.id),
                public_channel_slug,
                Some(locale),
            )
        })?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await
        .map_err(|error| {
            map_storefront_line_item_database_error(
                error,
                "load_variant_translations",
                tenant_id,
                Some(variant.id),
                Some(product_model.id),
                public_channel_slug,
                Some(locale),
            )
        })?;

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
                product_id: Some(product_model.id),
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
                product_model.id,
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
        &variant,
        input.quantity,
        public_channel_slug,
        Some(locale),
    )
    .await?;

    let base_title = crate::controllers::store::pick_product_translation(
        &product_translation_models,
        locale,
        default_locale,
    )
    .map(|translation| translation.title.clone())
    .unwrap_or_else(|| {
        variant
            .sku
            .clone()
            .unwrap_or_else(|| format!("Variant {}", variant.id))
    });
    let title = match crate::controllers::store::pick_variant_translation(
        &variant_translation_models,
        locale,
        default_locale,
    )
    .and_then(|translation| translation.title.clone())
    {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(ResolvedStoreLineItemInput {
        add_line_item: AddCartLineItemInput {
            product_id: Some(product_model.id),
            variant_id: Some(variant.id),
            shipping_profile_slug: Some(effective_shipping_profile_slug(
                product_model.shipping_profile_slug.as_deref(),
                &product_model.metadata,
                variant.shipping_profile_slug.as_deref(),
            )),
            sku: variant.sku.clone(),
            title,
            quantity: input.quantity,
            unit_price: base_unit_price,
            metadata: crate::controllers::store::merge_metadata(
                input.metadata,
                crate::controllers::store::seller_snapshot_metadata(
                    product_model.seller_id.as_deref(),
                ),
            ),
        },
        pricing_adjustment,
    })
}

pub(crate) async fn validate_store_line_item_quantity(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> HttpResult<()> {
    let variant = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| {
            map_storefront_line_item_database_error(
                error,
                "load_variant_for_quantity_validation",
                tenant_id,
                Some(variant_id),
                None,
                public_channel_slug,
                None,
            )
        })?
        .ok_or(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ))?;

    validate_store_variant_inventory(
        db,
        tenant_id,
        &variant,
        requested_quantity,
        public_channel_slug,
        None,
    )
    .await
}

async fn validate_store_variant_inventory(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
    locale: Option<&str>,
) -> HttpResult<()> {
    let available = check_variant_availability_for_public_channel(
        db,
        tenant_id,
        PublicChannelInventoryVariantProjectionInput {
            variant_id: variant.id,
            inventory_policy: &variant.inventory_policy,
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
            variant.id,
            variant.product_id,
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

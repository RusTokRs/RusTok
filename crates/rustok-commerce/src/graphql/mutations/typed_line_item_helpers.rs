use async_graphql::{ErrorExtensions, Result};
use rustok_api::{PortContext, PortError};
use rustok_inventory::{
    PublicChannelInventoryVariantProjectionInput, check_variant_availability_for_public_channel,
};
use rustok_pricing::{PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest};
use rustok_product::entities::{
    product, product_translation, product_variant, variant_translation,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CommerceError, storefront_channel::is_metadata_visible_for_public_channel,
    storefront_shipping::effective_shipping_profile_slug,
};

use super::super::types::AddStorefrontCartLineItemInput;
use super::legacy_helpers::ResolvedStorefrontLineItemInput;

const STOREFRONT_LINE_ITEM_GRAPHQL_BOUNDARY: &str = "commerce_graphql_storefront_line_item";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorefrontLineItemFailureKind {
    ProductUnavailable,
    InventoryInsufficient,
    InputInvalid,
    DependencyUnavailable,
}

enum StorefrontLineItemFailureSource {
    Database(sea_orm::DbErr),
    Pricing(PortError),
    Inventory(CommerceError),
    Metadata(serde_json::Error),
    Local(&'static str),
}

impl StorefrontLineItemFailureSource {
    fn kind(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            Self::Pricing(_) => "pricing_port",
            Self::Inventory(_) => "inventory_owner",
            Self::Metadata(_) => "metadata_json",
            Self::Local(_) => "local_policy",
        }
    }
}

struct StorefrontLineItemDiagnosticSource(StorefrontLineItemFailureSource);

impl From<StorefrontLineItemFailureSource> for StorefrontLineItemDiagnosticSource {
    fn from(source: StorefrontLineItemFailureSource) -> Self {
        Self(source)
    }
}

impl std::fmt::Debug for StorefrontLineItemDiagnosticSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            StorefrontLineItemFailureSource::Database(error) => formatter
                .debug_tuple("Database")
                .field(&error.to_string())
                .finish(),
            StorefrontLineItemFailureSource::Pricing(error) => formatter
                .debug_tuple("Pricing")
                .field(&error.to_string())
                .finish(),
            StorefrontLineItemFailureSource::Inventory(error) => formatter
                .debug_tuple("Inventory")
                .field(&error.to_string())
                .finish(),
            StorefrontLineItemFailureSource::Metadata(error) => formatter
                .debug_tuple("Metadata")
                .field(&error.to_string())
                .finish(),
            StorefrontLineItemFailureSource::Local(reason) => {
                formatter.debug_tuple("Local").field(reason).finish()
            }
        }
    }
}

struct StorefrontLineItemFailure {
    kind: StorefrontLineItemFailureKind,
    source_owner: &'static str,
    source_operation: &'static str,
    product_id: Option<Uuid>,
    source: StorefrontLineItemFailureSource,
}

impl StorefrontLineItemFailure {
    fn database(operation: &'static str, error: sea_orm::DbErr) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::DependencyUnavailable,
            source_owner: "rustok_product.persistence",
            source_operation: operation,
            product_id: None,
            source: StorefrontLineItemFailureSource::Database(error),
        }
    }

    fn pricing(product_id: Uuid, error: PortError) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::DependencyUnavailable,
            source_owner: "rustok_pricing",
            source_operation: "resolve_product_price",
            product_id: Some(product_id),
            source: StorefrontLineItemFailureSource::Pricing(error),
        }
    }

    fn inventory(product_id: Uuid, error: CommerceError) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::DependencyUnavailable,
            source_owner: "rustok_inventory.public_channel",
            source_operation: "check_variant_availability",
            product_id: Some(product_id),
            source: StorefrontLineItemFailureSource::Inventory(error),
        }
    }

    fn product_unavailable(
        product_id: Option<Uuid>,
        operation: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::ProductUnavailable,
            source_owner: "rustok_product",
            source_operation: operation,
            product_id,
            source: StorefrontLineItemFailureSource::Local(reason),
        }
    }

    fn inventory_insufficient(product_id: Uuid) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::InventoryInsufficient,
            source_owner: "rustok_inventory.public_channel",
            source_operation: "check_variant_availability",
            product_id: Some(product_id),
            source: StorefrontLineItemFailureSource::Local("available inventory is below request"),
        }
    }

    fn invalid_metadata(product_id: Uuid, error: serde_json::Error) -> Self {
        Self {
            kind: StorefrontLineItemFailureKind::InputInvalid,
            source_owner: "rustok_commerce.graphql_input",
            source_operation: "parse_line_item_metadata",
            product_id: Some(product_id),
            source: StorefrontLineItemFailureSource::Metadata(error),
        }
    }

    fn with_product_id(mut self, product_id: Uuid) -> Self {
        self.product_id = Some(product_id);
        self
    }
}

#[derive(Clone, Copy)]
enum StorefrontLineItemConsumerOperation {
    Resolve,
    ValidateQuantity,
}

impl StorefrontLineItemConsumerOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Resolve => "resolve_storefront_line_item_input",
            Self::ValidateQuantity => "validate_storefront_line_item_quantity",
        }
    }
}

fn public_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn storefront_line_item_public_policy(
    consumer_operation: StorefrontLineItemConsumerOperation,
    failure_kind: StorefrontLineItemFailureKind,
) -> (&'static str, &'static str, bool) {
    match (consumer_operation, failure_kind) {
        (_, StorefrontLineItemFailureKind::ProductUnavailable) => (
            "Product is not available",
            "CART_PRODUCT_UNAVAILABLE",
            false,
        ),
        (_, StorefrontLineItemFailureKind::InventoryInsufficient) => (
            "Requested quantity is not available",
            "CART_INVENTORY_INSUFFICIENT",
            false,
        ),
        (
            StorefrontLineItemConsumerOperation::Resolve,
            StorefrontLineItemFailureKind::InputInvalid,
        ) => (
            "Cart line item input is invalid",
            "CART_LINE_ITEM_INVALID",
            false,
        ),
        (StorefrontLineItemConsumerOperation::Resolve, _) => (
            "Cart line item could not be resolved",
            "CART_LINE_ITEM_RESOLUTION_FAILED",
            true,
        ),
        (StorefrontLineItemConsumerOperation::ValidateQuantity, _) => (
            "Inventory availability could not be verified",
            "CART_INVENTORY_UNAVAILABLE",
            true,
        ),
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn optional_text_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "empty",
        Some(_) => "present",
    }
}

#[allow(clippy::too_many_arguments)]
fn storefront_line_item_graphql_error(
    failure: StorefrontLineItemFailure,
    consumer_operation: StorefrontLineItemConsumerOperation,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
    locale: Option<&str>,
    correlation_id: Option<&str>,
) -> async_graphql::Error {
    let (message, code, retryable) =
        storefront_line_item_public_policy(consumer_operation, failure.kind);
    let StorefrontLineItemFailure {
        kind,
        source_owner,
        source_operation,
        product_id,
        source,
    } = failure;
    let failure_kind = match kind {
        StorefrontLineItemFailureKind::ProductUnavailable => "product_unavailable",
        StorefrontLineItemFailureKind::InventoryInsufficient => "inventory_insufficient",
        StorefrontLineItemFailureKind::InputInvalid => "input_invalid",
        StorefrontLineItemFailureKind::DependencyUnavailable => "dependency_unavailable",
    };
    let source_kind = source.kind();
    let source = StorefrontLineItemDiagnosticSource::from(source);
    let correlation_id_shape = optional_text_shape(correlation_id);
    let tenant_id_shape = uuid_shape(tenant_id);
    let variant_id_shape = uuid_shape(variant_id);
    let product_id_shape = optional_uuid_shape(product_id);
    let channel_slug_length = public_channel_slug.map(|value| value.chars().count());
    let locale_length = locale.map(|value| value.chars().count());

    match kind {
        StorefrontLineItemFailureKind::DependencyUnavailable => tracing::error!(
            source = ?source,
            source_kind,
            owner = source_owner,
            owner_operation = source_operation,
            consumer_operation = consumer_operation.name(),
            failure_kind,
            correlation_id_shape,
            tenant_id_shape,
            variant_id_shape,
            product_id_shape,
            requested_quantity,
            channel_slug_length = ?channel_slug_length,
            locale_length = ?locale_length,
            public_code = code,
            public_retryable = retryable,
            boundary = STOREFRONT_LINE_ITEM_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront line item dependency failed"
        ),
        _ => tracing::warn!(
            source = ?source,
            source_kind,
            owner = source_owner,
            owner_operation = source_operation,
            consumer_operation = consumer_operation.name(),
            failure_kind,
            correlation_id_shape,
            tenant_id_shape,
            variant_id_shape,
            product_id_shape,
            requested_quantity,
            channel_slug_length = ?channel_slug_length,
            locale_length = ?locale_length,
            public_code = code,
            public_retryable = retryable,
            boundary = STOREFRONT_LINE_ITEM_GRAPHQL_BOUNDARY,
            "commerce GraphQL storefront line item request was rejected"
        ),
    }

    public_graphql_error(message, code, retryable)
}

fn parse_line_item_metadata(
    value: Option<&str>,
    product_id: Uuid,
) -> std::result::Result<Value, StorefrontLineItemFailure> {
    match value.map(str::trim) {
        None | Some("") => Ok(Value::Object(Default::default())),
        Some(value) => serde_json::from_str(value)
            .map_err(|error| StorefrontLineItemFailure::invalid_metadata(product_id, error)),
    }
}

#[allow(clippy::too_many_arguments)]
async fn resolve_typed_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_read_port: &dyn PricingReadPort,
    pricing_port_context: PortContext,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: AddStorefrontCartLineItemInput,
) -> std::result::Result<ResolvedStorefrontLineItemInput, StorefrontLineItemFailure> {
    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| StorefrontLineItemFailure::database("load_variant", error))?
        .ok_or_else(|| {
            StorefrontLineItemFailure::product_unavailable(
                None,
                "load_variant",
                "variant was not found",
            )
        })?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| {
            StorefrontLineItemFailure::database("load_product", error)
                .with_product_id(variant.product_id)
        })?
        .ok_or_else(|| {
            StorefrontLineItemFailure::product_unavailable(
                Some(variant.product_id),
                "load_product",
                "product was not found",
            )
        })?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(StorefrontLineItemFailure::product_unavailable(
            Some(product_model.id),
            "validate_product_visibility",
            "product is not publicly available",
        ));
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await
        .map_err(|error| {
            StorefrontLineItemFailure::database("load_product_translations", error)
                .with_product_id(product_model.id)
        })?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await
        .map_err(|error| {
            StorefrontLineItemFailure::database("load_variant_translations", error)
                .with_product_id(product_model.id)
        })?;

    let resolved_price: rustok_pricing::ResolvedPrice = pricing_read_port
        .resolve_product_price(
            pricing_port_context,
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
        .map_err(|error| StorefrontLineItemFailure::pricing(product_model.id, error))?
        .into();
    let (base_unit_price, pricing_adjustment) =
        super::legacy_helpers::storefront_cart_pricing_snapshot(input.quantity, &resolved_price);
    validate_typed_storefront_variant_inventory(
        db,
        tenant_id,
        &variant,
        input.quantity,
        public_channel_slug,
    )
    .await?;

    let base_title = super::legacy_helpers::pick_product_translation(
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
    let title = match super::legacy_helpers::pick_variant_translation(
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

    Ok(ResolvedStorefrontLineItemInput {
        add_line_item: crate::dto::AddCartLineItemInput {
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
            metadata: super::legacy_helpers::merge_graphql_metadata(
                parse_line_item_metadata(input.metadata.as_deref(), product_model.id)?,
                super::legacy_helpers::seller_snapshot_metadata(product_model.seller_id.as_deref()),
            ),
        },
        pricing_adjustment,
    })
}

async fn validate_typed_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> std::result::Result<(), StorefrontLineItemFailure> {
    let variant = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| {
            StorefrontLineItemFailure::database("load_variant_for_quantity_validation", error)
        })?
        .ok_or_else(|| {
            StorefrontLineItemFailure::product_unavailable(
                None,
                "load_variant_for_quantity_validation",
                "variant was not found",
            )
        })?;
    validate_typed_storefront_variant_inventory(
        db,
        tenant_id,
        &variant,
        requested_quantity,
        public_channel_slug,
    )
    .await
}

async fn validate_typed_storefront_variant_inventory(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> std::result::Result<(), StorefrontLineItemFailure> {
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
    .map_err(|error| StorefrontLineItemFailure::inventory(variant.product_id, error))?;
    if !available {
        return Err(StorefrontLineItemFailure::inventory_insufficient(
            variant.product_id,
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn resolve_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_read_port: &dyn PricingReadPort,
    pricing_port_context: PortContext,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: AddStorefrontCartLineItemInput,
) -> Result<ResolvedStorefrontLineItemInput> {
    let variant_id = input.variant_id;
    let requested_quantity = input.quantity;
    let error_context = pricing_port_context.clone();
    resolve_typed_storefront_line_item_input(
        db,
        tenant_id,
        pricing_read_port,
        pricing_port_context,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    )
    .await
    .map_err(|failure| {
        storefront_line_item_graphql_error(
            failure,
            StorefrontLineItemConsumerOperation::Resolve,
            tenant_id,
            variant_id,
            requested_quantity,
            public_channel_slug,
            Some(locale),
            Some(error_context.correlation_id.as_str()),
        )
    })
}

pub(crate) async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    validate_typed_storefront_line_item_quantity(
        db,
        tenant_id,
        variant_id,
        requested_quantity,
        public_channel_slug,
    )
    .await
    .map_err(|failure| {
        storefront_line_item_graphql_error(
            failure,
            StorefrontLineItemConsumerOperation::ValidateQuantity,
            tenant_id,
            variant_id,
            requested_quantity,
            public_channel_slug,
            None,
            None,
        )
    })
}

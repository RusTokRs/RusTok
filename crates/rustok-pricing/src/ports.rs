use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport-neutral owner boundary for pricing read projections.
#[async_trait]
pub trait PricingReadPort: Send + Sync {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError>;

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveProductPriceRequest {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriceListProjectionRequest {
    pub price_list_id: Uuid,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedProductPriceSnapshot {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub price_list_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub currency_code: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[async_trait]
impl PricingReadPort for crate::PricingService {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let variant_id = request.variant_id.ok_or_else(|| {
            PortError::validation(
                "pricing.variant_id_required",
                "resolve_product_price currently requires a variant_id boundary key",
            )
        })?;

        let resolved = self
            .resolve_variant_price(
                tenant_id,
                variant_id,
                crate::PriceResolutionContext {
                    currency_code: request.currency_code,
                    region_id: request.region_id,
                    price_list_id: None,
                    channel_id: request.channel_id,
                    channel_slug: None,
                    quantity: None,
                },
            )
            .await
            .map_err(pricing_error_to_port_error)?
            .ok_or_else(|| {
                PortError::new(
                    rustok_api::PortErrorKind::NotFound,
                    "pricing.price_not_found",
                    format!("price for variant {variant_id} was not found"),
                    false,
                )
            })?;

        Ok(ResolvedProductPriceSnapshot {
            product_id: request.product_id,
            variant_id: Some(variant_id),
            currency_code: resolved.currency_code,
            amount: resolved.amount,
            price_list_id: resolved.price_list_id,
        })
    }

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        let lists = self
            .list_active_price_lists(tenant_id, Some(locale), Some(locale))
            .await
            .map_err(pricing_error_to_port_error)?;
        let list = lists
            .into_iter()
            .find(|list| list.id == request.price_list_id)
            .ok_or_else(|| {
                PortError::new(
                    rustok_api::PortErrorKind::NotFound,
                    "pricing.price_list_not_found",
                    format!("price list {} was not found", request.price_list_id),
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
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "pricing.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for pricing ports",
        )
    })
}

fn pricing_error_to_port_error(
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(error) => PortError::unavailable(
            "pricing.database_unavailable",
            format!("pricing storage unavailable: {error}"),
        ),
        CommerceError::ProductNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "pricing.product_not_found",
            format!("product {id} not found"),
            false,
        ),
        CommerceError::VariantNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "pricing.variant_not_found",
            format!("variant {id} not found"),
            false,
        ),
        CommerceError::DuplicateHandle { handle, locale } => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "pricing.duplicate_handle",
            format!("duplicate handle `{handle}` for locale `{locale}`"),
            false,
        ),
        CommerceError::DuplicateSku(sku) => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "pricing.duplicate_sku",
            format!("duplicate sku `{sku}`"),
            false,
        ),
        CommerceError::InvalidPrice(message) | CommerceError::Validation(message) => {
            PortError::validation("pricing.validation", message)
        }
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "pricing.insufficient_inventory",
            format!("insufficient inventory: requested {requested}, available {available}"),
            false,
        ),
        CommerceError::InvalidOptionCombination => PortError::validation(
            "pricing.invalid_option_combination",
            "invalid option combination",
        ),
        CommerceError::ShippingProfileNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "pricing.shipping_profile_not_found",
            format!("shipping profile {id} not found"),
            false,
        ),
        CommerceError::DuplicateShippingProfileSlug(slug) => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "pricing.duplicate_shipping_profile_slug",
            format!("duplicate shipping profile slug `{slug}`"),
            false,
        ),
        CommerceError::NoVariants => PortError::validation(
            "pricing.no_variants",
            "product must have at least one variant",
        ),
        CommerceError::CannotDeletePublished => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "pricing.cannot_delete_published",
            "cannot delete published product",
            false,
        ),
        CommerceError::Rich(error) => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "pricing.rich_error",
            error.to_string(),
            false,
        ),
        CommerceError::Core(error) => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "pricing.core_error",
            error.to_string(),
            false,
        ),
    }
}

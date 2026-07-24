use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

/// Builds the owner-managed in-process pricing read provider for explicit consumers.
pub fn in_process_pricing_read_port(
    db: sea_orm::DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn PricingReadPort> {
    Arc::new(crate::PricingService::new(db, event_bus))
}

pub fn in_process_pricing_write_port(
    db: sea_orm::DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn PricingWritePort> {
    Arc::new(crate::PricingService::new(db, event_bus))
}

#[async_trait]
pub trait PricingWritePort: Send + Sync {
    async fn upsert_variant_price(
        &self,
        context: PortContext,
        request: UpsertVariantPriceRequest,
    ) -> Result<crate::AdminPricingPrice, PortError>;

    async fn set_price_list_scope(
        &self,
        context: PortContext,
        request: SetPriceListScopeRequest,
    ) -> Result<crate::ActivePriceListOption, PortError>;
    async fn apply_variant_discount(
        &self,
        context: PortContext,
        request: ApplyVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError>;

    async fn set_price_list_percentage_rule(
        &self,
        context: PortContext,
        request: SetPriceListPercentageRuleRequest,
    ) -> Result<crate::ActivePriceListOption, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpsertVariantPriceRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetPriceListScopeRequest {
    pub price_list_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplyVariantDiscountRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub discount_percent: Decimal,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPriceListPercentageRuleRequest {
    pub price_list_id: Uuid,
    pub adjustment_percent: Option<Decimal>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveProductPriceRequest {
    pub product_id: Option<Uuid>,
    pub variant_id: Uuid,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub price_list_id: Option<Uuid>,
    pub quantity: Option<i32>,
    pub currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriceListProjectionRequest {
    pub price_list_id: Uuid,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivePriceListProjectionRequest {
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminProductPricingProjectionRequest {
    pub product_id: Uuid,
    pub fallback_locale: Option<String>,
    pub selected_price_list_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorefrontProductPricingProjectionRequest {
    pub handle: String,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewVariantDiscountRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub discount_percent: Decimal,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedProductPriceSnapshot {
    pub product_id: Option<Uuid>,
    pub variant_id: Uuid,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
    pub region_id: Option<Uuid>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

impl From<ResolvedProductPriceSnapshot> for crate::ResolvedPrice {
    fn from(snapshot: ResolvedProductPriceSnapshot) -> Self {
        Self {
            currency_code: snapshot.currency_code,
            amount: snapshot.amount,
            compare_at_amount: snapshot.compare_at_amount,
            discount_percent: snapshot.discount_percent,
            on_sale: snapshot.on_sale,
            region_id: snapshot.region_id,
            min_quantity: snapshot.min_quantity,
            max_quantity: snapshot.max_quantity,
            price_list_id: snapshot.price_list_id,
            channel_id: snapshot.channel_id,
            channel_slug: snapshot.channel_slug,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub currency_code: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivePriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub list_type: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<Decimal>,
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
        let variant_id = request.variant_id;

        // Resolve the tenant-owned product projection first and verify that the
        // boundary keys describe the same aggregate. Previously the port resolved
        // only variant_id and then echoed the caller-provided product_id, allowing a
        // valid variant price to be mislabeled as belonging to another product.
        if let Some(product_id) = request.product_id {
            let locale = context.locale.as_str();
            let product = self
                .get_admin_product_pricing_with_locale_fallback(
                    tenant_id,
                    product_id,
                    locale,
                    Some(locale),
                    None,
                )
                .await
                .map_err(|error| {
                    pricing_error_to_port_error(
                        &context,
                        "resolve_product_price.product_projection",
                        error,
                    )
                })?;
            if !product
                .variants
                .iter()
                .any(|variant| variant.id == variant_id)
            {
                return Err(PortError::validation(
                    "pricing.variant_product_mismatch",
                    format!("variant {variant_id} does not belong to product {product_id}"),
                ));
            }
        }

        let resolved = self
            .resolve_variant_price(
                tenant_id,
                variant_id,
                crate::PriceResolutionContext {
                    currency_code: request.currency_code,
                    region_id: request.region_id,
                    price_list_id: request.price_list_id,
                    channel_id: request.channel_id,
                    channel_slug: request.channel_slug,
                    quantity: request.quantity,
                },
            )
            .await
            .map_err(|error| pricing_error_to_port_error(&context, "resolve_product_price", error))?
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
            variant_id,
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
            .map_err(|error| {
                pricing_error_to_port_error(&context, "read_price_list_projection", error)
            })?;
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

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<ActivePriceListProjectionSnapshot>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let lists = self
            .list_active_price_lists_for_channel(
                tenant_id,
                request.channel_id,
                request.channel_slug.as_deref(),
                Some(context.locale.as_str()),
                request.fallback_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                pricing_error_to_port_error(&context, "list_active_price_list_projections", error)
            })?;

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
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.get_admin_product_pricing_with_locale_fallback(
            tenant_id,
            request.product_id,
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
            request.selected_price_list_id,
        )
        .await
        .map_err(|error| {
            pricing_error_to_port_error(&context, "read_admin_product_pricing_projection", error)
        })
    }

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<crate::StorefrontPricingProductDetail>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.get_published_product_pricing_by_handle_with_locale_fallback(
            tenant_id,
            request.handle.trim(),
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
        )
        .await
        .map_err(|error| {
            pricing_error_to_port_error(
                &context,
                "read_storefront_product_pricing_projection",
                error,
            )
        })
    }

    async fn preview_variant_discount(
        &self,
        context: PortContext,
        request: PreviewVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
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
        preview.map_err(|error| {
            pricing_error_to_port_error(&context, "preview_variant_discount", error)
        })
    }
}

#[async_trait]
impl PricingWritePort for crate::PricingService {
    async fn upsert_variant_price(
        &self,
        context: PortContext,
        request: UpsertVariantPriceRequest,
    ) -> Result<crate::AdminPricingPrice, PortError> {
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        self.upsert_admin_variant_price_with_channel(
            tenant_id,
            actor_id,
            request.variant_id,
            request.price_list_id,
            request.currency_code.as_str(),
            request.amount,
            request.compare_at_amount,
            request.channel_id,
            request.channel_slug,
            request.min_quantity,
            request.max_quantity,
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, "upsert_variant_price", error))
    }

    async fn set_price_list_scope(
        &self,
        context: PortContext,
        request: SetPriceListScopeRequest,
    ) -> Result<crate::ActivePriceListOption, PortError> {
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        self.set_price_list_scope(
            tenant_id,
            actor_id,
            request.price_list_id,
            request.channel_id,
            request.channel_slug,
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, "set_price_list_scope", error))
    }

    async fn apply_variant_discount(
        &self,
        context: PortContext,
        request: ApplyVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError> {
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        let result = if let Some(price_list_id) = request.price_list_id {
            self.apply_price_list_percentage_discount_with_channel(
                tenant_id,
                actor_id,
                request.variant_id,
                price_list_id,
                request.currency_code.as_str(),
                request.discount_percent,
                request.channel_id,
                request.channel_slug,
            )
            .await
        } else {
            self.apply_percentage_discount_with_channel(
                tenant_id,
                actor_id,
                request.variant_id,
                request.currency_code.as_str(),
                request.discount_percent,
                request.channel_id,
                request.channel_slug,
            )
            .await
        };
        result
            .map_err(|error| pricing_error_to_port_error(&context, "apply_variant_discount", error))
    }

    async fn set_price_list_percentage_rule(
        &self,
        context: PortContext,
        request: SetPriceListPercentageRuleRequest,
    ) -> Result<crate::ActivePriceListOption, PortError> {
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        self.set_price_list_percentage_rule_projection(
            tenant_id,
            actor_id,
            request.price_list_id,
            request.adjustment_percent,
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| {
            pricing_error_to_port_error(&context, "set_price_list_percentage_rule", error)
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

fn parse_port_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "pricing.actor_id_invalid",
            "pricing write actor must be a UUID",
        )
    })
}

fn pricing_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "pricing.database_unavailable",
                "pricing owner storage operation failed"
            );
            PortError::unavailable(
                "pricing.database_unavailable",
                "pricing storage is temporarily unavailable",
            )
        }
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
            tracing::warn!(
                cause = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "pricing.validation",
                "pricing owner rejected a domain request"
            );
            PortError::validation("pricing.validation", "pricing request is invalid")
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
        CommerceError::Rich(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "pricing.rich_error",
                "pricing owner rich error"
            );
            PortError::invariant_violation(
                "pricing.rich_error",
                "pricing operation failed an internal invariant",
            )
        }
        CommerceError::Core(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "pricing.core_error",
                "pricing owner core error"
            );
            PortError::invariant_violation(
                "pricing.core_error",
                "pricing operation failed an internal invariant",
            )
        }
    }
}

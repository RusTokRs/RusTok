use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::error::CommerceError;
use rustok_outbox::TransactionalEventBus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

const PRICING_OWNER: &str = "rustok_pricing";
const PRICING_PORT_BOUNDARY: &str = "pricing_owner_port";

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
        let owner_operation = "resolve_product_price";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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
                let facts = PricingOwnerErrorFacts::uuids(
                    "variant_product_mismatch",
                    &[variant_id, product_id],
                );
                log_pricing_port_failure(
                    &context,
                    owner_operation,
                    "pricing.variant_product_mismatch",
                    &facts,
                    false,
                );
                return Err(PortError::validation(
                    "pricing.variant_product_mismatch",
                    "variant does not belong to the requested product",
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
            .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))?
            .ok_or_else(|| {
                let facts = PricingOwnerErrorFacts::uuids("price_not_found", &[variant_id]);
                log_pricing_port_failure(
                    &context,
                    owner_operation,
                    "pricing.price_not_found",
                    &facts,
                    false,
                );
                PortError::new(
                    PortErrorKind::NotFound,
                    "pricing.price_not_found",
                    "price was not found",
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
        let owner_operation = "read_price_list_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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
        let lists = self
            .list_active_price_lists_for_channel(
                tenant_id,
                request.channel_id,
                request.channel_slug.as_deref(),
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
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.get_admin_product_pricing_with_locale_fallback(
            tenant_id,
            request.product_id,
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
            request.selected_price_list_id,
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<crate::StorefrontPricingProductDetail>, PortError> {
        let owner_operation = "read_storefront_product_pricing_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.get_published_product_pricing_by_handle_with_locale_fallback(
            tenant_id,
            request.handle.trim(),
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }

    async fn preview_variant_discount(
        &self,
        context: PortContext,
        request: PreviewVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError> {
        let owner_operation = "preview_variant_discount";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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

#[async_trait]
impl PricingWritePort for crate::PricingService {
    async fn upsert_variant_price(
        &self,
        context: PortContext,
        request: UpsertVariantPriceRequest,
    ) -> Result<crate::AdminPricingPrice, PortError> {
        let owner_operation = "upsert_variant_price";
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let actor_id = parse_port_actor_id(&context, owner_operation)?;
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
        .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }

    async fn set_price_list_scope(
        &self,
        context: PortContext,
        request: SetPriceListScopeRequest,
    ) -> Result<crate::ActivePriceListOption, PortError> {
        let owner_operation = "set_price_list_scope";
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let actor_id = parse_port_actor_id(&context, owner_operation)?;
        self.set_price_list_scope(
            tenant_id,
            actor_id,
            request.price_list_id,
            request.channel_id,
            request.channel_slug,
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }

    async fn apply_variant_discount(
        &self,
        context: PortContext,
        request: ApplyVariantDiscountRequest,
    ) -> Result<crate::PriceAdjustmentPreview, PortError> {
        let owner_operation = "apply_variant_discount";
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let actor_id = parse_port_actor_id(&context, owner_operation)?;
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
        result.map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }

    async fn set_price_list_percentage_rule(
        &self,
        context: PortContext,
        request: SetPriceListPercentageRuleRequest,
    ) -> Result<crate::ActivePriceListOption, PortError> {
        let owner_operation = "set_price_list_percentage_rule";
        context.require_write_semantics()?;
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let actor_id = parse_port_actor_id(&context, owner_operation)?;
        self.set_price_list_percentage_rule_projection(
            tenant_id,
            actor_id,
            request.price_list_id,
            request.adjustment_percent,
            context.locale.as_str(),
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| pricing_error_to_port_error(&context, owner_operation, error))
    }
}

struct PricingPortContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct PricingOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    numeric_field_count: usize,
    numeric_nonzero_count: usize,
    numeric_negative_count: usize,
    opaque_payload_present: bool,
}

impl PricingOwnerErrorFacts {
    fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            numeric_field_count: 0,
            numeric_nonzero_count: 0,
            numeric_negative_count: 0,
            opaque_payload_present: false,
        }
    }

    fn text(error_variant: &'static str, values: &[&str]) -> Self {
        Self {
            text_field_count: values.len(),
            text_total_length: values.iter().map(|value| value.chars().count()).sum(),
            ..Self::empty(error_variant)
        }
    }

    fn uuids(error_variant: &'static str, values: &[Uuid]) -> Self {
        Self {
            uuid_field_count: values.len(),
            uuid_non_nil_count: values.iter().filter(|value| !value.is_nil()).count(),
            ..Self::empty(error_variant)
        }
    }

    fn numbers(error_variant: &'static str, values: &[i32]) -> Self {
        Self {
            numeric_field_count: values.len(),
            numeric_nonzero_count: values.iter().filter(|value| **value != 0).count(),
            numeric_negative_count: values.iter().filter(|value| **value < 0).count(),
            ..Self::empty(error_variant)
        }
    }

    fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

fn pricing_port_context_facts(context: &PortContext) -> PricingPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    PricingPortContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn pricing_owner_error_facts(error: &CommerceError) -> PricingOwnerErrorFacts {
    match error {
        CommerceError::Database(_) => PricingOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            PricingOwnerErrorFacts::uuids("product_not_found", &[*value])
        }
        CommerceError::VariantNotFound(value) => {
            PricingOwnerErrorFacts::uuids("variant_not_found", &[*value])
        }
        CommerceError::DuplicateHandle { handle, locale } => {
            PricingOwnerErrorFacts::text("duplicate_handle", &[handle.as_str(), locale.as_str()])
        }
        CommerceError::DuplicateSku(value) => {
            PricingOwnerErrorFacts::text("duplicate_sku", &[value.as_str()])
        }
        CommerceError::InvalidPrice(value) => {
            PricingOwnerErrorFacts::text("invalid_price", &[value.as_str()])
        }
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => PricingOwnerErrorFacts::numbers("insufficient_inventory", &[*requested, *available]),
        CommerceError::InvalidOptionCombination => {
            PricingOwnerErrorFacts::empty("invalid_option_combination")
        }
        CommerceError::Validation(value) => {
            PricingOwnerErrorFacts::text("validation", &[value.as_str()])
        }
        CommerceError::ShippingProfileNotFound(value) => {
            PricingOwnerErrorFacts::uuids("shipping_profile_not_found", &[*value])
        }
        CommerceError::DuplicateShippingProfileSlug(value) => {
            PricingOwnerErrorFacts::text("duplicate_shipping_profile_slug", &[value.as_str()])
        }
        CommerceError::NoVariants => PricingOwnerErrorFacts::empty("no_variants"),
        CommerceError::CannotDeletePublished => {
            PricingOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Rich(_) => PricingOwnerErrorFacts::opaque("rich"),
        CommerceError::Core(_) => PricingOwnerErrorFacts::opaque("core"),
    }
}

fn log_pricing_port_failure(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    error_facts: &PricingOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = pricing_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = PRICING_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = PRICING_PORT_BOUNDARY,
            "pricing owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = PRICING_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = PRICING_PORT_BOUNDARY,
            "pricing owner operation was rejected with bounded diagnostics"
        );
    }
}

fn log_pricing_context_rejection(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    parse_target: &'static str,
) {
    let context_facts = pricing_port_context_facts(context);
    tracing::warn!(
        owner = PRICING_OWNER,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation,
        code,
        parse_target,
        parse_failed = true,
        boundary = PRICING_PORT_BOUNDARY,
        "pricing port context was rejected with bounded diagnostics"
    );
}

fn parse_port_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_pricing_context_rejection(context, operation, "pricing.tenant_id_invalid", "tenant_id");
        PortError::validation(
            "pricing.tenant_id_invalid",
            "pricing request context is invalid",
        )
    })
}

fn parse_port_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        log_pricing_context_rejection(context, operation, "pricing.actor_id_invalid", "actor_id");
        PortError::validation("pricing.actor_id_invalid", "pricing write actor is invalid")
    })
}

fn pricing_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_facts = pricing_owner_error_facts(&error);
    match error {
        CommerceError::Database(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.database_unavailable",
                &error_facts,
                true,
            );
            PortError::unavailable(
                "pricing.database_unavailable",
                "pricing storage is temporarily unavailable",
            )
        }
        CommerceError::ProductNotFound(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.product_not_found",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::NotFound,
                "pricing.product_not_found",
                "product was not found",
                false,
            )
        }
        CommerceError::VariantNotFound(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.variant_not_found",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::NotFound,
                "pricing.variant_not_found",
                "variant was not found",
                false,
            )
        }
        CommerceError::DuplicateHandle { .. } => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.duplicate_handle",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "pricing.duplicate_handle",
                "pricing handle is already in use",
                false,
            )
        }
        CommerceError::DuplicateSku(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.duplicate_sku",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "pricing.duplicate_sku",
                "pricing SKU is already in use",
                false,
            )
        }
        CommerceError::InvalidPrice(detail) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.validation",
                &error_facts,
                false,
            );
            PortError::validation("pricing.validation", detail)
        }
        CommerceError::Validation(detail) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.validation",
                &error_facts,
                false,
            );
            PortError::validation("pricing.validation", detail)
        }
        CommerceError::InsufficientInventory { .. } => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.insufficient_inventory",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "pricing.insufficient_inventory",
                "inventory is insufficient for the pricing operation",
                false,
            )
        }
        CommerceError::InvalidOptionCombination => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.invalid_option_combination",
                &error_facts,
                false,
            );
            PortError::validation(
                "pricing.invalid_option_combination",
                "invalid option combination",
            )
        }
        CommerceError::ShippingProfileNotFound(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.shipping_profile_not_found",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::NotFound,
                "pricing.shipping_profile_not_found",
                "shipping profile was not found",
                false,
            )
        }
        CommerceError::DuplicateShippingProfileSlug(_) => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.duplicate_shipping_profile_slug",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "pricing.duplicate_shipping_profile_slug",
                "shipping profile slug is already in use",
                false,
            )
        }
        CommerceError::NoVariants => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.no_variants",
                &error_facts,
                false,
            );
            PortError::validation(
                "pricing.no_variants",
                "product must have at least one variant",
            )
        }
        CommerceError::CannotDeletePublished => {
            log_pricing_port_failure(
                context,
                operation,
                "pricing.cannot_delete_published",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "pricing.cannot_delete_published",
                "cannot delete published product",
                false,
            )
        }
        CommerceError::Rich(_) => {
            log_pricing_port_failure(context, operation, "pricing.rich_error", &error_facts, true);
            PortError::invariant_violation(
                "pricing.rich_error",
                "pricing operation failed an internal invariant",
            )
        }
        CommerceError::Core(_) => {
            log_pricing_port_failure(context, operation, "pricing.core_error", &error_facts, true);
            PortError::invariant_violation(
                "pricing.core_error",
                "pricing operation failed an internal invariant",
            )
        }
    }
}

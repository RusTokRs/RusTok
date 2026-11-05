use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};

use crate::dto::pricing::*;
use crate::ports::diagnostics::{
    log_pricing_port_entry, parse_port_actor_id, parse_port_tenant_id, pricing_error_to_port_error,
    validate_non_nil_uuid,
};

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
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(price_list_id) = request.price_list_id {
            validate_non_nil_uuid(price_list_id, "price_list_id")?;
        }
        if let Some(channel_id) = request.channel_id {
            validate_non_nil_uuid(channel_id, "channel_id")?;
        }
        self.require_embedded_variant_state()?;
        log_pricing_port_entry(&context, owner_operation);
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
        validate_non_nil_uuid(request.price_list_id, "price_list_id")?;
        if let Some(channel_id) = request.channel_id {
            validate_non_nil_uuid(channel_id, "channel_id")?;
        }
        log_pricing_port_entry(&context, owner_operation);
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
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(price_list_id) = request.price_list_id {
            validate_non_nil_uuid(price_list_id, "price_list_id")?;
        }
        if let Some(channel_id) = request.channel_id {
            validate_non_nil_uuid(channel_id, "channel_id")?;
        }
        self.require_embedded_variant_state()?;
        log_pricing_port_entry(&context, owner_operation);
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
        validate_non_nil_uuid(request.price_list_id, "price_list_id")?;
        log_pricing_port_entry(&context, owner_operation);
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

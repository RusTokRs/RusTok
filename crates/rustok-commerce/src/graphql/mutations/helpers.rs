use async_graphql::{Context, FieldError, Result};
use rust_decimal::Decimal;
use rustok_api::locale_tags_match;
use rustok_api::{AuthContext, PortActor, PortContext, RequestContext, graphql::GraphQLError};
use rustok_cart::{CartStorefrontPort, CartStorefrontRepriceRequest};
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_fulfillment::FulfillmentService;
use rustok_inventory::check_variant_availability_for_public_channel;
use rustok_order::OrderService;
use rustok_pricing::{
    PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest,
    in_process_pricing_read_port,
};
use rustok_product::entities::{
    product, product_translation, product_variant, variant_translation,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    CreateReturnDecisionInput, ReturnClaimDecisionInput, ReturnDecisionInput,
    ReturnExchangeDecisionInput, ReturnRefundDecisionInput, ShippingProfileService,
    storefront_channel::{is_metadata_visible_for_public_channel, normalize_public_channel_slug},
    storefront_shipping::{
        effective_shipping_profile_slug, enrich_cart_delivery_groups,
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
};

use super::super::types::*;

pub(crate) fn convert_create_product_input(
    input: CreateProductInput,
) -> Result<crate::dto::CreateProductInput> {
    let translations = input
        .translations
        .into_iter()
        .map(|translation| crate::dto::ProductTranslationInput {
            locale: translation.locale,
            title: translation.title,
            handle: translation.handle,
            description: translation.description,
            meta_title: translation.meta_title,
            meta_description: translation.meta_description,
        })
        .collect();

    let options = input
        .options
        .unwrap_or_default()
        .into_iter()
        .map(|option| crate::dto::ProductOptionInput {
            translations: option
                .translations
                .into_iter()
                .map(|translation| crate::dto::ProductOptionTranslationInput {
                    locale: translation.locale,
                    name: translation.name,
                    values: translation.values,
                })
                .collect(),
        })
        .collect();

    let variants = input
        .variants
        .into_iter()
        .map(|variant| {
            let prices = variant
                .prices
                .into_iter()
                .map(|price| {
                    let amount = parse_decimal(&price.amount)?;
                    let compare_at_amount = match price.compare_at_amount {
                        Some(value) => Some(parse_decimal(&value)?),
                        None => None,
                    };

                    Ok(crate::dto::PriceInput {
                        currency_code: price.currency_code,
                        channel_id: price.channel_id,
                        channel_slug: price.channel_slug,
                        amount,
                        compare_at_amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(crate::dto::CreateVariantInput {
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
                option1: variant.option1,
                option2: variant.option2,
                option3: variant.option3,
                prices,
                inventory_quantity: variant.inventory_quantity.unwrap_or(0),
                inventory_policy: variant
                    .inventory_policy
                    .unwrap_or_else(|| "deny".to_string()),
                weight: None,
                weight_unit: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(crate::dto::CreateProductInput {
        translations,
        options,
        variants,
        seller_id: input.seller_id,
        vendor: input.vendor,
        product_type: input.product_type,
        shipping_profile_slug: input.shipping_profile_slug,
        primary_category_id: input.primary_category_id,
        tags: input.tags.unwrap_or_default(),
        metadata: serde_json::Value::Object(Default::default()),
        publish: input.publish.unwrap_or(false),
    })
}

pub(crate) fn parse_decimal(value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|_| async_graphql::Error::new("Invalid decimal value"))
}

pub(crate) fn parse_optional_decimal(value: Option<&str>) -> Result<Option<Decimal>> {
    value.map(parse_decimal).transpose()
}

pub(crate) fn parse_pricing_currency_code(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(async_graphql::Error::new(
            "currency_code must be a 3-letter code",
        ));
    }
    Ok(normalized)
}

pub(crate) fn validate_admin_cart_promotion_target(
    scope: GqlAdminCartPromotionScope,
    line_item_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    match scope {
        GqlAdminCartPromotionScope::Cart | GqlAdminCartPromotionScope::Shipping => {
            if line_item_id.is_some() {
                return Err(async_graphql::Error::new(
                    "line_item_id is allowed only for line_item scope",
                ));
            }
            Ok(None)
        }
        GqlAdminCartPromotionScope::LineItem => line_item_id.map(Some).ok_or_else(|| {
            async_graphql::Error::new("line_item_id is required for line_item scope")
        }),
    }
}

pub(crate) fn parse_required_promotion_decimal(
    value: Option<&str>,
    field: &str,
) -> Result<Decimal> {
    let Some(value) = value else {
        return Err(async_graphql::Error::new(format!(
            "{field} is required for the selected promotion kind"
        )));
    };
    parse_decimal(value)
}

pub(crate) fn ensure_no_unused_promotion_amount(value: Option<&str>, field: &str) -> Result<()> {
    if value.is_some() {
        return Err(async_graphql::Error::new(format!(
            "{field} must be omitted for the selected promotion kind"
        )));
    }
    Ok(())
}

pub(crate) fn map_cart_promotion_preview(
    scope: GqlAdminCartPromotionScope,
    preview: rustok_cart::services::cart::CartPromotionPreview,
) -> GqlCartPromotionPreview {
    GqlCartPromotionPreview {
        kind: match preview.kind {
            rustok_cart::services::cart::CartPromotionKind::PercentageDiscount => {
                "percentage_discount".to_string()
            }
            rustok_cart::services::cart::CartPromotionKind::FixedDiscount => {
                "fixed_discount".to_string()
            }
        },
        scope: match scope {
            GqlAdminCartPromotionScope::Cart => "cart".to_string(),
            GqlAdminCartPromotionScope::LineItem => "line_item".to_string(),
            GqlAdminCartPromotionScope::Shipping => "shipping".to_string(),
        },
        line_item_id: preview.line_item_id,
        currency_code: preview.currency_code,
        base_amount: preview.base_amount.to_string(),
        adjustment_amount: preview.adjustment_amount.to_string(),
        adjusted_amount: preview.adjusted_amount.to_string(),
    }
}

pub(crate) fn normalize_pricing_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

pub(crate) fn build_create_order_change_input(
    input: CreateOrderChangeInputObject,
) -> Result<crate::dto::CreateOrderChangeInput> {
    Ok(crate::dto::CreateOrderChangeInput {
        change_type: input.change_type,
        description: input.description,
        preview: parse_json_payload(input.preview.as_str(), "Invalid JSON preview payload")?,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

pub(crate) fn build_create_order_return_input(
    input: CreateOrderReturnInputObject,
) -> Result<crate::dto::CreateOrderReturnInput> {
    Ok(crate::dto::CreateOrderReturnInput {
        reason: input.reason,
        note: input.note,
        items: input
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                Ok(crate::dto::CreateOrderReturnItemInput {
                    line_item_id: item.line_item_id,
                    quantity: item.quantity,
                    reason: item.reason,
                    note: item.note,
                    metadata: parse_optional_metadata(item.metadata.as_deref())?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

pub(crate) fn build_create_return_decision_input(
    input: CreateReturnDecisionInputObject,
) -> Result<CreateReturnDecisionInput> {
    Ok(CreateReturnDecisionInput {
        return_request: build_create_order_return_input(input.return_request)?,
        decision: ReturnDecisionInput {
            action: input.decision.action,
            refund: input
                .decision
                .refund
                .map(build_return_refund_decision_input)
                .transpose()?,
            exchange: input
                .decision
                .exchange
                .map(build_return_exchange_decision_input)
                .transpose()?,
            claim: input
                .decision
                .claim
                .map(build_return_claim_decision_input)
                .transpose()?,
            metadata: parse_optional_metadata(input.decision.metadata.as_deref())?,
        },
    })
}

pub(crate) fn build_return_refund_decision_input(
    input: ReturnRefundDecisionInputObject,
) -> Result<ReturnRefundDecisionInput> {
    Ok(ReturnRefundDecisionInput {
        payment_collection_id: input.payment_collection_id,
        amount: parse_optional_decimal(input.amount.as_deref())?,
        reason: input.reason,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

pub(crate) fn build_return_exchange_decision_input(
    input: ReturnExchangeDecisionInputObject,
) -> Result<ReturnExchangeDecisionInput> {
    let preview = input.preview.unwrap_or_else(|| "{}".to_string());
    Ok(ReturnExchangeDecisionInput {
        description: input.description,
        preview: parse_json_payload(preview.as_str(), "Invalid JSON preview payload")?,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

pub(crate) fn build_return_claim_decision_input(
    input: ReturnClaimDecisionInputObject,
) -> Result<ReturnClaimDecisionInput> {
    let preview = input.preview.unwrap_or_else(|| "{}".to_string());
    Ok(ReturnClaimDecisionInput {
        description: input.description,
        preview: parse_json_payload(preview.as_str(), "Invalid JSON preview payload")?,
        metadata: parse_optional_metadata(input.metadata.as_deref())?,
    })
}

pub(crate) fn graphql_decision_requires_payments_update(
    action: &str,
    has_refund_payload: bool,
) -> bool {
    if has_refund_payload {
        return true;
    }

    action.trim().to_ascii_lowercase().replace('-', "_") == "refund"
}

pub(crate) async fn ensure_storefront_order_access(
    db: &sea_orm::DatabaseConnection,
    event_bus: &rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    ctx: &Context<'_>,
    order_id: Uuid,
) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let customer = in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            storefront_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
        .map_err(|err| match err.code.as_str() {
            "customer.customer_by_user_not_found" => {
                <FieldError as GraphQLError>::unauthenticated()
            }
            _ => async_graphql::Error::new(err.message),
        })?;

    let order = OrderService::new(db.clone(), event_bus.clone())
        .get_order(tenant_id, order_id)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    if order.customer_id != Some(customer.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Order does not belong to the current customer",
        ));
    }

    Ok(())
}

pub(crate) fn parse_json_payload(value: &str, message: &str) -> Result<Value> {
    serde_json::from_str(value).map_err(|_| async_graphql::Error::new(message))
}

pub(crate) fn parse_optional_metadata(value: Option<&str>) -> Result<Value> {
    match value.map(str::trim) {
        None | Some("") => Ok(Value::Object(Default::default())),
        Some(value) => serde_json::from_str(value)
            .map_err(|_| async_graphql::Error::new("Invalid JSON metadata payload")),
    }
}

pub(crate) async fn resolve_optional_storefront_customer_id(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            storefront_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(async_graphql::Error::new(error.message)),
    }
}

fn storefront_customer_port_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("storefront-customer:{user_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

pub(crate) fn storefront_cart_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    cart_id: Uuid,
    operation: &str,
    is_write: bool,
) -> PortContext {
    let actor = auth
        .map(|value| PortActor::user(value.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("rustok-commerce.graphql"));
    let correlation_id = format!("storefront-cart:{operation}:{cart_id}");
    let context = PortContext::new(
        tenant_id.to_string(),
        actor,
        request_context.locale.as_str(),
        correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let context = request_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context);
    if is_write {
        context.with_idempotency_key(correlation_id)
    } else {
        context
    }
}

pub(crate) fn cart_port_error(error: rustok_api::PortError) -> async_graphql::Error {
    async_graphql::Error::new(error.message)
}

pub(crate) fn ensure_storefront_cart_access(
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

pub(crate) fn merge_graphql_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

pub(crate) fn cart_context_metadata(
    cart: &crate::dto::CartResponse,
    context: &crate::dto::StoreContextResponse,
) -> Value {
    serde_json::json!({
        "cart_context": {
            "region_id": context.region.as_ref().map(|region| region.id),
            "country_code": cart.country_code.clone(),
            "locale": context.locale.clone(),
            "currency_code": cart.currency_code.clone(),
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "shipping_selections": current_shipping_selections(cart),
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

pub(crate) async fn enrich_storefront_cart(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| normalize_public_channel_slug(request_context.channel_slug.as_deref()));
    enrich_cart_delivery_groups(
        db,
        tenant_id,
        cart,
        public_channel_slug.as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await
    .map_err(|err| async_graphql::Error::new(err.to_string()))
}

pub(crate) fn request_public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| {
            normalize_public_channel_slug(request_context.channel_slug.as_deref())
        })
}

pub(crate) fn storefront_public_channel_slug_for_cart(
    cart: &crate::dto::CartResponse,
    ctx: &Context<'_>,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| request_public_channel_slug(ctx))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    cart: &crate::dto::CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    let selections = if let Some(shipping_selections) = shipping_selections {
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
        if cart.delivery_groups.len() > 1 {
            return Err(async_graphql::Error::new(
                "selectedShippingOptionId can only be used for carts with a single delivery group",
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: None,
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        current_shipping_selections(cart)
    };

    for selection in selections {
        let Some(selected_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let option = FulfillmentService::new(db.clone())
            .get_shipping_option(
                tenant_id,
                selected_shipping_option_id,
                requested_locale,
                tenant_default_locale,
            )
            .await?;
        if !option.currency_code.eq_ignore_ascii_case(currency_code) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} uses currency {}, expected {}",
                option.id, option.currency_code, currency_code
            )));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} is not available for the current channel",
                option.id
            )));
        }
        let required_shipping_profiles =
            std::collections::BTreeSet::from([normalize_shipping_profile_slug(
                selection.shipping_profile_slug.as_str(),
            )
            .unwrap_or_else(|| "default".to_string())]);
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(async_graphql::Error::new(format!(
                "Shipping option {} is not compatible with shipping profile {}",
                option.id, selection.shipping_profile_slug
            )));
        }
    }

    Ok(())
}

pub(crate) fn current_shipping_selections(
    cart: &crate::dto::CartResponse,
) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: None,
            selected_shipping_option_id: group.selected_shipping_option_id,
        })
        .collect()
}

pub(crate) async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(())
}

pub(crate) async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(())
}

pub(crate) fn maybe_undefined_or_existing<T>(
    value: async_graphql::MaybeUndefined<T>,
    current: Option<T>,
) -> Option<T> {
    match value {
        async_graphql::MaybeUndefined::Value(value) => Some(value),
        async_graphql::MaybeUndefined::Null => None,
        async_graphql::MaybeUndefined::Undefined => current,
    }
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
    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Variant not found"))?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Product not found"))?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(async_graphql::Error::new("Product not found"));
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await?;

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
        .map_err(cart_port_error)?
        .into();
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(input.quantity, &resolved_price);
    validate_storefront_variant_inventory(
        db,
        tenant_id,
        &variant,
        input.quantity,
        public_channel_slug,
    )
    .await?;

    let base_title = pick_product_translation(&product_translation_models, locale, default_locale)
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match pick_variant_translation(&variant_translation_models, locale, default_locale)
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
            metadata: merge_graphql_metadata(
                parse_optional_metadata(input.metadata.as_deref())?,
                seller_snapshot_metadata(product_model.seller_id.as_deref()),
            ),
        },
        pricing_adjustment,
    })
}

pub(crate) struct ResolvedStorefrontLineItemInput {
    pub(crate) add_line_item: crate::dto::AddCartLineItemInput,
    pub(crate) pricing_adjustment: Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
}

pub(crate) fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn pick_variant_translation<'a>(
    translations: &'a [variant_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a variant_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn resolve_commerce_graphql_locale(
    ctx: &Context<'_>,
    requested: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    requested
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            ctx.data_opt::<RequestContext>()
                .map(|request| request.locale.clone())
        })
        .unwrap_or_else(|| tenant_default_locale.to_string())
}

pub(crate) fn build_storefront_pricing_context(
    cart: &crate::dto::CartResponse,
    request_context: &RequestContext,
    public_channel_slug: Option<&str>,
    quantity: i32,
) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: cart.currency_code.to_ascii_uppercase(),
        region_id: cart.region_id,
        price_list_id: None,
        channel_id: cart.channel_id.or(request_context.channel_id),
        channel_slug: public_channel_slug.map(|slug| slug.to_string()),
        quantity: Some(quantity),
    }
}

pub(crate) async fn reprice_storefront_cart_line_items(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    event_bus: &rustok_outbox::TransactionalEventBus,
    cart_storefront_port: &dyn CartStorefrontPort,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| normalize_public_channel_slug(request_context.channel_slug.as_deref()));
    let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context = build_storefront_pricing_context(
            &cart,
            request_context,
            public_channel_slug.as_deref(),
            line_item.quantity,
        );
        let resolved_price: rustok_pricing::ResolvedPrice = pricing_read_port
            .resolve_product_price(
                storefront_pricing_port_context(tenant_id, request_context, cart.id, line_item.id),
                ResolveProductPriceRequest {
                    product_id: line_item.product_id,
                    variant_id,
                    region_id: pricing_context.region_id,
                    channel_id: pricing_context.channel_id,
                    channel_slug: pricing_context.channel_slug,
                    price_list_id: pricing_context.price_list_id,
                    quantity: pricing_context.quantity,
                    currency_code: pricing_context.currency_code,
                },
            )
            .await
            .map_err(cart_port_error)?
            .into();
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        cart_storefront_port
            .reprice_storefront_line_items(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    None,
                    cart.id,
                    "reprice",
                    true,
                ),
                CartStorefrontRepriceRequest {
                    cart_id: cart.id,
                    updates,
                },
            )
            .await
            .map_err(cart_port_error)
    }
}

pub(crate) fn storefront_pricing_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart_id: Uuid,
    line_item_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-pricing"),
        request_context.locale.as_str(),
        format!("storefront-pricing:{cart_id}:{line_item_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    request_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

pub(crate) fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(quantity, resolved_price);

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

pub(crate) fn storefront_cart_pricing_snapshot(
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> (
    Decimal,
    Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
) {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    (base_unit_price, pricing_adjustment)
}

pub(crate) fn normalize_graphql_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub(crate) fn seller_snapshot_metadata(seller_id: Option<&str>) -> Value {
    let seller_id = normalize_graphql_seller_id(seller_id);

    serde_json::json!({
        "seller": {
            "id": seller_id,
        }
    })
}

pub(crate) async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    validate_storefront_variant_inventory(
        db,
        tenant_id,
        &product_variant::Entity::find_by_id(variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Variant not found"))?,
        requested_quantity,
        public_channel_slug,
    )
    .await
}

pub(crate) async fn validate_storefront_variant_inventory(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    let available = check_variant_availability_for_public_channel(
        db,
        tenant_id,
        variant,
        requested_quantity,
        public_channel_slug,
    )
    .await
    .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if !available {
        return Err(async_graphql::Error::new(format!(
            "Variant {} does not have enough available inventory for the current channel",
            variant.id
        )));
    }

    Ok(())
}

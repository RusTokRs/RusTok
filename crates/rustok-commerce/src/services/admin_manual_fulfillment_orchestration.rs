use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_fulfillment::{
    CreateAdminFulfillmentRequest, CreateFulfillmentInput, CreateFulfillmentItemInput,
    FulfillmentAdminCreateCommandPort, FulfillmentReadPort, FulfillmentResponse,
    ListFulfillmentProjectionsRequest, MANUAL_FULFILLMENT_PROVIDER_ID,
    ReadShippingOptionProjectionRequest, ShippingOptionReadPort, ShippingOptionResponse,
};
use rustok_order::{OrderLineItemResponse, OrderReadPort, ReadOrderProjectionRequest};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::storefront_shipping::{
    is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
};

pub(crate) struct AdminManualFulfillmentOrchestrationService {
    order_read_port: Arc<dyn OrderReadPort>,
    fulfillment_read_port: Arc<dyn FulfillmentReadPort>,
    shipping_option_read_port: Arc<dyn ShippingOptionReadPort>,
    create_command_port: Arc<dyn FulfillmentAdminCreateCommandPort>,
}

impl AdminManualFulfillmentOrchestrationService {
    pub(crate) fn new(
        order_read_port: Arc<dyn OrderReadPort>,
        fulfillment_read_port: Arc<dyn FulfillmentReadPort>,
        shipping_option_read_port: Arc<dyn ShippingOptionReadPort>,
        create_command_port: Arc<dyn FulfillmentAdminCreateCommandPort>,
    ) -> Self {
        Self {
            order_read_port,
            fulfillment_read_port,
            shipping_option_read_port,
            create_command_port,
        }
    }

    pub(crate) async fn create_manual_fulfillment(
        &self,
        read_context: PortContext,
        write_context: PortContext,
        input: CreateFulfillmentInput,
    ) -> Result<FulfillmentResponse, PortError> {
        input.validate().map_err(|_| invalid_request())?;
        let order_id = input.order_id;
        let order = self
            .order_read_port
            .read_order_projection(
                read_context.clone(),
                ReadOrderProjectionRequest {
                    order_id,
                    tenant_default_locale: None,
                },
            )
            .await?;

        let requested_items = input.items.clone().ok_or_else(invalid_request)?;
        if requested_items.is_empty() {
            return Err(invalid_request());
        }
        if input.customer_id.is_some() && input.customer_id != order.customer_id {
            return Err(invalid_request());
        }

        let order_line_items_by_id = order
            .line_items
            .iter()
            .cloned()
            .map(|item| (item.id, item))
            .collect::<BTreeMap<Uuid, OrderLineItemResponse>>();
        let existing_fulfillments = self
            .load_all_fulfillments_for_order(read_context.clone(), order_id)
            .await?;
        let mut fulfilled_quantities = BTreeMap::<Uuid, i32>::new();
        for fulfillment in existing_fulfillments {
            if fulfillment.status == "cancelled" {
                continue;
            }
            if fulfillment.items.is_empty() {
                return Err(invalid_request());
            }
            for item in fulfillment.items {
                let entry = fulfilled_quantities
                    .entry(item.order_line_item_id)
                    .or_insert(0);
                *entry = entry
                    .checked_add(item.quantity)
                    .ok_or_else(invalid_request)?;
            }
        }

        let requested_groups = requested_items
            .iter()
            .map(|item| {
                let line_item = order_line_items_by_id
                    .get(&item.order_line_item_id)
                    .ok_or_else(invalid_request)?;
                Ok(DeliveryGroupKey {
                    shipping_profile_slug: normalize_shipping_profile_slug(
                        line_item.shipping_profile_slug.as_str(),
                    )
                    .unwrap_or_else(|| "default".to_string()),
                    seller_id: normalize_seller_id(
                        line_item
                            .seller_id
                            .clone()
                            .or_else(|| seller_id_from_metadata(&line_item.metadata))
                            .as_deref(),
                    ),
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;
        let canonical_group = requested_groups
            .first()
            .cloned()
            .ok_or_else(invalid_request)?;
        if requested_groups.iter().any(|group| {
            group.shipping_profile_slug != canonical_group.shipping_profile_slug
                || group.seller_id != canonical_group.seller_id
        }) {
            return Err(invalid_request());
        }

        let shipping_option = match input.shipping_option_id {
            Some(shipping_option_id) => Some(
                self.shipping_option_read_port
                    .read_shipping_option_projection(
                        read_context.clone(),
                        ReadShippingOptionProjectionRequest {
                            shipping_option_id,
                            requested_locale: Some(read_context.locale.clone()),
                            tenant_default_locale: None,
                        },
                    )
                    .await?,
            ),
            None => None,
        };
        if let Some(option) = shipping_option.as_ref() {
            validate_shipping_option_against_order(
                option,
                order.currency_code.as_str(),
                canonical_group.shipping_profile_slug.as_str(),
            )?;
        }
        let provider_id = shipping_option
            .as_ref()
            .map(|option| option.provider_id.clone())
            .unwrap_or_else(|| MANUAL_FULFILLMENT_PROVIDER_ID.to_string());

        let mut items = Vec::with_capacity(requested_items.len());
        for item in requested_items {
            let line_item = order_line_items_by_id
                .get(&item.order_line_item_id)
                .ok_or_else(invalid_request)?;
            let already_fulfilled = fulfilled_quantities
                .get(&item.order_line_item_id)
                .copied()
                .unwrap_or_default();
            let remaining_quantity = line_item
                .quantity
                .checked_sub(already_fulfilled)
                .ok_or_else(invalid_request)?;
            if remaining_quantity <= 0 || item.quantity > remaining_quantity {
                return Err(invalid_request());
            }

            items.push(CreateFulfillmentItemInput {
                order_line_item_id: item.order_line_item_id,
                quantity: item.quantity,
                metadata: merge_metadata(
                    item.metadata,
                    serde_json::json!({
                        "shipping_profile_slug": canonical_group.shipping_profile_slug,
                        "seller_id": canonical_group.seller_id,
                        "post_order": {
                            "manual": true
                        }
                    }),
                ),
            });
        }

        let metadata = merge_metadata(
            input.metadata,
            serde_json::json!({
                "delivery_group": {
                    "shipping_profile_slug": canonical_group.shipping_profile_slug,
                    "seller_id": canonical_group.seller_id,
                    "order_line_item_ids": items
                        .iter()
                        .map(|item| item.order_line_item_id)
                        .collect::<Vec<_>>(),
                },
                "post_order": {
                    "manual": true
                }
            }),
        );

        self.create_command_port
            .create_fulfillment(
                write_context,
                CreateAdminFulfillmentRequest {
                    input: CreateFulfillmentInput {
                        order_id,
                        shipping_option_id: input.shipping_option_id,
                        customer_id: order.customer_id,
                        carrier: input.carrier,
                        tracking_number: input.tracking_number,
                        items: Some(items),
                        metadata,
                    },
                    provider_id,
                },
            )
            .await
    }

    async fn load_all_fulfillments_for_order(
        &self,
        context: PortContext,
        order_id: Uuid,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        let mut page = 1_u64;
        let mut all = Vec::new();
        loop {
            let result = self
                .fulfillment_read_port
                .list_fulfillment_projections(
                    context.clone(),
                    ListFulfillmentProjectionsRequest {
                        page,
                        per_page: 100,
                        status: None,
                        order_id: Some(order_id),
                        customer_id: None,
                    },
                )
                .await?;
            let returned = result.items.len();
            all.extend(result.items);
            if returned == 0 || all.len() as u64 >= result.total {
                return Ok(all);
            }
            page = page.checked_add(1).ok_or_else(invalid_request)?;
        }
    }
}

#[derive(Clone)]
struct DeliveryGroupKey {
    shipping_profile_slug: String,
    seller_id: Option<String>,
}

fn validate_shipping_option_against_order(
    option: &ShippingOptionResponse,
    order_currency_code: &str,
    required_shipping_profile_slug: &str,
) -> Result<(), PortError> {
    if !option
        .currency_code
        .eq_ignore_ascii_case(order_currency_code)
    {
        return Err(invalid_request());
    }
    let required_profiles = BTreeSet::from([required_shipping_profile_slug.to_string()]);
    if !is_shipping_option_compatible_with_profiles(option, &required_profiles) {
        return Err(invalid_request());
    }
    Ok(())
}

fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn seller_id_from_metadata(metadata: &Value) -> Option<String> {
    metadata
        .get("seller")
        .and_then(|seller| seller.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_seller_id(Some(value)))
        .or_else(|| {
            metadata
                .get("seller_id")
                .and_then(Value::as_str)
                .and_then(|value| normalize_seller_id(Some(value)))
        })
}

fn merge_metadata(current: Value, patch: Value) -> Value {
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

fn invalid_request() -> PortError {
    PortError::validation(
        "commerce.fulfillment_create_invalid",
        "manual fulfillment request is invalid",
    )
}

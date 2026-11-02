/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Authenticated native projection of the selected Product command port, never a local service.

use crate::model::{VariantCreateOutcome, VariantDraft};
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::native_server_adapter::{catalog_update_scope, parse_uuid, write_context};

#[cfg(feature = "ssr")]
fn owner_input(
    input: VariantDraft,
) -> Result<rustok_product::dto::CreateVariantInput, ServerFnError> {
    use rustok_product::dto::{CreateVariantInput, PriceInput, VariantAxisValueInput};
    let decimal = |text: &str| {
        text.parse::<rust_decimal::Decimal>()
            .map_err(|_| ServerFnError::new("Invalid Product price"))
    };
    Ok(CreateVariantInput {
        sku: input.sku,
        barcode: input.barcode,
        shipping_profile_slug: input.shipping_profile_slug,
        axis_values: input
            .axis_values
            .into_iter()
            .map(|value| {
                Ok(VariantAxisValueInput {
                    attribute_id: parse_uuid(&value.attribute_id, "attribute_id")?,
                    option_id: parse_uuid(&value.option_id, "option_id")?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?,
        prices: input
            .prices
            .into_iter()
            .map(|price| {
                Ok(PriceInput {
                    currency_code: price.currency_code,
                    channel_id: None,
                    channel_slug: None,
                    amount: decimal(&price.amount)?,
                    compare_at_amount: price
                        .compare_at_amount
                        .as_deref()
                        .map(decimal)
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?,
        inventory_quantity: input.inventory_quantity.unwrap_or_default(),
        inventory_policy: input.inventory_policy.unwrap_or_else(|| "deny".into()),
        weight: None,
        weight_unit: None,
    })
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-variant", input = leptos::server_fn::codec::Json)]
pub(super) async fn create_variant_native(
    tenant_id: String,
    actor_id: String,
    product_id: String,
    locale: String,
    idempotency_key: String,
    input: VariantDraft,
) -> Result<VariantCreateOutcome, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (runtime, auth, tenant, locale) =
            catalog_update_scope(&tenant_id, &actor_id, &locale).await?;
        if !crate::core::variant_create::valid_input(&input)
            || !crate::core::write_journal::valid_uuid(&idempotency_key)
        {
            // Transport admission is not a durable owner refusal.
            return Err(ServerFnError::new("Invalid Product SKU request"));
        }
        let product_id = parse_uuid(&product_id, "product_id")?;
        let context = write_context(
            &auth,
            &tenant,
            locale,
            // This is the deployed GraphQL *caller-key* framing, not the port's receipt name.
            "create_product_variant",
            Some(product_id),
            &idempotency_key,
        )?;
        let result = runtime
            .command_port()
            .create_variant(context, product_id, owner_input(input)?)
            .await;
        Ok(match result {
            Ok(value) if !value.id.is_nil() && value.product_id == product_id => {
                VariantCreateOutcome::Confirmed {
                    variant_id: value.id.to_string(),
                }
            }
            Ok(_) => VariantCreateOutcome::Unknown,
            Err(error) => {
                tracing::warn!(operation = "create_product_variant", kind = ?error.kind,
                    code_length = error.code.len(), retryable = error.retryable,
                    "Product native SKU creation did not return success");
                match crate::core::variant_create::normalized_refusal(&error.code)
                    .filter(|_| rustok_product::product_catalog_command_error_is_terminal(&error))
                {
                    Some(code) => VariantCreateOutcome::Rejected {
                        code: code.to_owned(),
                    },
                    None => VariantCreateOutcome::Unknown,
                }
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native Product SKU creation requires the SSR host",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    #[test]
    fn full_frozen_input_maps_to_the_same_owner_fields_as_graphql() {
        let input: VariantDraft = serde_json::from_value(serde_json::json!({
            "sku": "SKU", "barcode": "123", "shippingProfileSlug": "parcel",
            "axisValues": [{ "attributeId": "00000000-0000-4000-8000-000000000001", "optionId": "00000000-0000-4000-8000-000000000002" }],
            "prices": [{ "currencyCode": "USD", "amount": "12.000001", "compareAtAmount": "15" }],
            "inventoryQuantity": 17, "inventoryPolicy": "continue"
        })).unwrap();
        assert!(crate::core::variant_create::valid_input(&input));
        let domain = owner_input(input.clone()).unwrap();
        assert_eq!(domain.sku, input.sku);
        assert_eq!(domain.barcode, input.barcode);
        assert_eq!(domain.shipping_profile_slug, input.shipping_profile_slug);
        assert_eq!(domain.inventory_quantity, 17);
        assert_eq!(domain.inventory_policy, "continue");
        assert_eq!(
            domain.axis_values[0].attribute_id.to_string(),
            input.axis_values[0].attribute_id
        );
        assert_eq!(
            domain.axis_values[0].option_id.to_string(),
            input.axis_values[0].option_id
        );
        assert_eq!(domain.prices[0].amount.to_string(), "12.000001");
        assert_eq!(
            domain.prices[0].compare_at_amount.unwrap().to_string(),
            "15"
        );
        assert!(domain.prices[0].channel_id.is_none());
        assert!(domain.prices[0].channel_slug.is_none());
        assert!(domain.weight.is_none());
        assert!(domain.weight_unit.is_none());
    }
}

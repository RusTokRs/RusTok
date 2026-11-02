/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Native axes endpoints use only the host-selected Product command runtime. They never
//! construct CatalogService or switch to a local provider after an external failure.

use crate::model::{SetVariantAxesDraft, VariantAxesApplyOutcome, VariantAxesPreview};
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::native_server_adapter::{catalog_update_scope, parse_uuid, write_context};

#[cfg(feature = "ssr")]
fn input(
    draft: SetVariantAxesDraft,
) -> Result<rustok_product::dto::SetVariantAxesInput, ServerFnError> {
    use rustok_product::dto::{
        SetVariantAxesInput, VariantAxisInput, VariantAxisReassignmentInput, VariantAxisValueInput,
    };
    Ok(SetVariantAxesInput {
        axes: draft
            .axes
            .into_iter()
            .map(|axis| {
                Ok(VariantAxisInput {
                    attribute_id: parse_uuid(&axis.attribute_id, "attribute_id")?,
                    position: axis.position.unwrap_or_default(),
                    allowed_option_ids: axis
                        .allowed_option_ids
                        .iter()
                        .map(|id| parse_uuid(id, "option_id"))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?,
        expected_state: draft.expected_state,
        reassignments: draft
            .reassignments
            .into_iter()
            .map(|row| {
                Ok(VariantAxisReassignmentInput {
                    variant_id: parse_uuid(&row.variant_id, "variant_id")?,
                    axis_values: row
                        .axis_values
                        .into_iter()
                        .map(|value| {
                            Ok(VariantAxisValueInput {
                                attribute_id: parse_uuid(&value.attribute_id, "attribute_id")?,
                                option_id: parse_uuid(&value.option_id, "option_id")?,
                            })
                        })
                        .collect::<Result<Vec<_>, ServerFnError>>()?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?,
    })
}

#[cfg(feature = "ssr")]
fn axis(value: rustok_product::dto::VariantAxisConfigResponse) -> crate::model::VariantAxisConfig {
    crate::model::VariantAxisConfig {
        id: value.id.to_string(),
        attribute_id: value.attribute_id.to_string(),
        code: value.code,
        name: value.name,
        position: value.position,
        allowed_values: value
            .allowed_values
            .into_iter()
            .map(|value| crate::model::AxisAllowedValue {
                option_id: value.option_id.to_string(),
                value: value.value,
                position: value.position,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn preview(value: rustok_product::dto::VariantAxesPreviewResponse) -> VariantAxesPreview {
    let values = |values: Vec<rustok_product::dto::VariantAxisValueInput>| {
        values
            .into_iter()
            .map(|value| crate::model::VariantAxisValueDraft {
                attribute_id: value.attribute_id.to_string(),
                option_id: value.option_id.to_string(),
            })
            .collect()
    };
    VariantAxesPreview {
        product_id: value.product_id.to_string(),
        product_revision: value.product_revision,
        state_token: value.state_token,
        variant_count: value.variant_count,
        can_apply: value.can_apply,
        has_changes: value.has_changes,
        has_more_conflicts: value.has_more_conflicts,
        conflicts: value
            .conflicts
            .into_iter()
            .map(|conflict| crate::model::VariantAxesConflict {
                kind: conflict.kind.as_str().to_owned(),
                attribute_id: conflict.attribute_id.map(|id| id.to_string()),
                option_id: conflict.option_id.map(|id| id.to_string()),
                variant_ids: conflict
                    .variant_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                has_more_variants: conflict.has_more_variants,
            })
            .collect(),
        variants: value
            .variants
            .into_iter()
            .map(|variant| crate::model::VariantAxesPreviewVariant {
                variant_id: variant.variant_id.to_string(),
                sku: variant.sku,
                current_values: values(variant.current_values),
                proposed_values: values(variant.proposed_values),
                changes: variant.changes,
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/preview-variant-axes", input = leptos::server_fn::codec::Json)]
pub(super) async fn preview_variant_axes_native(
    tenant_id: String,
    actor_id: String,
    product_id: String,
    locale: String,
    draft: SetVariantAxesDraft,
) -> Result<VariantAxesPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (runtime, auth, tenant, locale) =
            catalog_update_scope(&tenant_id, &actor_id, &locale).await?;
        let product_id = parse_uuid(&product_id, "product_id")?;
        let mut context = rustok_api::PortContext::new(
            tenant.id.to_string(),
            rustok_api::PortActor::user(auth.user_id.to_string()),
            locale,
            format!("product-admin-preview-axes:{product_id}"),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        for permission in &auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = runtime
            .command_port()
            .preview_variant_axes(context, product_id, input(draft)?)
            .await;
        match result {
            Ok(value) => Ok(preview(value)),
            Err(error) => {
                tracing::warn!(operation = "preview_variant_axes", kind = ?error.kind, code_length = error.code.len(), "Product native axes preview refused");
                Err(ServerFnError::new(
                    "Product axes preview is unavailable or the proposal is invalid",
                ))
            }
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native Product axes execution requires the SSR host",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/set-variant-axes", input = leptos::server_fn::codec::Json)]
pub(super) async fn set_variant_axes_native(
    tenant_id: String,
    actor_id: String,
    product_id: String,
    locale: String,
    idempotency_key: String,
    draft: SetVariantAxesDraft,
) -> Result<VariantAxesApplyOutcome, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (runtime, auth, tenant, locale) =
            catalog_update_scope(&tenant_id, &actor_id, &locale).await?;
        let product_id = parse_uuid(&product_id, "product_id")?;
        let context = write_context(
            &auth,
            &tenant,
            locale,
            "set_product_variant_axes",
            Some(product_id),
            &idempotency_key,
        )?;
        let result = runtime
            .command_port()
            .set_variant_axes(context, product_id, input(draft)?)
            .await;
        Ok(match result {
            Ok(axes) => VariantAxesApplyOutcome::Confirmed {
                axes: axes.into_iter().map(axis).collect(),
            },
            Err(error) => {
                tracing::warn!(operation = "set_variant_axes", kind = ?error.kind, code_length = error.code.len(), retryable = error.retryable,
                    "Product native axes application did not return success");
                match crate::core::variant_axes::normalized_refusal(&error.code)
                    .filter(|_| rustok_product::product_catalog_command_error_is_terminal(&error))
                {
                    Some(code) => VariantAxesApplyOutcome::Rejected {
                        code: code.to_owned(),
                    },
                    None => VariantAxesApplyOutcome::Unknown,
                }
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native Product axes execution requires the SSR host",
        ))
    }
}

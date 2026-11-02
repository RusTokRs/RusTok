/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Existing-axis GraphQL adapter. Uses the common metadata-preserving HTTP executor, never a
//! Product-local HTTP client or a message/HTTP-status heuristic for terminal decisions.

use crate::core::variant_axes::normalized_refusal;
use crate::model::{SetVariantAxesDraft, VariantAxesApplyOutcome, VariantAxesPreview};
use rustok_graphql::{
    GraphqlHttpError, GraphqlOperationError, GraphqlRequest, execute_with_metadata, graphql_url,
};
use serde::{Deserialize, Serialize};

const PREVIEW: &str = "query ProductAdminPreviewVariantAxes($tenantId: UUID!, $productId: UUID!, $input: SetVariantAxesInput!) { previewProductVariantAxes(tenantId: $tenantId, productId: $productId, input: $input) { productId productRevision stateToken variantCount canApply hasChanges hasMoreConflicts conflicts { kind attributeId optionId variantIds hasMoreVariants } variants { variantId sku changes currentValues { attributeId optionId } proposedValues { attributeId optionId } } } }";
const APPLY: &str = "mutation ProductAdminSetVariantAxes($idempotencyKey: String!, $productId: UUID!, $input: SetVariantAxesInput!) { setProductVariantAxes(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) { id attributeId code name position allowedValues { optionId value position } } }";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewVariables {
    tenant_id: String,
    product_id: String,
    input: SetVariantAxesDraft,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResponse {
    preview_product_variant_axes: VariantAxesPreview,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyVariables {
    idempotency_key: String,
    product_id: String,
    input: SetVariantAxesDraft,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyResponse {
    set_product_variant_axes: Vec<crate::model::VariantAxisConfig>,
}

fn log_error(operation: &'static str, error: &GraphqlOperationError) {
    let kind = match &error.error {
        GraphqlHttpError::Network => "network",
        GraphqlHttpError::Http(_) => "http",
        GraphqlHttpError::Graphql(_) => "graphql",
        GraphqlHttpError::Unauthorized => "unauthorized",
    };
    tracing::warn!(
        owner = "rustok_product.admin",
        operation,
        error_kind = kind,
        metadata_present = error.extensions.is_some(),
        code_length = error
            .extensions
            .as_ref()
            .and_then(|metadata| metadata.code.as_ref())
            .map(String::len),
        "Product axes request did not return a success acknowledgement"
    );
}

pub(super) fn definitive_refusal(error: &GraphqlOperationError) -> Option<&'static str> {
    // A response with execution errors is not automatically a rejected mutation. Only the
    // selected owner's explicit durable outcome and a closed operation code can settle it.
    if !matches!(error.error, GraphqlHttpError::Graphql(_)) {
        return None;
    }
    let metadata = error.extensions.as_ref()?;
    if metadata.retryable != Some(false) || metadata.outcome.as_deref() != Some("rejected") {
        return None;
    }
    normalized_refusal(metadata.code.as_deref()?)
}

pub(super) async fn preview(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: String,
    locale: String,
    input: SetVariantAxesDraft,
) -> Result<VariantAxesPreview, GraphqlHttpError> {
    let result: Result<PreviewResponse, _> = execute_with_metadata(
        &graphql_url(),
        GraphqlRequest::new(
            PREVIEW,
            Some(PreviewVariables {
                tenant_id,
                product_id,
                input,
            }),
        ),
        token,
        tenant_slug,
        Some(locale),
    )
    .await;
    match result {
        Ok(response) => Ok(response.preview_product_variant_axes),
        Err(error) => {
            log_error("preview_variant_axes", &error);
            Err(GraphqlHttpError::Graphql(
                "Product axes preview is unavailable or the proposal is invalid".into(),
            ))
        }
    }
}

pub(super) async fn apply(
    token: Option<String>,
    tenant_slug: Option<String>,
    product_id: String,
    locale: String,
    idempotency_key: String,
    input: SetVariantAxesDraft,
) -> VariantAxesApplyOutcome {
    let result: Result<ApplyResponse, _> = execute_with_metadata(
        &graphql_url(),
        GraphqlRequest::new(
            APPLY,
            Some(ApplyVariables {
                idempotency_key,
                product_id,
                input,
            }),
        ),
        token,
        tenant_slug,
        Some(locale),
    )
    .await;
    match result {
        Ok(response) => VariantAxesApplyOutcome::Confirmed {
            axes: response.set_product_variant_axes,
        },
        Err(error) => {
            log_error("set_variant_axes", &error);
            match definitive_refusal(&error) {
                Some(code) => VariantAxesApplyOutcome::Rejected {
                    code: code.to_owned(),
                },
                None => VariantAxesApplyOutcome::Unknown,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_closed_nonretryable_owner_rejections_are_terminal() {
        let mut error = GraphqlOperationError {
            error: GraphqlHttpError::Graphql("private backend text".into()),
            extensions: Some(rustok_graphql::GraphqlErrorExtensions {
                code: Some("PRODUCT_VARIANT_AXES_STALE".into()),
                retryable: Some(false),
                outcome: Some("rejected".into()),
            }),
        };
        assert_eq!(definitive_refusal(&error), Some("stale"));
        error.extensions.as_mut().unwrap().outcome = None;
        assert_eq!(definitive_refusal(&error), None);
        error.extensions.as_mut().unwrap().outcome = Some("rejected".into());
        error.extensions.as_mut().unwrap().retryable = Some(true);
        assert_eq!(definitive_refusal(&error), None);
        error.extensions.as_mut().unwrap().retryable = Some(false);
        error.extensions.as_mut().unwrap().code = Some("PRODUCT_OPERATION_FAILED".into());
        assert_eq!(definitive_refusal(&error), None);
        error.extensions.as_mut().unwrap().code = Some("PRODUCT_VALIDATION".into());
        error.error = GraphqlHttpError::Unauthorized;
        assert_eq!(definitive_refusal(&error), None);
    }
}

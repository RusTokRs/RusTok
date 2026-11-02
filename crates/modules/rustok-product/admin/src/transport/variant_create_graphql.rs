/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Metadata-preserving projection of the existing Commerce GraphQL Product command.

use crate::core::variant_create::normalized_refusal;
use crate::core::write_journal::{ProductWriteScope, valid_uuid};
use crate::model::{VariantCreateOutcome, VariantDraft};
use rustok_graphql::{
    GraphqlHttpError, GraphqlOperationError, GraphqlRequest, execute_with_metadata, graphql_url,
};
use serde::{Deserialize, Serialize};

const CREATE: &str = "mutation ProductAdminCreateVariant($idempotencyKey: String!, $productId: UUID!, $input: CreateVariantInput!) { createProductVariant(idempotencyKey: $idempotencyKey, productId: $productId, input: $input) { id } }";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variables {
    idempotency_key: String,
    product_id: String,
    input: VariantDraft,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    create_product_variant: CreatedVariant,
}
#[derive(Deserialize)]
struct CreatedVariant {
    id: String,
}

pub(super) fn definitive_refusal(error: &GraphqlOperationError) -> Option<&'static str> {
    if !matches!(error.error, GraphqlHttpError::Graphql(_)) {
        return None;
    }
    let metadata = error.extensions.as_ref()?;
    if metadata.retryable != Some(false) || metadata.outcome.as_deref() != Some("rejected") {
        return None;
    }
    normalized_refusal(metadata.code.as_deref()?)
}

pub(super) async fn create(
    token: Option<String>,
    tenant_slug: Option<String>,
    scope: ProductWriteScope,
    idempotency_key: String,
    input: VariantDraft,
) -> VariantCreateOutcome {
    let result: Result<Response, _> = execute_with_metadata(
        &graphql_url(),
        GraphqlRequest::new(
            CREATE,
            Some(Variables {
                idempotency_key,
                product_id: scope.product_id,
                input,
            }),
        )
        .with_token(token)
        .with_tenant(tenant_slug)
        .with_locale(Some(scope.locale)),
    )
    .await;
    match result {
        Ok(response) if valid_uuid(&response.create_product_variant.id) => {
            VariantCreateOutcome::Confirmed {
                variant_id: response.create_product_variant.id,
            }
        }
        Ok(_) => VariantCreateOutcome::Unknown,
        Err(error) => {
            tracing::warn!(
                operation = "create_product_variant",
                metadata_present = error.extensions.is_some(),
                "Product GraphQL SKU creation did not return a success acknowledgement"
            );
            match definitive_refusal(&error) {
                Some(code) => VariantCreateOutcome::Rejected {
                    code: code.to_owned(),
                },
                None => VariantCreateOutcome::Unknown,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_nonretryable_closed_owner_refusals_can_settle_creation() {
        let mut error = GraphqlOperationError {
            error: GraphqlHttpError::Graphql("redacted".into()),
            extensions: Some(rustok_graphql::GraphqlErrorExtensions {
                code: Some("DUPLICATE_SKU".into()),
                retryable: Some(false),
                outcome: Some("rejected".into()),
            }),
        };
        assert_eq!(definitive_refusal(&error), Some("duplicate_sku"));
        for kind in [
            GraphqlHttpError::Network,
            GraphqlHttpError::Http("409".into()),
            GraphqlHttpError::Unauthorized,
        ] {
            error.error = kind;
            assert_eq!(definitive_refusal(&error), None);
        }
        error.error = GraphqlHttpError::Graphql("redacted".into());
        error.extensions.as_mut().unwrap().retryable = Some(true);
        assert_eq!(definitive_refusal(&error), None);
        error.extensions.as_mut().unwrap().retryable = Some(false);
        error.extensions.as_mut().unwrap().outcome = Some("unknown".into());
        assert_eq!(definitive_refusal(&error), None);
        error.extensions.as_mut().unwrap().outcome = Some("rejected".into());
        error.extensions.as_mut().unwrap().code = Some("PRODUCT_OPERATION_FAILED".into());
        assert_eq!(definitive_refusal(&error), None);
    }
}

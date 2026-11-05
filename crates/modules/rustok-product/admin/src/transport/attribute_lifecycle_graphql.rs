/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

use crate::model::{
    AttributeDefinitionAction, AttributeDefinitionOutcome, AttributeDefinitionResult,
    AttributeLifecycleRequest, AttributeLifecycleWorkspace, UpdateAttributeDefinitionDraft,
};
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute_with_metadata, graphql_url};
use serde::{Deserialize, Serialize};

const READ: &str = "query ProductAttributeLifecycle($tenantId: UUID!, $locale: String!, $attributeId: UUID, $optionId: UUID, $includeArchived: Boolean!) { productAttributeLifecycle(tenantId: $tenantId, locale: $locale, attributeId: $attributeId, optionId: $optionId, includeArchived: $includeArchived) }";
const WRITE: &str = "mutation ProductUpdateAttributeDefinition($idempotencyKey: String!, $input: UpdateProductAttributeDefinitionInput!) { updateProductAttributeDefinition(idempotencyKey: $idempotencyKey, input: $input) }";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadVariables {
    tenant_id: String,
    locale: String,
    #[serde(flatten)]
    request: AttributeLifecycleRequest,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadResponse {
    product_attribute_lifecycle: AttributeLifecycleWorkspace,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteVariables {
    idempotency_key: String,
    input: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteResponse {
    update_product_attribute_definition: AttributeDefinitionResult,
}

pub(super) async fn read(
    token: Option<String>,
    tenant: Option<String>,
    tenant_id: String,
    locale: String,
    request: AttributeLifecycleRequest,
) -> Result<AttributeLifecycleWorkspace, ()> {
    let response: ReadResponse = execute_with_metadata(
        &graphql_url(),
        GraphqlRequest::new(
            READ,
            Some(ReadVariables {
                tenant_id,
                locale: locale.clone(),
                request,
            }),
        ),
        token,
        tenant,
        Some(locale),
    )
    .await
    .map_err(|_| ())?;
    Ok(response.product_attribute_lifecycle)
}

pub(super) async fn update(
    token: Option<String>,
    tenant: Option<String>,
    key: String,
    input: UpdateAttributeDefinitionDraft,
) -> AttributeDefinitionOutcome {
    let mut wire = match serde_json::to_value(&input) {
        Ok(value) => value,
        Err(_) => return AttributeDefinitionOutcome::Unknown,
    };
    // Deterministic enum projection only. Text, IDs, state token and original locale are untouched.
    wire["action"] = serde_json::Value::String(
        match input.action {
            AttributeDefinitionAction::Update => "UPDATE",
            AttributeDefinitionAction::Archive => "ARCHIVE",
            AttributeDefinitionAction::Restore => "RESTORE",
        }
        .into(),
    );
    let response: Result<WriteResponse, _> = execute_with_metadata(
        &graphql_url(),
        GraphqlRequest::new(
            WRITE,
            Some(WriteVariables {
                idempotency_key: key,
                input: wire,
            }),
        ),
        token,
        tenant,
        Some(input.locale),
    )
    .await;
    match response {
        Ok(response) => AttributeDefinitionOutcome::Decision {
            result: response.update_product_attribute_definition,
        },
        Err(error) => {
            let rejected = matches!(error.error, GraphqlHttpError::Graphql(_))
                && error.extensions.as_ref().is_some_and(|meta| {
                    meta.code.as_deref() == Some("PRODUCT_VALIDATION")
                        && meta.retryable == Some(false)
                        && meta.outcome.as_deref() == Some("rejected")
                });
            if rejected {
                AttributeDefinitionOutcome::Rejected {
                    code: "validation".into(),
                }
            } else {
                AttributeDefinitionOutcome::Unknown
            }
        }
    }
}

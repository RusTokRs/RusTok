/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! One preselected write path. An unknown native result never triggers GraphQL/provider fallback.

use crate::core::variant_create::valid_input;
use crate::core::write_journal::{ProductWriteScope, valid_uuid};
use crate::model::{VariantCreateOutcome, VariantDraft};

pub(crate) async fn create_product_variant(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    scope: ProductWriteScope,
    idempotency_key: String,
    input: VariantDraft,
) -> VariantCreateOutcome {
    if !scope.valid() || !valid_uuid(&idempotency_key) || !valid_input(&input) {
        return VariantCreateOutcome::Unknown;
    }
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        match super::variant_create_native::create_variant_native(
            scope.tenant_id,
            scope.actor_id,
            scope.product_id,
            scope.locale,
            idempotency_key,
            input,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(
                    operation = "create_product_variant",
                    error_length = error.to_string().len(),
                    "Product native SKU creation has an unknown outcome"
                );
                VariantCreateOutcome::Unknown
            }
        }
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        if !super::authenticated_write_scope_matches(_token.clone(), _tenant_slug.clone(), &scope)
            .await
        {
            return VariantCreateOutcome::Unknown;
        }
        super::variant_create_graphql::create(_token, _tenant_slug, scope, idempotency_key, input)
            .await
    }
}

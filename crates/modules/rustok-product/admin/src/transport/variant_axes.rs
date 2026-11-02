/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Transport-blind axes facade. The host build profile selects native or GraphQL before the
//! request; an ambiguous native response never triggers a different transport/provider.

use crate::core::write_journal::ProductWriteScope;
use crate::model::{SetVariantAxesDraft, VariantAxesApplyOutcome, VariantAxesPreview};
use rustok_graphql::GraphqlHttpError;

pub(crate) async fn preview_variant_axes(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    scope: ProductWriteScope,
    draft: SetVariantAxesDraft,
) -> Result<VariantAxesPreview, GraphqlHttpError> {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        super::variant_axes_native::preview_variant_axes_native(
            scope.tenant_id,
            scope.actor_id,
            scope.product_id,
            scope.locale,
            draft,
        )
        .await
        .map_err(|error| {
            tracing::warn!(
                operation = "preview_variant_axes",
                error_length = error.to_string().len(),
                "Product native axes preview unavailable"
            );
            GraphqlHttpError::Graphql(
                "Product axes preview is unavailable or the proposal is invalid".into(),
            )
        })
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        if !super::authenticated_write_scope_matches(_token.clone(), _tenant_slug.clone(), &scope)
            .await
        {
            return Err(GraphqlHttpError::Unauthorized);
        }
        super::variant_axes_graphql::preview(
            _token,
            _tenant_slug,
            scope.tenant_id,
            scope.product_id,
            scope.locale,
            draft,
        )
        .await
    }
}

pub(crate) async fn set_variant_axes(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    scope: ProductWriteScope,
    idempotency_key: String,
    draft: SetVariantAxesDraft,
) -> VariantAxesApplyOutcome {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        match super::variant_axes_native::set_variant_axes_native(
            scope.tenant_id,
            scope.actor_id,
            scope.product_id,
            scope.locale,
            idempotency_key,
            draft,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(
                    operation = "set_variant_axes",
                    error_length = error.to_string().len(),
                    "Product native axes outcome is unknown"
                );
                VariantAxesApplyOutcome::Unknown
            }
        }
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        if !super::authenticated_write_scope_matches(_token.clone(), _tenant_slug.clone(), &scope)
            .await
        {
            return VariantAxesApplyOutcome::Unknown;
        }
        super::variant_axes_graphql::apply(
            _token,
            _tenant_slug,
            scope.product_id,
            scope.locale,
            idempotency_key,
            draft,
        )
        .await
    }
}

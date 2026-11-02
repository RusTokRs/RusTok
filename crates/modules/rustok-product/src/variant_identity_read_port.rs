/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Small Product-owned SKU identity projection for price/identity consumers.
//! This is not a public merchandising/visibility read or a SKU reservation.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CatalogService;
use crate::entities::{product, product_variant};

pub const MAX_VARIANT_IDENTITY_BATCH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentitiesRequest {
    /// When provided, every SKU must belong to this Product, not merely to the tenant.
    pub product_id: Option<Uuid>,
    pub variant_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentityListRequest {
    pub product_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantIdentitySnapshot {
    /// Explicit owner response scope; a consumer must correlate it with the trusted context.
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Uuid,
}

#[async_trait]
pub trait ProductVariantIdentityReadPort: Send + Sync {
    /// Returns every requested identity in request order, or refuses the whole read.
    /// A foreign/missing SKU is not an absent price and is never silently omitted.
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError>;
    /// Bounded identities for an already selected Product page, not a catalog discovery query.
    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError>;
}

#[async_trait]
impl ProductVariantIdentityReadPort for CatalogService {
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_request(&request)?;
        let tenant_id = context
            .tenant_id
            .parse::<Uuid>()
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(|| {
                PortError::validation(
                    "product.tenant_id_invalid",
                    "variant identity reads require a non-nil tenant UUID",
                )
            })?;
        let variants = product_variant::Entity::find()
            .select_only()
            .column(product_variant::Column::Id)
            .column(product_variant::Column::ProductId)
            .inner_join(product::Entity)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .filter(product::Column::TenantId.eq(tenant_id))
            .filter(product_variant::Column::Id.is_in(request.variant_ids.clone()))
            .limit((MAX_VARIANT_IDENTITY_BATCH + 1) as u64)
            .into_tuple::<(Uuid, Uuid)>()
            .all(self.database())
            .await
            .map_err(|_| {
                tracing::error!(
                    owner = "rustok_product",
                    owner_operation = "read_variant_identities",
                    requested_count = request.variant_ids.len(),
                    correlation_id_length = context.correlation_id.chars().count(),
                    database_failure = true,
                    "Product SKU identity projection storage failed"
                );
                PortError::unavailable(
                    "product.variant_identity_unavailable",
                    "SKU identity verification is temporarily unavailable",
                )
            })?;
        if variants
            .iter()
            .any(|(variant, product)| variant.is_nil() || product.is_nil())
        {
            return Err(PortError::invariant_violation(
                "product.variant_identity_invalid_data",
                "Stored SKU identity data is invalid",
            ));
        }
        let identities = variants.into_iter().collect::<BTreeMap<_, _>>();
        if identities.len() != request.variant_ids.len() {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                "product.variant_identity_missing",
                "one or more SKUs were not found in the current tenant",
                false,
            ));
        }
        if let Some(product_id) = request.product_id
            && identities.values().any(|owner| *owner != product_id)
        {
            return Err(PortError::validation(
                "product.variant_product_mismatch",
                "one or more SKUs do not belong to the requested product",
            ));
        }
        request
            .variant_ids
            .into_iter()
            .map(|variant_id| {
                let product_id = identities.get(&variant_id).copied().ok_or_else(|| {
                    PortError::invariant_violation(
                        "product.variant_identity_incomplete",
                        "SKU identity verification could not be completed safely",
                    )
                })?;
                Ok(ProductVariantIdentitySnapshot {
                    tenant_id,
                    product_id,
                    variant_id,
                })
            })
            .collect()
    }
    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = context
            .tenant_id
            .parse::<Uuid>()
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(|| {
                PortError::validation(
                    "product.tenant_id_invalid",
                    "a non-nil tenant UUID is required",
                )
            })?;
        let max_products = crate::MAX_PUBLISHED_PRODUCTS_PER_PAGE as usize;
        if request.product_ids.len() > max_products
            || request.product_ids.iter().any(Uuid::is_nil)
            || request
                .product_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != request.product_ids.len()
        {
            return Err(PortError::validation(
                "product.variant_identity_page_invalid",
                "request unique Product identities within the Product page limit",
            ));
        }
        if request.product_ids.is_empty() {
            return Ok(Vec::new());
        }
        let products = product::Entity::find()
            .select_only()
            .column(product::Column::Id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .filter(product::Column::Id.is_in(request.product_ids.clone()))
            .into_tuple::<Uuid>()
            .all(self.database())
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "product.variant_identity_unavailable",
                    "SKU identity verification is temporarily unavailable",
                )
            })?;
        if products.len() != request.product_ids.len() {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                "product.variant_identity_product_missing",
                "one or more Products were not found in the current tenant",
                false,
            ));
        }
        // The row probe follows this actual page, not the maximum 48-Product page size.
        let max_variants = request.product_ids.len() * MAX_VARIANT_IDENTITY_BATCH;
        let variants = product_variant::Entity::find()
            .select_only()
            .column(product_variant::Column::Id)
            .column(product_variant::Column::ProductId)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .filter(product_variant::Column::ProductId.is_in(request.product_ids))
            .limit((max_variants + 1) as u64)
            .into_tuple::<(Uuid, Uuid)>()
            .all(self.database())
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "product.variant_identity_unavailable",
                    "SKU identity verification is temporarily unavailable",
                )
            })?;
        let mut counts = BTreeMap::<Uuid, usize>::new();
        for (variant_id, product_id) in &variants {
            if variant_id.is_nil() || product_id.is_nil() {
                return Err(PortError::invariant_violation(
                    "product.variant_identity_invalid_data",
                    "Stored SKU identity data is invalid",
                ));
            }
            *counts.entry(*product_id).or_default() += 1;
        }
        if variants.len() > max_variants
            || counts
                .values()
                .any(|count| *count > MAX_VARIANT_IDENTITY_BATCH)
        {
            return Err(PortError::new(
                PortErrorKind::Conflict,
                "product.variant_identity_page_limit",
                "Product SKU counts exceed the identity page bound",
                false,
            ));
        }
        Ok(variants
            .into_iter()
            .map(|(variant_id, product_id)| ProductVariantIdentitySnapshot {
                tenant_id,
                product_id,
                variant_id,
            })
            .collect())
    }
}

fn validate_request(request: &ProductVariantIdentitiesRequest) -> Result<(), PortError> {
    if request.variant_ids.is_empty() || request.variant_ids.len() > MAX_VARIANT_IDENTITY_BATCH {
        return Err(PortError::validation(
            "product.variant_identity_batch_invalid",
            "request between 1 and 256 SKU identities",
        ));
    }
    if request.product_id.is_some_and(|id| id.is_nil())
        || request.variant_ids.iter().any(Uuid::is_nil)
    {
        return Err(PortError::validation(
            "product.variant_identity_invalid",
            "product and SKU identities must be non-nil UUIDs",
        ));
    }
    if request
        .variant_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != request.variant_ids.len()
    {
        return Err(PortError::validation(
            "product.variant_identity_duplicate",
            "request each SKU identity only once",
        ));
    }
    Ok(())
}

/// Validate the selected owner's complete exact-identity response before consuming data
/// keyed by these UUIDs (for example tenant-less price rows). This is a projection contract,
/// not a second catalog resolver, authorization grant or cross-owner snapshot/reservation.
pub fn validate_variant_identities_response(
    context: &PortContext,
    request: &ProductVariantIdentitiesRequest,
    response: &[ProductVariantIdentitySnapshot],
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    validate_request(request)?;
    let tenant_id = identity_tenant_id(context)?;
    if response.len() != request.variant_ids.len()
        || response
            .iter()
            .zip(&request.variant_ids)
            .any(|(identity, expected)| {
                identity.tenant_id != tenant_id
                    || identity.product_id.is_nil()
                    || identity.variant_id != *expected
                    || request
                        .product_id
                        .is_some_and(|id| identity.product_id != id)
            })
    {
        return Err(invalid_identity_projection());
    }
    Ok(())
}

/// List responses may be ordered by the owner, but cannot add foreign parents/SKUs,
/// duplicate an identity, exceed the actual page's bound or omit explicit tenant scope.
pub fn validate_product_variant_identity_list_response(
    context: &PortContext,
    request: &ProductVariantIdentityListRequest,
    response: &[ProductVariantIdentitySnapshot],
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    let tenant_id = identity_tenant_id(context)?;
    let products = request.product_ids.iter().copied().collect::<BTreeSet<_>>();
    if request.product_ids.len() > crate::MAX_PUBLISHED_PRODUCTS_PER_PAGE as usize
        || products.len() != request.product_ids.len()
        || products.iter().any(Uuid::is_nil)
    {
        return Err(PortError::validation(
            "product.variant_identity_page_invalid",
            "request unique Product identities within the Product page limit",
        ));
    }
    if response.len() > products.len() * MAX_VARIANT_IDENTITY_BATCH {
        return Err(invalid_identity_projection());
    }
    let mut seen = BTreeSet::new();
    let mut counts = BTreeMap::<Uuid, usize>::new();
    for identity in response {
        if identity.tenant_id != tenant_id
            || identity.variant_id.is_nil()
            || !products.contains(&identity.product_id)
            || !seen.insert(identity.variant_id)
        {
            return Err(invalid_identity_projection());
        }
        let count = counts.entry(identity.product_id).or_default();
        *count += 1;
        if *count > MAX_VARIANT_IDENTITY_BATCH {
            return Err(invalid_identity_projection());
        }
    }
    Ok(())
}

fn identity_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    context
        .tenant_id
        .trim()
        .parse::<Uuid>()
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "product.tenant_id_invalid",
                "variant identity reads require a non-nil tenant UUID",
            )
        })
}

fn invalid_identity_projection() -> PortError {
    PortError::invariant_violation(
        "product.variant_identity_projection_invalid",
        "The Product owner returned an inconsistent SKU identity projection",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_requests_are_bounded_unique_and_non_nil() {
        let id = Uuid::new_v4();
        let valid = ProductVariantIdentitiesRequest {
            product_id: None,
            variant_ids: vec![id],
        };
        assert!(validate_request(&valid).is_ok());
        for ids in [vec![], vec![Uuid::nil()], vec![id, id], vec![id; 257]] {
            assert!(
                validate_request(&ProductVariantIdentitiesRequest {
                    product_id: None,
                    variant_ids: ids,
                })
                .is_err()
            );
        }
        assert!(
            validate_request(&ProductVariantIdentitiesRequest {
                product_id: Some(Uuid::nil()),
                ..valid
            })
            .is_err()
        );
    }
    fn response_fixture() -> (
        PortContext,
        ProductVariantIdentitiesRequest,
        Vec<ProductVariantIdentitySnapshot>,
    ) {
        let tenant_id = Uuid::from_u128(1);
        let product_id = Uuid::from_u128(2);
        let ids = vec![Uuid::from_u128(3), Uuid::from_u128(4)];
        let context = PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::service("identity-response-test"),
            "en",
            "identity-response-test",
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let request = ProductVariantIdentitiesRequest {
            product_id: Some(product_id),
            variant_ids: ids.clone(),
        };
        let response = ids
            .into_iter()
            .map(|variant_id| ProductVariantIdentitySnapshot {
                tenant_id,
                product_id,
                variant_id,
            })
            .collect();
        (context, request, response)
    }

    #[test]
    fn exact_response_retains_tenant_parent_and_request_order() {
        let (context, request, response) = response_fixture();
        validate_variant_identities_response(&context, &request, &response).unwrap();
        let mut any_parent = request.clone();
        any_parent.product_id = None;
        let mut mixed = response.clone();
        mixed[1].product_id = Uuid::from_u128(9);
        validate_variant_identities_response(&context, &any_parent, &mixed).unwrap();
        assert!(validate_variant_identities_response(&context, &request, &mixed).is_err());
    }

    #[test]
    fn exact_response_rejects_omitted_extra_duplicate_and_reordered_skus() {
        let (context, request, response) = response_fixture();
        let mut extra = response.clone();
        extra.push(response[0].clone());
        let mut duplicate = response.clone();
        duplicate[1] = response[0].clone();
        let mut reordered = response.clone();
        reordered.reverse();
        for malformed in [
            Vec::new(),
            vec![response[0].clone()],
            extra,
            duplicate,
            reordered,
        ] {
            let error =
                validate_variant_identities_response(&context, &request, &malformed).unwrap_err();
            assert_eq!(error.kind, PortErrorKind::InvariantViolation);
            assert!(!error.retryable);
        }
    }

    #[test]
    fn exact_response_rejects_foreign_tenant_parent_nil_and_substituted_sku() {
        let (context, request, response) = response_fixture();
        for field in 0..5 {
            let mut malformed = response.clone();
            match field {
                0 => malformed[0].tenant_id = Uuid::from_u128(9),
                1 => malformed[0].product_id = Uuid::from_u128(9),
                2 => malformed[0].product_id = Uuid::nil(),
                3 => malformed[0].variant_id = Uuid::nil(),
                _ => malformed[0].variant_id = Uuid::from_u128(9),
            }
            assert!(validate_variant_identities_response(&context, &request, &malformed).is_err());
        }
    }

    #[test]
    fn response_cannot_erase_required_tenant_scope_in_serialization() {
        let (_, _, response) = response_fixture();
        let mut encoded = serde_json::to_value(&response[0]).unwrap();
        encoded.as_object_mut().unwrap().remove("tenant_id");
        assert!(serde_json::from_value::<ProductVariantIdentitySnapshot>(encoded).is_err());
    }

    #[test]
    fn page_response_checks_exact_scope_and_unique_sku_without_imposing_row_order() {
        let (context, request, mut response) = response_fixture();
        let page = ProductVariantIdentityListRequest {
            product_ids: vec![request.product_id.unwrap()],
        };
        response.reverse();
        validate_product_variant_identity_list_response(&context, &page, &response).unwrap();
        for field in 0..4 {
            let mut malformed = response.clone();
            match field {
                0 => malformed[0].tenant_id = Uuid::from_u128(9),
                1 => malformed[0].product_id = Uuid::from_u128(9),
                2 => malformed[0].variant_id = Uuid::nil(),
                _ => malformed[0] = malformed[1].clone(),
            }
            assert!(
                validate_product_variant_identity_list_response(&context, &page, &malformed)
                    .is_err()
            );
        }
    }

    #[test]
    fn page_response_never_expands_actual_page_or_per_product_sku_bound() {
        let (context, request, response) = response_fixture();
        let page = ProductVariantIdentityListRequest {
            product_ids: vec![request.product_id.unwrap(), Uuid::from_u128(9)],
        };
        let oversized = (0..=MAX_VARIANT_IDENTITY_BATCH)
            .map(|index| ProductVariantIdentitySnapshot {
                variant_id: Uuid::from_u128(100 + index as u128),
                ..response[0].clone()
            })
            .collect::<Vec<_>>();
        assert!(
            validate_product_variant_identity_list_response(&context, &page, &oversized).is_err()
        );
        let empty = ProductVariantIdentityListRequest {
            product_ids: Vec::new(),
        };
        validate_product_variant_identity_list_response(&context, &empty, &[]).unwrap();
        assert!(
            validate_product_variant_identity_list_response(&context, &empty, &response).is_err()
        );
    }

    #[test]
    fn page_response_admits_only_bounded_unique_non_nil_selected_products() {
        let (context, request, _) = response_fixture();
        let product = request.product_id.unwrap();
        for products in [
            vec![product, product],
            vec![Uuid::nil()],
            (1..=49).map(Uuid::from_u128).collect(),
        ] {
            let page = ProductVariantIdentityListRequest {
                product_ids: products,
            };
            assert_eq!(
                validate_product_variant_identity_list_response(&context, &page, &[])
                    .unwrap_err()
                    .kind,
                PortErrorKind::Validation
            );
        }
    }
}

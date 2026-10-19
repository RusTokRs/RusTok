use std::collections::HashSet;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ports::{product_owner_error_facts, product_port_context_facts};

use crate::{CatalogService, CommerceError, entities};

const HYDRATE_STOREFRONT_PRODUCT_TAGS_OPERATION: &str = "hydrate_storefront_product_tags";
const MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductStorefrontTagHydrationRequest {
    pub product_ids: Vec<Uuid>,
    pub fallback_locale: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductStorefrontTagHydration {
    pub items: Vec<ProductStorefrontTagHydrationItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductStorefrontTagHydrationItem {
    pub product_id: Uuid,
    pub tags: Vec<String>,
}

/// Product-owned post-page tag projection.
///
/// Consumers provide only Product identities that were already selected by an authoritative page. Product
/// remains responsible for tenant-scoped product-tag relation ordering and Taxonomy requested/fallback name
/// resolution. `product_tags` is the only Product tag attachment source.
#[async_trait]
pub trait ProductStorefrontTagReadPort: Send + Sync {
    async fn hydrate_storefront_product_tags(
        &self,
        context: PortContext,
        request: ProductStorefrontTagHydrationRequest,
    ) -> Result<ProductStorefrontTagHydration, PortError>;
}

#[async_trait]
impl ProductStorefrontTagReadPort for CatalogService {
    async fn hydrate_storefront_product_tags(
        &self,
        context: PortContext,
        request: ProductStorefrontTagHydrationRequest,
    ) -> Result<ProductStorefrontTagHydration, PortError> {
        let owner_operation = HYDRATE_STOREFRONT_PRODUCT_TAGS_OPERATION;
        require_tag_read_context(&context, owner_operation)?;
        validate_request(&context, owner_operation, &request)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        if request.product_ids.is_empty() {
            return Ok(ProductStorefrontTagHydration { items: Vec::new() });
        }

        let products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Id.is_in(request.product_ids.clone()))
            .all(self.database())
            .await
            .map_err(|error| {
                tag_error_to_port_error(&context, owner_operation, CommerceError::Database(error))
            })?;
        if products.len() != request.product_ids.len() {
            let context_facts = product_port_context_facts(&context);
            tracing::error!(
                requested = request.product_ids.len(),
                found = products.len(),
                correlation_id_length = context_facts.correlation_id_length,
                tenant_id_length = context_facts.tenant_id_length,
                operation = owner_operation,
                code = "product.storefront_tag_product_missing",
                "Storefront tag hydration Product page contains an owner-missing identity"
            );
            return Err(PortError::invariant_violation(
                "product.storefront_tag_product_missing",
                "product tag projection could not be completed safely",
            ));
        }

        let mut tags_by_product = self
            .load_product_tag_map(
                tenant_id,
                &products,
                context.locale.as_str(),
                Some(request.fallback_locale.as_str()),
            )
            .await
            .map_err(|error| tag_error_to_port_error(&context, owner_operation, error))?;
        let items = request
            .product_ids
            .into_iter()
            .map(|product_id| ProductStorefrontTagHydrationItem {
                product_id,
                tags: tags_by_product.remove(&product_id).unwrap_or_default(),
            })
            .collect();
        Ok(ProductStorefrontTagHydration { items })
    }
}

fn validate_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &ProductStorefrontTagHydrationRequest,
) -> Result<(), PortError> {
    if request.product_ids.len() > MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS {
        return Err(tag_validation_error(
            context,
            owner_operation,
            "product.storefront_tag_page_too_large",
            "product tag projection request is too large",
        ));
    }
    if request.fallback_locale.trim().is_empty() {
        return Err(tag_validation_error(
            context,
            owner_operation,
            "product.storefront_tag_fallback_locale_required",
            "product tag projection fallback locale is required",
        ));
    }

    let mut seen = HashSet::with_capacity(request.product_ids.len());
    for product_id in &request.product_ids {
        if product_id.is_nil() {
            return Err(tag_validation_error(
                context,
                owner_operation,
                "product.storefront_tag_product_id_invalid",
                "product tag projection Product identity is invalid",
            ));
        }
        if !seen.insert(*product_id) {
            return Err(tag_validation_error(
                context,
                owner_operation,
                "product.storefront_tag_product_id_duplicate",
                "product tag projection Product identities must be unique",
            ));
        }
    }
    Ok(())
}

fn require_tag_read_context(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| {
            let context_facts = product_port_context_facts(context);
            tracing::warn!(
                owner_code_length = error.code.chars().count(),
                owner_message_length = error.message.chars().count(),
                kind = ?error.kind,
                retryable = error.retryable,
                correlation_id_length = context_facts.correlation_id_length,
                tenant_id_length = context_facts.tenant_id_length,
                operation = owner_operation,
                code = "product.storefront_tag_context_invalid",
                "Product Storefront tag read context was rejected"
            );
            let PortError {
                kind,
                code,
                retryable,
                ..
            } = error;
            match kind {
                PortErrorKind::Timeout => {
                    PortError::timeout(code, "product tag request context is invalid")
                }
                PortErrorKind::Validation => {
                    PortError::validation(code, "product tag request context is invalid")
                }
                kind => PortError::new(
                    kind,
                    "product.storefront_tag_context_invalid",
                    "product tag request context is invalid",
                    retryable,
                ),
            }
        })
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let context_facts = product_port_context_facts(context);
        tracing::warn!(
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            operation = owner_operation,
            code = "product.tenant_id_invalid",
            "Product Storefront tag tenant context is invalid"
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product tag request context is invalid",
        )
    })
}

fn tag_validation_error(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    message: &'static str,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        code,
        "Product Storefront tag hydration request was rejected"
    );
    PortError::validation(code, message)
}

fn tag_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_facts = product_owner_error_facts(&error);
    let code = match &error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::Validation(_) => "product.validation",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::Core(error) => crate::error::core_error_code(error),
        CommerceError::Conflict(_) => "product.conflict",
        _ => "product.invariant_violation",
    };
    let context_facts = product_port_context_facts(context);
    tracing::error!(
        error_variant = error_facts.error_variant,
        text_field_count = error_facts.text_field_count,
        text_total_length = error_facts.text_total_length,
        uuid_field_count = error_facts.uuid_field_count,
        uuid_non_nil_count = error_facts.uuid_non_nil_count,
        opaque_payload_present = error_facts.opaque_payload_present,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        code,
        "Product Storefront tag owner read failed"
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "product storage is temporarily unavailable",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.validation", "product tag request is invalid")
        }
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        CommerceError::Core(error) => crate::error::core_port_error(&error),
        CommerceError::Conflict(_) => PortError::conflict(
            "product.conflict",
            "product request conflicts with the current state",
        ),
        _ => PortError::invariant_violation(
            "product.invariant_violation",
            "product tag projection could not be completed safely",
        ),
    }
}

#[cfg(test)]
mod diagnostic_policy_tests {
    use super::*;

    #[test]
    fn tag_context_rejection_keeps_deadline_and_tenant_policy_without_payload() {
        let context = PortContext::new(
            "private-invalid-tenant",
            rustok_api::PortActor::system(),
            "private-locale",
            "private-correlation",
        );
        let deadline = require_tag_read_context(&context, "scenario").unwrap_err();
        assert_eq!(deadline.kind, PortErrorKind::Timeout);
        assert!(deadline.retryable);
        assert!(!deadline.message.contains("private-"));
        let context = context.with_deadline(std::time::Duration::from_secs(1));
        assert!(require_tag_read_context(&context, "scenario").is_ok());
        let tenant = parse_tenant_id(&context, "scenario").unwrap_err();
        assert_eq!(tenant.kind, PortErrorKind::Validation);
        assert_eq!(tenant.code, "product.tenant_id_invalid");
        assert!(!tenant.retryable);
        assert!(!tenant.message.contains("private-"));
    }

    #[test]
    fn tag_owner_errors_keep_nested_classification_retryability_and_safe_copy() {
        use rustok_core::error::Error as CoreError;
        let context = PortContext::new(
            "private-tenant",
            rustok_api::PortActor::system(),
            "private-locale",
            "private-correlation",
        );
        for (error, kind, retryable) in [
            (
                CommerceError::Core(CoreError::External("private-payload".into())),
                PortErrorKind::Unavailable,
                true,
            ),
            (
                CommerceError::Core(CoreError::Forbidden("private-payload".into())),
                PortErrorKind::Forbidden,
                false,
            ),
            (
                CommerceError::Core(CoreError::Internal("private-payload".into())),
                PortErrorKind::InvariantViolation,
                false,
            ),
            (
                CommerceError::Conflict("private-payload".into()),
                PortErrorKind::Conflict,
                false,
            ),
        ] {
            let mapped = tag_error_to_port_error(&context, "scenario", error);
            assert_eq!(mapped.kind, kind);
            assert_eq!(mapped.retryable, retryable);
            assert!(!format!("{mapped}").contains("private-"));
        }
    }
}

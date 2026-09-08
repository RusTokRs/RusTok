use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationTargetCapability, TranslationTargetChange, TranslationTargetChangePage,
    TranslationTargetChangesRequest, TranslationTargetProgressFacts, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationTargetProviderDescriptor,
    provider_support::contract_validation_error, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CatalogService, CommerceError, ProductVariantTranslationExactLocaleError,
    services::catalog::ProductVariantTranslationChangeLifecycle, variant_translation_target,
};

const TRANSLATION_OWNER_SLUG: &str = "product";
const TRANSLATION_RESOURCE_KIND: &str = "variant";
const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

/// Adds aggregate progress and durable change polling to the exact-locale
/// Product Variant target while keeping inventory/read/validate/apply in the
/// already-proven inner adapter.
#[derive(Clone)]
pub struct ProductVariantTranslationTargetProvider {
    inner: variant_translation_target::ProductVariantTranslationTargetProvider,
    service: Arc<CatalogService>,
}

impl ProductVariantTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self {
            inner: variant_translation_target::ProductVariantTranslationTargetProvider::new(
                service.clone(),
            ),
            service,
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductVariantTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor
            .capabilities
            .insert(TranslationTargetCapability::AggregateProgress);
        descriptor
            .capabilities
            .insert(TranslationTargetCapability::ChangeCursor);
        descriptor
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        self.inner.list_resources(context, request).await
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        self.inner.read_resource(context, request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        self.inner.validate_patch(context, request).await
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        self.inner.apply_patch(context, request).await
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize_read(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = self
                .service
                .product_variant_translation_change_highwater(tenant_id)
                .await
                .map_err(product_error_to_port_error)?;
            let owner = self
                .service
                .read_product_variant_translation_exact_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(variant_translation_error_to_port_error)?;
            let after = self
                .service
                .product_variant_translation_change_highwater(tenant_id)
                .await
                .map_err(product_error_to_port_error)?;
            if before != after {
                continue;
            }

            // Variant title is nullable and therefore optional in the target contract.
            // With no required fields every inventory resource is complete by the
            // required-field definition; optional-unit progress separately reports
            // how many exact target titles are actually populated. The owner cursor
            // certifies that the aggregate and journal high-water describe one stable
            // Product-owned observation window.
            let facts = TranslationTargetProgressFacts {
                required_units: 0,
                exact_required_units: 0,
                optional_units: owner.resources,
                exact_optional_units: owner.exact_optional_units,
                resources: owner.resources,
                complete_resources: owner.resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "product.variant_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }

        Err(PortError::unavailable(
            "product.variant_translation_progress_unstable",
            "Product Variant translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize_read(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let parsed = request
            .after
            .as_ref()
            .map(parse_change_cursor)
            .transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .service
                    .product_variant_translation_change_highwater(tenant_id)
                    .await
                    .map_err(product_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .product_variant_translation_change_highwater(tenant_id)
                    .await
                    .map_err(product_error_to_port_error)?
                    .unwrap_or(0),
                0,
            ),
        };
        if through == 0 {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }

        let owner_changes = self
            .service
            .read_product_variant_translation_changes(tenant_id, after, through, request.limit)
            .await
            .map_err(product_error_to_port_error)?;
        let last_seq = owner_changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        let changes = owner_changes
            .into_iter()
            .map(|change| {
                Ok(TranslationTargetChange {
                    identity: variant_identity(change.variant_id),
                    resource_revision: opaque_revision(
                        change.resource_revision,
                        "resource_revision",
                    )?,
                    lifecycle: translation_change_lifecycle(change.lifecycle),
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;

        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "product.invalid_tenant_id",
            "Product Variant translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.variant_translation_permission_denied",
            "products:read permission is required",
        ));
    }
    Ok(())
}

fn variant_identity(variant_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Variant resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(variant_id.to_string())
            .expect("Variant UUID must satisfy the target resource id contract"),
        subresource_id: None,
    }
}

fn translation_change_lifecycle(
    lifecycle: ProductVariantTranslationChangeLifecycle,
) -> TranslationResourceLifecycle {
    match lifecycle {
        ProductVariantTranslationChangeLifecycle::Active => TranslationResourceLifecycle::Active,
        ProductVariantTranslationChangeLifecycle::Archived => {
            TranslationResourceLifecycle::Archived
        }
        ProductVariantTranslationChangeLifecycle::Deleted => TranslationResourceLifecycle::Deleted,
    }
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "product.variant_translation_change_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_change_cursor(cursor: &OpaqueCursor) -> Result<(u64, u64), PortError> {
    let mut parts = cursor.as_str().split(':');
    let version = parts.next();
    let through = parts.next().and_then(|value| value.parse::<u64>().ok());
    let after = parts.next().and_then(|value| value.parse::<u64>().ok());
    if version != Some(CHANGE_CURSOR_VERSION)
        || parts.next().is_some()
        || through.is_none()
        || after.is_none()
    {
        return Err(PortError::validation(
            "product.variant_translation_change_cursor_invalid",
            "Product Variant translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "product.variant_translation_change_cursor_invalid",
            "Product Variant translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "product.variant_translation_revision_invalid",
            format!("Product Variant {field} is invalid: {error}"),
        )
    })
}

fn variant_translation_error_to_port_error(
    error: ProductVariantTranslationExactLocaleError,
) -> PortError {
    match error {
        ProductVariantTranslationExactLocaleError::VariantNotFound(_) => PortError::not_found(
            "product.variant_translation_resource_not_found",
            "Product Variant translation resource was not found",
        ),
        ProductVariantTranslationExactLocaleError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "product.variant_translation_source_not_found",
                "Exact source Product Variant locale was not found",
            )
        }
        ProductVariantTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "product.variant_translation_owner_invariant",
                "Product Variant exact target locale is missing after owner apply",
            )
        }
        ProductVariantTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "product.variant_translation_revision_conflict",
            "Product Variant translation state conflicts with the requested mutation",
        ),
        ProductVariantTranslationExactLocaleError::Commerce(error) => {
            product_error_to_port_error(error)
        }
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.variant_translation_owner_unavailable",
            "Product Variant translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => PortError::not_found(
            "product.variant_translation_resource_not_found",
            "Product Variant translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. } | CommerceError::DuplicateSku(_) => {
            PortError::conflict(
                "product.variant_translation_owner_conflict",
                "Product state conflicts with the requested Variant translation mutation",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.variant_translation_owner_validation",
            "Product rejected the Variant translation request",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.variant_translation_owner_conflict",
            "Product state conflicts with the requested Variant translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.variant_translation_owner_invariant",
            "Product Variant translation state is invalid",
        ),
    }
}

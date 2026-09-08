use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, ReadTranslationResourceRequest, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourcePage,
    TranslationResourceSnapshot, TranslationTargetCapability, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider, TranslationTargetProviderDescriptor,
    provider_support::contract_validation_error, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CatalogService, CommerceError, ProductVariantTranslationExactLocaleError,
    variant_translation_target,
};

/// Adds aggregate progress to the exact-locale Product Variant target while
/// keeping the already-proven inventory/read/validate/apply adapter unchanged.
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
        let owner = self
            .service
            .read_product_variant_translation_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(variant_translation_error_to_port_error)?;

        // Variant title is nullable and therefore optional in the target contract.
        // With no required fields every inventory resource is complete by the
        // required-field definition; optional-unit progress separately reports
        // how many exact target titles are actually populated.
        let facts = TranslationTargetProgressFacts {
            required_units: 0,
            exact_required_units: 0,
            optional_units: owner.resources,
            exact_optional_units: owner.exact_optional_units,
            resources: owner.resources,
            complete_resources: owner.resources,
            owner_change_cursor: None,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation(
                "product.variant_translation_progress_invalid",
                error.to_string(),
            )
        })?;
        Ok(facts)
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
        ProductVariantTranslationExactLocaleError::Commerce(error) => product_error_to_port_error(error),
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

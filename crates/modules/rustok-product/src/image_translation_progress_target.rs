use std::sync::Arc;

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
    CatalogService, CommerceError, ProductImageTranslationExactLocaleError,
    image_translation_target,
};

/// Adds aggregate progress to the exact-locale Product Image target while
/// preserving the existing list/read/validate/apply adapter unchanged.
#[derive(Clone)]
pub struct ProductImageTranslationTargetProvider {
    inner: image_translation_target::ProductImageTranslationTargetProvider,
    service: Arc<CatalogService>,
}

impl ProductImageTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self {
            inner: image_translation_target::ProductImageTranslationTargetProvider::new(
                service.clone(),
            ),
            service,
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductImageTranslationTargetProvider {
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
            .read_product_image_translation_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(image_translation_error_to_port_error)?;

        // Image alt text is nullable and therefore optional in the target contract.
        // With no required fields every inventory resource is complete by required-
        // field semantics; optional-unit progress reports populated exact target alt.
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
                "product.image_translation_progress_invalid",
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
            "Product Image translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.image_translation_permission_denied",
            "products:read permission is required",
        ));
    }
    Ok(())
}

fn image_translation_error_to_port_error(
    error: ProductImageTranslationExactLocaleError,
) -> PortError {
    match error {
        ProductImageTranslationExactLocaleError::ImageNotFound(_) => PortError::not_found(
            "product.image_translation_resource_not_found",
            "Product Image translation resource was not found",
        ),
        ProductImageTranslationExactLocaleError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "product.image_translation_source_not_found",
                "Exact source Product Image locale was not found",
            )
        }
        ProductImageTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "product.image_translation_owner_invariant",
                "Product Image exact target locale is missing after owner apply",
            )
        }
        ProductImageTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "product.image_translation_revision_conflict",
            "Product Image translation state conflicts with the request",
        ),
        ProductImageTranslationExactLocaleError::Commerce(error) => {
            product_error_to_port_error(error)
        }
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.image_translation_owner_unavailable",
            "Product Image translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => PortError::not_found(
            "product.image_translation_resource_not_found",
            "Product Image translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. } | CommerceError::DuplicateSku(_) => {
            PortError::conflict(
                "product.image_translation_owner_conflict",
                "Product state conflicts with the Image translation request",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.image_translation_owner_validation",
            "Product rejected the Image translation request",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.image_translation_owner_conflict",
            "Product state conflicts with the Image translation request",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.image_translation_owner_invariant",
            "Product Image translation state is invalid",
        ),
    }
}

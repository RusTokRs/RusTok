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
    CatalogService, CommerceError, ProductOptionTranslationExactLocaleError,
    option_translation_target,
};

/// Adds exact-locale aggregate progress to the Product Option target while
/// preserving the existing list/read/validate/apply adapter unchanged.
#[derive(Clone)]
pub struct ProductOptionTranslationTargetProvider {
    inner: option_translation_target::ProductOptionTranslationTargetProvider,
    service: Arc<CatalogService>,
}

impl ProductOptionTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self {
            inner: option_translation_target::ProductOptionTranslationTargetProvider::new(
                service.clone(),
            ),
            service,
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductOptionTranslationTargetProvider {
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
            .read_product_option_translation_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(option_translation_error_to_port_error)?;

        let facts = TranslationTargetProgressFacts {
            required_units: owner.required_units,
            exact_required_units: owner.exact_required_units,
            optional_units: 0,
            exact_optional_units: 0,
            resources: owner.resources,
            complete_resources: owner.complete_resources,
            owner_change_cursor: None,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation(
                "product.option_translation_progress_invalid",
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
            "Product Option translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.option_translation_permission_denied",
            "products:read permission is required",
        ));
    }
    Ok(())
}

fn option_translation_error_to_port_error(
    error: ProductOptionTranslationExactLocaleError,
) -> PortError {
    match error {
        ProductOptionTranslationExactLocaleError::OptionNotFound(_) => PortError::not_found(
            "product.option_translation_resource_not_found",
            "Product Option translation resource was not found",
        ),
        ProductOptionTranslationExactLocaleError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "product.option_translation_source_not_found",
                "Exact source Product Option locale was not found",
            )
        }
        ProductOptionTranslationExactLocaleError::IncompleteLocale { .. }
        | ProductOptionTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "product.option_translation_owner_invariant",
                "Product Option exact locale aggregate is incomplete",
            )
        }
        ProductOptionTranslationExactLocaleError::ValueSetMismatch => PortError::conflict(
            "product.option_translation_value_set_conflict",
            "Product Option values changed while translation state was being read",
        ),
        ProductOptionTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "product.option_translation_revision_conflict",
            "Product Option translation state conflicts with the request",
        ),
        ProductOptionTranslationExactLocaleError::Commerce(error) => {
            product_error_to_port_error(error)
        }
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.option_translation_owner_unavailable",
            "Product Option translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => PortError::not_found(
            "product.option_translation_resource_not_found",
            "Product Option translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. } | CommerceError::DuplicateSku(_) => {
            PortError::conflict(
                "product.option_translation_owner_conflict",
                "Product state conflicts with the Option translation request",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.option_translation_owner_validation",
            "Product rejected the Option translation request",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.option_translation_owner_conflict",
            "Product state conflicts with the Option translation request",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.option_translation_owner_invariant",
            "Product Option translation state is invalid",
        ),
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, ReadTranslationResourceRequest,
    TranslationApplicationReceipt, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourcePage, TranslationResourceSnapshot, TranslationTargetCapability,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, provider_support::contract_validation_error,
    validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CommerceError, collection_translation_target,
    services::collection_translation::{
        CollectionTranslationExactLocaleError, CollectionTranslationService,
    },
};

#[derive(Clone)]
pub struct CommerceCollectionTranslationTargetProvider {
    inner: collection_translation_target::CommerceCollectionTranslationTargetProvider,
    service: Arc<CollectionTranslationService>,
}

impl CommerceCollectionTranslationTargetProvider {
    pub fn new(service: Arc<CollectionTranslationService>) -> Self {
        Self {
            inner: collection_translation_target::CommerceCollectionTranslationTargetProvider::new(
                service.clone(),
            ),
            service,
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for CommerceCollectionTranslationTargetProvider {
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
            .read_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(collection_error_to_port_error)?;
        let required_units = owner.resources.checked_mul(2).ok_or_else(|| {
            PortError::invariant_violation(
                "commerce.collection_translation_progress_overflow",
                "Commerce Collection required translation progress count overflow",
            )
        })?;
        let facts = TranslationTargetProgressFacts {
            required_units,
            exact_required_units: owner.exact_required_units,
            optional_units: owner.resources,
            exact_optional_units: owner.exact_optional_units,
            resources: owner.resources,
            complete_resources: owner.complete_resources,
            owner_change_cursor: None,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation(
                "commerce.collection_translation_progress_invalid",
                error.to_string(),
            )
        })?;
        Ok(facts)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "commerce.invalid_tenant_id",
            "Commerce Collection translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "commerce.collection_translation_permission_denied",
            "products:read permission is required",
        ));
    }
    Ok(())
}

fn collection_error_to_port_error(error: CollectionTranslationExactLocaleError) -> PortError {
    match error {
        CollectionTranslationExactLocaleError::CollectionNotFound(_) => PortError::not_found(
            "commerce.collection_translation_resource_not_found",
            "Commerce Collection translation resource was not found",
        ),
        CollectionTranslationExactLocaleError::SourceLocaleNotFound { .. } => PortError::not_found(
            "commerce.collection_translation_source_not_found",
            "Exact source Commerce Collection locale was not found",
        ),
        CollectionTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "commerce.collection_translation_owner_invariant",
                "Commerce Collection exact target locale is missing after owner apply",
            )
        }
        CollectionTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "commerce.collection_translation_revision_conflict",
            "Commerce Collection translation state conflicts with the request",
        ),
        CollectionTranslationExactLocaleError::OperationReceipt(error) => error,
        CollectionTranslationExactLocaleError::Commerce(error) => commerce_error_to_port_error(error),
    }
}

fn commerce_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "commerce.collection_translation_owner_unavailable",
            "Commerce Collection translation storage is temporarily unavailable",
        ),
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "commerce.collection_translation_handle_conflict",
            "Commerce Collection handle already exists for the target locale",
        ),
        CommerceError::Validation(_) => PortError::validation(
            "commerce.collection_translation_owner_validation",
            "Commerce rejected the Collection translation request",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "commerce.collection_translation_owner_invariant",
            "Commerce Collection translation owner state is invalid",
        ),
        _ => PortError::invariant_violation(
            "commerce.collection_translation_owner_invariant",
            "Commerce returned an unexpected Collection translation owner error",
        ),
    }
}

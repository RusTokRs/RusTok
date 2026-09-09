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
    FlexSchemaTranslationError, FlexSchemaTranslationOwnerPort,
    FlexSchemaTranslationProgressOwnerPort, schema_translation_target,
};

/// Adds dynamic exact-locale AggregateProgress to the base Flex schema-copy target while
/// leaving its list/read/validate/apply adapter unchanged.
#[derive(Clone)]
pub struct FlexSchemaTranslationProgressTargetProvider {
    inner: schema_translation_target::FlexSchemaTranslationTargetProvider,
    progress: Arc<dyn FlexSchemaTranslationProgressOwnerPort>,
}

impl FlexSchemaTranslationProgressTargetProvider {
    pub fn new(
        owner: Arc<dyn FlexSchemaTranslationOwnerPort>,
        progress: Arc<dyn FlexSchemaTranslationProgressOwnerPort>,
    ) -> Self {
        Self {
            inner: schema_translation_target::FlexSchemaTranslationTargetProvider::new(owner),
            progress,
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexSchemaTranslationProgressTargetProvider {
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
            .progress
            .read_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(owner_error_to_port_error)?;
        owner.validate().map_err(owner_error_to_port_error)?;

        let facts = TranslationTargetProgressFacts {
            required_units: owner.required_units,
            exact_required_units: owner.exact_required_units,
            optional_units: owner.optional_units,
            exact_optional_units: owner.exact_optional_units,
            resources: owner.resources,
            complete_resources: owner.complete_resources,
            // The owner read model itself is a consistent repeatable-read snapshot. A
            // durable resumable cursor is added only once the Flex change journal exists.
            owner_change_cursor: None,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation(
                "flex.schema_translation_progress_invalid",
                error.to_string(),
            )
        })?;
        Ok(facts)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|tenant_id| !tenant_id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "flex.invalid_tenant_id",
                "Flex schema translation progress context must carry a non-nil UUID tenant_id",
            )
        })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::FlexSchemas, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "flex.schema_translation_permission_denied",
            "flex_schemas:read permission is required",
        ));
    }
    Ok(())
}

fn owner_error_to_port_error(error: FlexSchemaTranslationError) -> PortError {
    match error {
        FlexSchemaTranslationError::Invalid(_) => PortError::validation(
            "flex.schema_translation_progress_validation",
            "Flex rejected the schema translation progress request",
        ),
        FlexSchemaTranslationError::SchemaNotFound(_)
        | FlexSchemaTranslationError::SourceLocaleNotFound { .. } => PortError::invariant_violation(
            "flex.schema_translation_progress_owner_invariant",
            "Flex schema translation progress inventory became inconsistent",
        ),
        FlexSchemaTranslationError::RevisionConflict { .. } => PortError::invariant_violation(
            "flex.schema_translation_progress_owner_invariant",
            "Flex schema translation progress unexpectedly encountered a revision conflict",
        ),
        FlexSchemaTranslationError::Operation(error) => error,
        FlexSchemaTranslationError::Database(_) => PortError::unavailable(
            "flex.schema_translation_progress_unavailable",
            "Flex schema translation progress storage is temporarily unavailable",
        ),
        FlexSchemaTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.schema_translation_progress_owner_invariant",
            "Flex schema translation progress owner state is invalid",
        ),
    }
}

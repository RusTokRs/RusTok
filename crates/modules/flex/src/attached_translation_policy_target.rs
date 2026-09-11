//! Translation-target policy decorator for attached Flex fields.
//!
//! Classification and AI-export admission are governance metadata, not translated content.
//! This decorator therefore enriches live field descriptors without changing donor content
//! revisions or ChangeCursor semantics. Any future export capability delegated through this
//! decorator receives the same live policy enrichment before resources leave the provider.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, ReadTranslationResourceRequest, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourcePage,
    TranslationResourceSnapshot, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetExportPage, TranslationTargetImportRequest, TranslationTargetImportValidation,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor,
};
use uuid::Uuid;

use crate::{
    FlexAttachedFieldPolicy, FlexAttachedFieldPolicyError, FlexAttachedFieldPolicyResolver,
    FlexAttachedTranslationError, FlexAttachedTranslationProgressTargetProvider,
    FlexAttachedTranslationResult, is_valid_flex_entity_type,
};

/// Adds live Flex-owned classification / AI-export policy to one attached Translation provider.
#[derive(Clone)]
pub struct FlexAttachedTranslationPolicyTargetProvider {
    inner: FlexAttachedTranslationProgressTargetProvider,
    policies: Arc<dyn FlexAttachedFieldPolicyResolver>,
    entity_type: String,
}

impl FlexAttachedTranslationPolicyTargetProvider {
    pub fn new(
        inner: FlexAttachedTranslationProgressTargetProvider,
        entity_type: impl Into<String>,
        policies: Arc<dyn FlexAttachedFieldPolicyResolver>,
    ) -> FlexAttachedTranslationResult<Self> {
        let entity_type = entity_type.into();
        if !is_valid_flex_entity_type(&entity_type) {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation policy provider has invalid entity type `{entity_type}`"
            )));
        }
        Ok(Self {
            inner,
            policies,
            entity_type,
        })
    }

    async fn enrich_snapshot(
        &self,
        tenant_id: Uuid,
        mut snapshot: TranslationResourceSnapshot,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let field_keys = snapshot
            .fields
            .iter()
            .map(|field| field.descriptor.key.as_str().to_string())
            .collect::<Vec<_>>();
        let policies = self
            .policies
            .resolve(tenant_id, &self.entity_type, &field_keys)
            .await
            .map_err(policy_error_to_port_error)?;
        for field in &mut snapshot.fields {
            let policy = policies
                .get(field.descriptor.key.as_str())
                .copied()
                .unwrap_or_default();
            apply_policy(field, policy);
        }
        snapshot.validate().map_err(|error| {
            PortError::invariant_violation(
                "flex.attached_translation_policy_snapshot_invalid",
                error.to_string(),
            )
        })?;
        Ok(snapshot)
    }

    async fn enrich_export_page(
        &self,
        tenant_id: Uuid,
        mut page: TranslationTargetExportPage,
    ) -> Result<TranslationTargetExportPage, PortError> {
        let field_keys = page
            .resources
            .iter()
            .flat_map(|resource| resource.fields.iter())
            .map(|field| field.descriptor.key.as_str().to_string())
            .collect::<Vec<_>>();
        let policies = self
            .policies
            .resolve(tenant_id, &self.entity_type, &field_keys)
            .await
            .map_err(policy_error_to_port_error)?;
        for resource in &mut page.resources {
            for field in &mut resource.fields {
                let policy = policies
                    .get(field.descriptor.key.as_str())
                    .copied()
                    .unwrap_or_default();
                apply_policy(field, policy);
            }
            resource.validate().map_err(|error| {
                PortError::invariant_violation(
                    "flex.attached_translation_policy_export_snapshot_invalid",
                    error.to_string(),
                )
            })?;
        }
        Ok(page)
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexAttachedTranslationPolicyTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        self.inner.descriptor()
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
        let tenant_id = parse_tenant_id(&context)?;
        let snapshot = self.inner.read_resource(context, request).await?;
        self.enrich_snapshot(tenant_id, snapshot).await
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
        self.inner.read_progress(context, request).await
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        self.inner.read_changes(context, request).await
    }

    async fn export_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationTargetExportPage, PortError> {
        let tenant_id = parse_tenant_id(&context)?;
        let page = self.inner.export_resources(context, request).await?;
        self.enrich_export_page(tenant_id, page).await
    }

    async fn validate_import(
        &self,
        context: PortContext,
        request: TranslationTargetImportRequest,
    ) -> Result<TranslationTargetImportValidation, PortError> {
        self.inner.validate_import(context, request).await
    }
}

fn apply_policy(
    field: &mut rustok_translation_targets::TranslationFieldSnapshot,
    policy: FlexAttachedFieldPolicy,
) {
    field.descriptor.classification = policy.classification.into();
    field.descriptor.ai_export_allowed = policy.ai_export_allowed;
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|tenant_id| !tenant_id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "flex.invalid_tenant_id",
                "Flex attached translation policy context must carry a non-nil UUID tenant_id",
            )
        })
}

fn policy_error_to_port_error(error: FlexAttachedFieldPolicyError) -> PortError {
    match error {
        FlexAttachedFieldPolicyError::Invalid(message) => {
            PortError::invariant_violation("flex.attached_field_policy_invalid", message)
        }
        FlexAttachedFieldPolicyError::Storage(_) => PortError::unavailable(
            "flex.attached_field_policy_unavailable",
            "Flex attached field policy storage is temporarily unavailable",
        ),
    }
}

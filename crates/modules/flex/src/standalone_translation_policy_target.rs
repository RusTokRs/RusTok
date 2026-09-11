//! Translation-target policy decorator for standalone Flex localized fields.
//!
//! Classification and AI-export admission are governance metadata, not translated content.
//! Policies are resolved by the resource schema id carried in `subresource_id`, so schemas can
//! govern the same field key independently without changing content revisions or ChangeCursor
//! semantics.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, ReadTranslationResourceRequest, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourcePage, TranslationResourceSnapshot, TranslationTargetChangePage,
    TranslationTargetChangesRequest, TranslationTargetExportPage, TranslationTargetImportRequest,
    TranslationTargetImportValidation, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider, TranslationTargetProviderDescriptor,
};
use uuid::Uuid;

use crate::{
    FlexStandaloneFieldPolicy, FlexStandaloneFieldPolicyError, FlexStandaloneFieldPolicyResolver,
    FlexStandaloneTranslationProgressTargetProvider,
};

/// Adds live Flex-owned classification / AI-export policy to the standalone Translation provider.
#[derive(Clone)]
pub struct FlexStandaloneTranslationPolicyTargetProvider {
    inner: FlexStandaloneTranslationProgressTargetProvider,
    policies: Arc<dyn FlexStandaloneFieldPolicyResolver>,
}

impl FlexStandaloneTranslationPolicyTargetProvider {
    pub fn new(
        inner: FlexStandaloneTranslationProgressTargetProvider,
        policies: Arc<dyn FlexStandaloneFieldPolicyResolver>,
    ) -> Self {
        Self { inner, policies }
    }

    async fn enrich_snapshot(
        &self,
        tenant_id: Uuid,
        mut snapshot: TranslationResourceSnapshot,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let schema_id = schema_id_from_identity(&snapshot.summary.identity)?;
        let field_keys = snapshot
            .fields
            .iter()
            .map(|field| field.descriptor.key.as_str().to_string())
            .collect::<Vec<_>>();
        let policies = self
            .policies
            .resolve(tenant_id, schema_id, &field_keys)
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
                "flex.standalone_translation_policy_snapshot_invalid",
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
        // A page can contain entries from multiple standalone schemas. Resolve each resource
        // against its own schema-scoped policy plane rather than merging keys across schemas.
        for resource in &mut page.resources {
            let schema_id = schema_id_from_identity(&resource.summary.identity)?;
            let field_keys = resource
                .fields
                .iter()
                .map(|field| field.descriptor.key.as_str().to_string())
                .collect::<Vec<_>>();
            let policies = self
                .policies
                .resolve(tenant_id, schema_id, &field_keys)
                .await
                .map_err(policy_error_to_port_error)?;
            for field in &mut resource.fields {
                let policy = policies
                    .get(field.descriptor.key.as_str())
                    .copied()
                    .unwrap_or_default();
                apply_policy(field, policy);
            }
            resource.validate().map_err(|error| {
                PortError::invariant_violation(
                    "flex.standalone_translation_policy_export_snapshot_invalid",
                    error.to_string(),
                )
            })?;
        }
        Ok(page)
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexStandaloneTranslationPolicyTargetProvider {
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
    policy: FlexStandaloneFieldPolicy,
) {
    field.descriptor.classification = policy.classification.into();
    field.descriptor.ai_export_allowed = policy.ai_export_allowed;
}

fn schema_id_from_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    let raw = identity
        .subresource_id
        .as_ref()
        .map(|value| value.as_str())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "flex.standalone_translation_policy_schema_missing",
                "Flex standalone Translation identity must carry schema_id in subresource_id",
            )
        })?;
    Uuid::parse_str(raw)
        .ok()
        .filter(|schema_id| !schema_id.is_nil())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "flex.standalone_translation_policy_schema_invalid",
                "Flex standalone Translation identity carries an invalid schema_id",
            )
        })
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|tenant_id| !tenant_id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "flex.invalid_tenant_id",
                "Flex standalone translation policy context must carry a non-nil UUID tenant_id",
            )
        })
}

fn policy_error_to_port_error(error: FlexStandaloneFieldPolicyError) -> PortError {
    match error {
        FlexStandaloneFieldPolicyError::Invalid(message) => {
            PortError::invariant_violation("flex.standalone_field_policy_invalid", message)
        }
        FlexStandaloneFieldPolicyError::Storage(_) => PortError::unavailable(
            "flex.standalone_field_policy_unavailable",
            "Flex standalone field policy storage is temporarily unavailable",
        ),
    }
}

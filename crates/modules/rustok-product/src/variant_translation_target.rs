use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values,
        normalize_optional_target_value, read_request_from_patch, validate_patch_against_snapshot,
        validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CatalogService, CommerceError, ProductVariantTranslationExactLocaleApply,
    ProductVariantTranslationExactLocaleApplyReceipt, ProductVariantTranslationExactLocaleError,
    ProductVariantTranslationExactLocaleRecord, ProductVariantTranslationExactLocaleSnapshot,
    entities::product::ProductStatus, services::with_product_operation_receipt,
};

const TRANSLATION_OWNER_SLUG: &str = "product";
const TRANSLATION_RESOURCE_KIND: &str = "variant";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_variant_patch";

#[derive(Clone)]
/// Product-owned neutral adapter for exact Variant title localization.
///
/// Translation sees only the dependency-neutral target contract. Exact source
/// inventory, lifecycle-aware revisions, CAS writes, Product -> Variant locking,
/// sibling-locale preservation, outbox publication, and durable owner receipts
/// remain inside `CatalogService`.
pub struct ProductVariantTranslationTargetProvider {
    service: Arc<CatalogService>,
}

impl ProductVariantTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Variant resource kind must satisfy the target contract"),
            display_name: "Product variants".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["products:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["products:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let variant_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_product_variant_translation_exact_locale(
                tenant_id,
                variant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(variant_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Product Variant translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductVariantTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .cursor
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "product.variant_translation_cursor_invalid",
                        "Product Variant translation cursor must be a Variant UUID",
                    )
                })
            })
            .transpose()?;
        let (owner_resources, next_after) = self
            .service
            .list_product_variant_translation_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(variant_translation_error_to_port_error)?;
        let resources = owner_resources
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|variant_id| {
                OpaqueCursor::new(variant_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.variant_translation_cursor_invalid",
                        error.to_string(),
                    )
                })
            })
            .transpose()?;
        Ok(TranslationResourcePage {
            resources,
            next_cursor,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.equal_source_target_locale",
                "source and target locale must differ",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        self.load_snapshot(tenant_id, &request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let snapshot = self
            .load_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        Ok(validate_patch_against_snapshot(&request, &snapshot))
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        let security = authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let variant_id = parse_identity(&request.identity)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let lease = match idempotency::admit(
            self.service.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            TRANSLATION_OWNER_SLUG,
            idempotency_key,
            OPERATION_APPLY_PATCH,
            &request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let owner_receipt = decode_owner_receipt(value)?;
                return application_receipt(&owner_receipt, &request);
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let snapshot = self
                .load_snapshot(tenant_id, &read_request_from_patch(&request))
                .await?;
            let validation = validate_patch_against_snapshot(&request, &snapshot);
            if !validation.accepted {
                return Err(validation_to_port_error(&validation));
            }
            let title = merged_target_title(&request, &snapshot);
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_product_variant_translation_exact_locale(
                    tenant_id,
                    security.user_id,
                    variant_id,
                    ProductVariantTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        title,
                        expected_resource_revision: request
                            .expected_resource_revision
                            .as_str()
                            .to_string(),
                        expected_source_revision: request
                            .expected_source_revision
                            .as_str()
                            .to_string(),
                        expected_target_revision: request
                            .expected_target_revision
                            .as_ref()
                            .map(|revision| revision.as_str().to_string()),
                    },
                ),
            )
            .await
            .map_err(variant_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.variant_translation_receipt_identity_invalid",
                    "Product Variant owner receipt is not bound to the active translation operation",
                ));
            }
            application_receipt(&applied, &request)
        }
        .await;

        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
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

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.variant_translation_permission_denied",
            format!("products:{action} permission is required"),
        ));
    }
    Ok(security)
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "product.variant_translation_identity_invalid",
            "Product Variant translation identity must address product/variant without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.variant_translation_resource_id_invalid",
            "Product Variant translation resource id must be a UUID",
        )
    })
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

fn summary_from_owner(
    snapshot: ProductVariantTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    let display_label = snapshot
        .source
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| format!("Variant {}", snapshot.variant_id));
    Ok(TranslationResourceSummary {
        identity: variant_identity(snapshot.variant_id),
        display_label,
        lifecycle: product_lifecycle(&snapshot.product_status),
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: ProductVariantTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: variant_identity(snapshot.variant_id),
        display_label: snapshot
            .source
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| format!("Variant {}", snapshot.variant_id)),
        lifecycle: product_lifecycle(&snapshot.product_status),
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    };
    let resource = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_revision(snapshot.source_revision, "source_revision")?,
        target_revision: snapshot
            .target_revision
            .map(|revision| opaque_revision(revision, "target_revision"))
            .transpose()?,
        fields: variant_fields(&snapshot.source, snapshot.target.as_ref()),
    };
    resource.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.variant_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(resource)
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "product.variant_translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn product_lifecycle(status: &ProductStatus) -> TranslationResourceLifecycle {
    match status {
        ProductStatus::Draft | ProductStatus::Active => TranslationResourceLifecycle::Active,
        ProductStatus::Archived => TranslationResourceLifecycle::Archived,
    }
}

fn variant_fields(
    source: &ProductVariantTranslationExactLocaleRecord,
    target: Option<&ProductVariantTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    let source_value = source.title.as_deref().unwrap_or_default();
    vec![TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new("title")
                .expect("static Product Variant field key must satisfy the target contract"),
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: false,
            ai_export_allowed: true,
            max_characters: Some(255),
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value: target.and_then(|value| value.title.clone()),
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    }]
}

fn merged_target_title(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Option<String> {
    let mut values = merged_patch_values(request, snapshot);
    values
        .remove("title")
        .flatten()
        .and_then(normalize_optional_target_value)
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<ProductVariantTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &ProductVariantTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "product.variant_translation_receipt_identity_missing",
            "Product Variant translation owner receipt is missing its operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-variant:{operation_id}"),
        resource_revision: opaque_revision(
            owner_receipt.resource_revision.clone(),
            "resource_revision",
        )?,
        target_revision: opaque_revision(owner_receipt.target_revision.clone(), "target_revision")?,
        applied_field_keys: request
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect(),
    })
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
            "Product rejected the Variant translation mutation",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(locale: &str, title: Option<&str>) -> ProductVariantTranslationExactLocaleRecord {
        ProductVariantTranslationExactLocaleRecord {
            locale: locale.to_string(),
            title: title.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn descriptor_claims_only_current_variant_capabilities() {
        let descriptor = ProductVariantTranslationTargetProvider::descriptor_value();
        assert_eq!(descriptor.owner_slug.as_str(), "product");
        assert_eq!(descriptor.resource_kind.as_str(), "variant");
        assert_eq!(
            descriptor.capabilities,
            BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ])
        );
    }

    #[test]
    fn variant_title_is_optional_public_and_bounded() {
        let fields = variant_fields(&record("en", Some("Small / Blue")), None);
        assert_eq!(fields.len(), 1);
        let field = &fields[0];
        assert_eq!(field.descriptor.key.as_str(), "title");
        assert!(!field.descriptor.required);
        assert!(field.descriptor.ai_export_allowed);
        assert_eq!(field.descriptor.max_characters, Some(255));
        assert_eq!(
            field.descriptor.classification,
            TranslationDataClassification::Public
        );
    }
}

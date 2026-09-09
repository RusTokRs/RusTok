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
    CatalogService, CommerceError, ProductImageTranslationExactLocaleApply,
    ProductImageTranslationExactLocaleApplyReceipt, ProductImageTranslationExactLocaleError,
    ProductImageTranslationExactLocaleRecord, ProductImageTranslationExactLocaleSnapshot,
    entities::product::ProductStatus, services::with_product_operation_receipt,
};

const TRANSLATION_OWNER_SLUG: &str = "product";
const TRANSLATION_RESOURCE_KIND: &str = "image";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_image_patch";
const IMAGE_ALT_TEXT_FIELD: &str = "alt_text";

#[derive(Clone)]
/// Product-owned neutral adapter for exact Product Image alt-text localization.
///
/// Translation sees only the dependency-neutral target contract. Exact source
/// inventory, lifecycle-aware revisions, CAS writes, Product -> Image locking,
/// sibling-locale preservation, explicit NULL rows, outbox publication, and
/// durable owner receipts remain inside `CatalogService`.
pub struct ProductImageTranslationTargetProvider {
    service: Arc<CatalogService>,
}

impl ProductImageTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Image resource kind must satisfy the target contract"),
            display_name: "Product images".to_string(),
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
        let image_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_product_image_translation_exact_locale(
                tenant_id,
                image_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(image_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Product Image translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductImageTranslationTargetProvider {
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
                        "product.image_translation_cursor_invalid",
                        "Product Image translation cursor must be an Image UUID",
                    )
                })
            })
            .transpose()?;
        let (owner_resources, next_after) = self
            .service
            .list_product_image_translation_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(image_translation_error_to_port_error)?;
        let resources = owner_resources
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|image_id| {
                OpaqueCursor::new(image_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.image_translation_cursor_invalid",
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
        let image_id = parse_identity(&request.identity)?;
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
            let alt_text = merged_target_alt_text(&request, &snapshot);
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_product_image_translation_exact_locale(
                    tenant_id,
                    security.user_id,
                    image_id,
                    ProductImageTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        alt_text,
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
            .map_err(image_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.image_translation_receipt_identity_invalid",
                    "Product Image owner receipt is not bound to the active translation operation",
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
            "Product Image translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.image_translation_permission_denied",
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
            "product.image_translation_identity_invalid",
            "Product Image translation identity must address product/image without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.image_translation_resource_id_invalid",
            "Product Image translation resource id must be a UUID",
        )
    })
}

fn image_identity(image_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Image resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(image_id.to_string())
            .expect("Image UUID must satisfy the target resource id contract"),
        subresource_id: None,
    }
}

fn display_label(snapshot: &ProductImageTranslationExactLocaleSnapshot) -> String {
    snapshot
        .source
        .alt_text
        .clone()
        .filter(|alt_text| !alt_text.trim().is_empty())
        .unwrap_or_else(|| format!("Image {}", snapshot.image_id))
}

fn summary_from_owner(
    snapshot: ProductImageTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: image_identity(snapshot.image_id),
        display_label: display_label(&snapshot),
        lifecycle: product_lifecycle(&snapshot.product_status),
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: ProductImageTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: image_identity(snapshot.image_id),
        display_label: display_label(&snapshot),
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
        fields: image_fields(&snapshot.source, snapshot.target.as_ref()),
    };
    resource.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.image_translation_snapshot_invalid",
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
                    "product.image_translation_locale_invalid",
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

fn image_fields(
    source: &ProductImageTranslationExactLocaleRecord,
    target: Option<&ProductImageTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    let source_value = source.alt_text.as_deref().unwrap_or_default();
    vec![TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new(IMAGE_ALT_TEXT_FIELD)
                .expect("static Product Image field key must satisfy the target contract"),
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: false,
            ai_export_allowed: true,
            max_characters: Some(255),
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value: target.and_then(|value| value.alt_text.clone()),
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    }]
}

fn merged_target_alt_text(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Option<String> {
    let mut values = merged_patch_values(request, snapshot);
    values
        .remove(IMAGE_ALT_TEXT_FIELD)
        .flatten()
        .and_then(normalize_optional_target_value)
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<ProductImageTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &ProductImageTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "product.image_translation_receipt_identity_missing",
            "Product Image translation owner receipt is missing its operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-image:{operation_id}"),
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
            "product.image_translation_revision_invalid",
            format!("Product Image {field} is invalid: {error}"),
        )
    })
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
            "Product Image translation state conflicts with the requested mutation",
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
                "Product state conflicts with the requested Image translation mutation",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.image_translation_owner_validation",
            "Product rejected the Image translation mutation",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.image_translation_owner_conflict",
            "Product state conflicts with the requested Image translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.image_translation_owner_invariant",
            "Product Image translation state is invalid",
        ),
    }
}

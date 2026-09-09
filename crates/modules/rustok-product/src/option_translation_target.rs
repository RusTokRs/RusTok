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
        contract_validation_error, field_hash, merged_patch_values, read_request_from_patch,
        required_target_value, validate_patch_against_snapshot, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CatalogService, CommerceError, ProductOptionTranslationExactLocaleApply,
    ProductOptionTranslationExactLocaleApplyReceipt, ProductOptionTranslationExactLocaleError,
    ProductOptionTranslationExactLocaleRecord, ProductOptionTranslationExactLocaleSnapshot,
    ProductOptionTranslationExactLocaleValueApply, entities::product::ProductStatus,
    services::with_product_operation_receipt,
};

const TRANSLATION_OWNER_SLUG: &str = "product";
const TRANSLATION_RESOURCE_KIND: &str = "option";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_option_patch";
const OPTION_TITLE_FIELD: &str = "title";
const OPTION_VALUE_FIELD_PREFIX: &str = "value:";

#[derive(Clone)]
/// Product-owned neutral adapter for exact Product Option localization.
///
/// One Translation resource is one Product Option aggregate: its localized title
/// plus every ordered Product-owned value. Translation sees stable field keys,
/// while exact inventory, aggregate revisions, CAS, locking, persistence and
/// durable owner receipts remain inside `CatalogService`.
pub struct ProductOptionTranslationTargetProvider {
    service: Arc<CatalogService>,
}

impl ProductOptionTranslationTargetProvider {
    pub fn new(service: Arc<CatalogService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Option resource kind must satisfy the target contract"),
            display_name: "Product options".to_string(),
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
        let option_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_product_option_translation_exact_locale(
                tenant_id,
                option_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(option_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Product Option translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductOptionTranslationTargetProvider {
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
                        "product.option_translation_cursor_invalid",
                        "Product Option translation cursor must be an Option UUID",
                    )
                })
            })
            .transpose()?;
        let (owner_resources, next_after) = self
            .service
            .list_product_option_translation_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(option_translation_error_to_port_error)?;
        let resources = owner_resources
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|option_id| {
                OpaqueCursor::new(option_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.option_translation_cursor_invalid",
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
        let option_id = parse_identity(&request.identity)?;
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
            let (title, values) = merged_target(&request, &snapshot)?;
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_product_option_translation_exact_locale(
                    tenant_id,
                    security.user_id,
                    option_id,
                    ProductOptionTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        title,
                        values,
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
            .map_err(option_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.option_translation_receipt_identity_invalid",
                    "Product Option owner receipt is not bound to the active translation operation",
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
            "Product Option translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.option_translation_permission_denied",
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
            "product.option_translation_identity_invalid",
            "Product Option translation identity must address product/option without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.option_translation_resource_id_invalid",
            "Product Option translation resource id must be a UUID",
        )
    })
}

fn option_identity(option_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Option resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(option_id.to_string())
            .expect("Option UUID must satisfy the target resource id contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    snapshot: ProductOptionTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    let display_label = if snapshot.source.title.trim().is_empty() {
        format!("Option {}", snapshot.option_id)
    } else {
        snapshot.source.title.clone()
    };
    Ok(TranslationResourceSummary {
        identity: option_identity(snapshot.option_id),
        display_label,
        lifecycle: product_lifecycle(&snapshot.product_status),
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: ProductOptionTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: option_identity(snapshot.option_id),
        display_label: if snapshot.source.title.trim().is_empty() {
            format!("Option {}", snapshot.option_id)
        } else {
            snapshot.source.title.clone()
        },
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
        fields: option_fields(&snapshot.source, snapshot.target.as_ref())?,
    };
    resource.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.option_translation_snapshot_invalid",
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
                    "product.option_translation_locale_invalid",
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

fn option_fields(
    source: &ProductOptionTranslationExactLocaleRecord,
    target: Option<&ProductOptionTranslationExactLocaleRecord>,
) -> Result<Vec<TranslationFieldSnapshot>, PortError> {
    let mut fields = Vec::with_capacity(source.values.len() + 1);
    fields.push(text_field(
        FieldKey::new(OPTION_TITLE_FIELD).expect("static Option title field must be valid"),
        &source.title,
        target.map(|target| target.title.clone()),
    ));
    for source_value in &source.values {
        let key = value_field_key(source_value.value_id)?;
        let target_value = target
            .and_then(|target| {
                target
                    .values
                    .iter()
                    .find(|value| value.value_id == source_value.value_id)
            })
            .map(|value| value.value.clone());
        fields.push(text_field(key, &source_value.value, target_value));
    }
    Ok(fields)
}

fn text_field(
    key: FieldKey,
    source_value: &str,
    exact_target_value: Option<String>,
) -> TranslationFieldSnapshot {
    TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key,
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: true,
            ai_export_allowed: true,
            max_characters: Some(100),
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value,
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    }
}

fn value_field_key(value_id: Uuid) -> Result<FieldKey, PortError> {
    FieldKey::new(format!("{OPTION_VALUE_FIELD_PREFIX}{value_id}")).map_err(|error| {
        PortError::invariant_violation(
            "product.option_translation_field_key_invalid",
            error.to_string(),
        )
    })
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<(String, Vec<ProductOptionTranslationExactLocaleValueApply>), PortError> {
    let option_id = parse_identity(&request.identity)?;
    let mut values = merged_patch_values(request, snapshot);
    let title = required_target_value(values.remove(OPTION_TITLE_FIELD).flatten(), "option title")?;
    let mut translated_values = Vec::new();
    for field in &snapshot.fields {
        let Some(value_id) = parse_value_field_key(&field.descriptor.key)? else {
            continue;
        };
        let value = required_target_value(
            values.remove(field.descriptor.key.as_str()).flatten(),
            "option value",
        )?;
        translated_values.push(ProductOptionTranslationExactLocaleValueApply { value_id, value });
    }
    if translated_values.len() + 1 != snapshot.fields.len() {
        return Err(PortError::invariant_violation(
            "product.option_translation_field_set_invalid",
            format!("Product Option {option_id} translation field set is invalid"),
        ));
    }
    Ok((title, translated_values))
}

fn parse_value_field_key(key: &FieldKey) -> Result<Option<Uuid>, PortError> {
    let Some(raw) = key.as_str().strip_prefix(OPTION_VALUE_FIELD_PREFIX) else {
        return Ok(None);
    };
    Uuid::parse_str(raw).map(Some).map_err(|_| {
        PortError::invariant_violation(
            "product.option_translation_field_key_invalid",
            "Product Option value field key does not contain a valid value UUID",
        )
    })
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<ProductOptionTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &ProductOptionTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "product.option_translation_receipt_identity_missing",
            "Product Option translation owner receipt is missing its operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-option:{operation_id}"),
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
            "product.option_translation_revision_invalid",
            format!("Product Option {field} is invalid: {error}"),
        )
    })
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
            "Product Option values changed while the translation mutation was being prepared",
        ),
        ProductOptionTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "product.option_translation_revision_conflict",
            "Product Option translation state conflicts with the requested mutation",
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
                "Product state conflicts with the requested Option translation mutation",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.option_translation_owner_validation",
            "Product rejected the Option translation mutation",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.option_translation_owner_conflict",
            "Product state conflicts with the requested Option translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.option_translation_owner_invariant",
            "Product Option translation state is invalid",
        ),
    }
}

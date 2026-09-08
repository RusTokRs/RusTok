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
        normalize_optional_target_value, read_request_from_patch, required_target_value,
        validate_patch_against_snapshot, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    CommerceError,
    services::collection_translation::{
        CollectionTranslationExactLocaleApply, CollectionTranslationExactLocaleApplyReceipt,
        CollectionTranslationExactLocaleError, CollectionTranslationExactLocaleRecord,
        CollectionTranslationExactLocaleSnapshot, CollectionTranslationService,
        with_collection_translation_operation_receipt,
    },
};

const TRANSLATION_OWNER_SLUG: &str = "commerce";
const TRANSLATION_RESOURCE_KIND: &str = "collection_copy";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_collection_copy_patch";

#[derive(Clone)]
pub struct CommerceCollectionTranslationTargetProvider {
    service: Arc<CollectionTranslationService>,
}

impl CommerceCollectionTranslationTargetProvider {
    pub fn new(service: Arc<CollectionTranslationService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Commerce owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Collection Copy resource kind must satisfy the target contract"),
            display_name: "Commerce collections".to_string(),
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
        let collection_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_exact_locale(
                tenant_id,
                collection_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(collection_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Commerce Collection translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for CommerceCollectionTranslationTargetProvider {
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
                        "commerce.collection_translation_cursor_invalid",
                        "Commerce Collection translation cursor must be a Collection UUID",
                    )
                })
            })
            .transpose()?;
        let (snapshots, next_after) = self
            .service
            .list_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(collection_error_to_port_error)?;
        let resources = snapshots
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|collection_id| {
                OpaqueCursor::new(collection_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "commerce.collection_translation_cursor_invalid",
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
        let collection_id = parse_identity(&request.identity)?;
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
            let target = merged_target(&request, &snapshot)?;
            let applied = with_collection_translation_operation_receipt(
                lease,
                self.service.apply_exact_locale(
                    tenant_id,
                    security.user_id,
                    collection_id,
                    CollectionTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        title: target.title,
                        handle: target.handle,
                        description: target.description,
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
            .map_err(collection_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "commerce.collection_translation_receipt_identity_invalid",
                    "Commerce Collection owner receipt is not bound to the active translation operation",
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

#[derive(Debug)]
struct MergedCollectionTarget {
    title: String,
    handle: String,
    description: Option<String>,
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "commerce.invalid_tenant_id",
            "Commerce Collection translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "commerce.collection_translation_permission_denied",
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
            "commerce.collection_translation_identity_invalid",
            "Commerce Collection translation identity must address commerce/collection_copy without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "commerce.collection_translation_resource_id_invalid",
            "Commerce Collection translation resource id must be a UUID",
        )
    })
}

fn collection_identity(collection_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Commerce owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Collection Copy resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(collection_id.to_string())
            .expect("Collection UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    snapshot: CollectionTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: collection_identity(snapshot.collection_id),
        display_label: snapshot.source.title,
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: CollectionTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: collection_identity(snapshot.collection_id),
        display_label: snapshot.source.title.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
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
        fields: translation_fields(&snapshot.source, snapshot.target.as_ref()),
    };
    resource.validate().map_err(|error| {
        PortError::invariant_violation(
            "commerce.collection_translation_snapshot_invalid",
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
                    "commerce.collection_translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn translation_fields(
    source: &CollectionTranslationExactLocaleRecord,
    target: Option<&CollectionTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "title",
            source.title.as_str(),
            target.map(|value| value.title.as_str()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            true,
            true,
            Some(255),
        ),
        (
            "handle",
            source.handle.as_str(),
            target.map(|value| value.handle.as_str()),
            TranslationValueProfile::Slug,
            TranslationStrategy::TransliterateWithReview,
            true,
            false,
            Some(255),
        ),
        (
            "description",
            source.description.as_deref().unwrap_or_default(),
            target.and_then(|value| value.description.as_deref()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            false,
            true,
            None,
        ),
    ]
    .into_iter()
    .map(
        |(key, source_value, target_value, profile, strategy, required, ai_export_allowed, max)| {
            TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new(key)
                        .expect("static Collection field key must satisfy the target contract"),
                    profile,
                    strategy,
                    classification: TranslationDataClassification::Public,
                    required,
                    ai_export_allowed,
                    max_characters: max,
                    preserves_whitespace: false,
                },
                source_value: source_value.to_string(),
                exact_target_value: target_value.map(str::to_string),
                source_hash: field_hash(source_value),
                protected_tokens: Vec::new(),
            }
        },
    )
    .collect()
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<MergedCollectionTarget, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let title = required_target_value(values.remove("title").flatten(), "title")?;
    let handle = required_target_value(values.remove("handle").flatten(), "handle")?;
    let description = values
        .remove("description")
        .flatten()
        .and_then(normalize_optional_target_value);
    Ok(MergedCollectionTarget {
        title,
        handle,
        description,
    })
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<CollectionTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &CollectionTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "commerce.collection_translation_receipt_identity_missing",
            "Commerce Collection owner receipt is missing its operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("commerce-collection:{operation_id}"),
        resource_revision: opaque_revision(
            owner_receipt.resource_revision.clone(),
            "resource_revision",
        )?,
        target_revision: opaque_revision(
            owner_receipt.target_revision.clone(),
            "target_revision",
        )?,
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
            "commerce.collection_translation_revision_invalid",
            format!("Commerce Collection {field} is invalid: {error}"),
        )
    })
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

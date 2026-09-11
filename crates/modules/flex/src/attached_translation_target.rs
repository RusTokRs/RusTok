use std::{collections::BTreeMap, collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, normalize_optional_target_value,
        read_request_from_patch, required_target_value, validate_patch_against_snapshot,
        validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    FlexAttachedTranslationError, FlexAttachedTranslationExactLocaleApply,
    FlexAttachedTranslationExactLocaleApplyReceipt, FlexAttachedTranslationExactLocaleSnapshot,
    FlexAttachedTranslationLeaf, FlexAttachedTranslationOperationContext,
    FlexAttachedTranslationOwnerPort, FlexAttachedTranslationResult,
    FlexAttachedTranslationTargetValue, is_valid_flex_entity_type,
};

const TRANSLATION_OWNER_SLUG: &str = "flex";
const TRANSLATION_RESOURCE_KIND: &str = "attached_localized_value";
const PATCH_FINGERPRINT_NAMESPACE: &str = "rustok-flex/attached-neutral-patch/v1";

/// Neutral Translation target adapter for one registered attached Flex donor type.
///
/// The neutral provider key stays `flex/attached_localized_value`; donor identity is carried as the
/// resource subresource id and in the list cursor. This keeps one future-proof provider namespace
/// instead of creating a provider kind per donor while still preventing UUID collisions between
/// different donor entity types.
#[derive(Clone)]
pub struct FlexAttachedTranslationTargetProvider {
    owner: Arc<dyn FlexAttachedTranslationOwnerPort>,
    entity_type: String,
}

impl FlexAttachedTranslationTargetProvider {
    pub fn new(
        owner: Arc<dyn FlexAttachedTranslationOwnerPort>,
    ) -> FlexAttachedTranslationResult<Self> {
        let entity_type = owner.entity_type().to_string();
        if !is_valid_flex_entity_type(&entity_type) {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation provider owner has invalid entity type `{entity_type}`"
            )));
        }
        Ok(Self { owner, entity_type })
    }

    pub fn entity_type(&self) -> &str {
        &self.entity_type
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Flex owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Flex attached-value kind must satisfy the target contract"),
            display_name: "Flex attached localized values".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["flex_entries:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["flex_entries:update".to_string()]),
        }
    }

    async fn load_owner_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<FlexAttachedTranslationExactLocaleSnapshot, PortError> {
        let entity_id = parse_identity(&request.identity, &self.entity_type)?;
        self.owner
            .read_exact_locale(
                tenant_id,
                entity_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(owner_error_to_port_error)
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let owner = self.load_owner_snapshot(tenant_id, request).await?;
        neutralize_snapshot(owner, request, &self.entity_type).map(|neutral| neutral.snapshot)
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexAttachedTranslationTargetProvider {
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
            .map(|cursor| parse_cursor(cursor.as_str(), &self.entity_type))
            .transpose()?;
        let page = self
            .owner
            .list_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(owner_error_to_port_error)?;
        let resources = page
            .resources
            .into_iter()
            .map(|snapshot| summary_from_owner(snapshot, &self.entity_type))
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = page
            .next_after
            .map(|entity_id| attached_cursor(&self.entity_type, entity_id))
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
        ensure_distinct_locales(&request)?;
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
        let entity_id = parse_identity(&request.identity, &self.entity_type)?;
        let request_fingerprint = neutral_request_fingerprint(&request);
        let read_request = read_request_from_patch(&request);
        let owner_snapshot = self.load_owner_snapshot(tenant_id, &read_request).await?;
        let neutral =
            neutralize_snapshot(owner_snapshot.clone(), &read_request, &self.entity_type)?;
        let source_revision_matches =
            request.expected_source_revision == neutral.snapshot.source_revision;

        // Preserve owner replay semantics when the source moved. With the expected source still
        // current, reject unsupported fields/source hashes before opening the durable owner lease.
        // With a stale source, the owner distinguishes an idempotent replay from a new stale write.
        let validation = if source_revision_matches {
            only_field_issues(validate_patch_against_snapshot(&request, &neutral.snapshot))
        } else {
            accepted_validation()
        };
        if !validation.accepted {
            return Err(validation_to_port_error(&validation));
        }

        let target_values = merge_target_values(
            &request,
            &owner_snapshot,
            &neutral.leaf_by_key,
            source_revision_matches,
        )?;
        let applied = self
            .owner
            .apply_exact_locale(
                tenant_id,
                security.user_id,
                entity_id,
                FlexAttachedTranslationExactLocaleApply {
                    operation: FlexAttachedTranslationOperationContext {
                        idempotency_key: context.idempotency_key.clone().unwrap_or_default(),
                        proposal_id: request.proposal_id.clone(),
                        approval_receipt_id: request.approval_receipt_id.clone(),
                        request_fingerprint,
                    },
                    source_locale: request.source_locale.as_str().to_string(),
                    target_locale: request.target_locale.as_str().to_string(),
                    target_values,
                    expected_resource_revision: request
                        .expected_resource_revision
                        .as_str()
                        .to_string(),
                    expected_source_revision: request.expected_source_revision.as_str().to_string(),
                    expected_target_revision: request
                        .expected_target_revision
                        .as_ref()
                        .map(|revision| revision.as_str().to_string()),
                },
            )
            .await
            .map_err(owner_error_to_port_error)?;
        application_receipt(&applied, &request, entity_id, &self.entity_type)
    }
}

struct NeutralizedSnapshot {
    snapshot: TranslationResourceSnapshot,
    leaf_by_key: BTreeMap<String, FlexAttachedTranslationLeaf>,
}

fn neutralize_snapshot(
    owner: FlexAttachedTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
    expected_entity_type: &str,
) -> Result<NeutralizedSnapshot, PortError> {
    if owner.source_locale != request.source_locale.as_str()
        || owner.target_locale != request.target_locale.as_str()
    {
        return Err(PortError::invariant_violation(
            "flex.attached_translation_locale_identity_invalid",
            "Flex attached owner returned a snapshot for different exact locales",
        ));
    }
    let expected_entity_id = parse_identity(&request.identity, expected_entity_type)?;
    if owner.entity_type != expected_entity_type || owner.entity_id != expected_entity_id {
        return Err(PortError::invariant_violation(
            "flex.attached_translation_resource_identity_invalid",
            "Flex attached owner returned a different donor resource identity",
        ));
    }

    let summary = TranslationResourceSummary {
        identity: attached_identity(expected_entity_type, owner.entity_id)?,
        display_label: attached_display_label(expected_entity_type, owner.entity_id),
        lifecycle: attached_lifecycle(owner.is_active),
        resource_revision: opaque_revision(owner.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(owner.exact_locales)?,
    };

    let mut fields = Vec::with_capacity(owner.leaves.len());
    let mut leaf_by_key = BTreeMap::new();
    for leaf in &owner.leaves {
        let key = field_key_for_leaf(&leaf.leaf)?;
        if let Some(previous) = leaf_by_key.insert(key.as_str().to_string(), leaf.leaf.clone()) {
            if previous != leaf.leaf {
                return Err(PortError::invariant_violation(
                    "flex.attached_translation_field_key_collision",
                    "two Flex attached leaves resolved to the same neutral field key",
                ));
            }
            return Err(PortError::invariant_violation(
                "flex.attached_translation_duplicate_leaf",
                "Flex attached owner returned the same leaf more than once",
            ));
        }
        fields.push(field_snapshot(key, leaf));
    }

    let snapshot = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_revision(owner.source_revision, "source_revision")?,
        target_revision: owner
            .target_revision
            .map(|revision| opaque_revision(revision, "target_revision"))
            .transpose()?,
        fields,
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(NeutralizedSnapshot {
        snapshot,
        leaf_by_key,
    })
}

fn field_snapshot(
    key: FieldKey,
    leaf: &crate::FlexAttachedTranslationLeafSnapshot,
) -> TranslationFieldSnapshot {
    TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key,
            profile: TranslationValueProfile::LocalizedScalar,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::TenantPrivate,
            required: leaf.required,
            ai_export_allowed: false,
            max_characters: None,
            preserves_whitespace: false,
        },
        source_value: leaf.source_value.clone(),
        exact_target_value: leaf.target_value.clone(),
        source_hash: field_hash(&leaf.source_value),
        protected_tokens: Vec::new(),
    }
}

fn merge_target_values(
    request: &TranslationPatchRequest,
    owner: &FlexAttachedTranslationExactLocaleSnapshot,
    leaf_by_key: &BTreeMap<String, FlexAttachedTranslationLeaf>,
    enforce_complete_target: bool,
) -> Result<Vec<FlexAttachedTranslationTargetValue>, PortError> {
    let patch_values = request
        .fields
        .iter()
        .map(|field| (field.key.as_str(), field.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut target_values = Vec::with_capacity(owner.leaves.len());
    for leaf in &owner.leaves {
        let key = field_key_for_leaf(&leaf.leaf)?;
        let mapped = leaf_by_key.get(key.as_str()).ok_or_else(|| {
            PortError::invariant_violation(
                "flex.attached_translation_field_mapping_missing",
                "neutral Flex attached field mapping is incomplete",
            )
        })?;
        if mapped != &leaf.leaf {
            return Err(PortError::invariant_violation(
                "flex.attached_translation_field_mapping_invalid",
                "neutral Flex attached field mapping changed during apply",
            ));
        }
        let value = patch_values
            .get(key.as_str())
            .map(|value| (*value).to_string())
            .or_else(|| leaf.target_value.clone());
        let value = if enforce_complete_target {
            normalize_target_value(leaf.required, value)?
        } else {
            value
        };
        target_values.push(FlexAttachedTranslationTargetValue {
            leaf: leaf.leaf.clone(),
            value,
        });
    }
    Ok(target_values)
}

fn normalize_target_value(
    required: bool,
    value: Option<String>,
) -> Result<Option<String>, PortError> {
    if required {
        return required_target_value(value, "Flex attached field").map(Some);
    }
    Ok(value.and_then(normalize_optional_target_value))
}

fn field_key_for_leaf(leaf: &FlexAttachedTranslationLeaf) -> Result<FieldKey, PortError> {
    FieldKey::new(leaf.field_key.clone()).map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_field_key_invalid",
            error.to_string(),
        )
    })
}

fn neutral_request_fingerprint(request: &TranslationPatchRequest) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, PATCH_FINGERPRINT_NAMESPACE);
    hash_component(&mut hasher, request.identity.owner_slug.as_str());
    hash_component(&mut hasher, request.identity.resource_kind.as_str());
    hash_component(&mut hasher, request.identity.resource_id.as_str());
    hash_optional_component(
        &mut hasher,
        request
            .identity
            .subresource_id
            .as_ref()
            .map(|value| value.as_str()),
    );
    hash_component(&mut hasher, request.source_locale.as_str());
    hash_component(&mut hasher, request.target_locale.as_str());
    hash_component(&mut hasher, request.expected_resource_revision.as_str());
    hash_component(&mut hasher, request.expected_source_revision.as_str());
    hash_optional_component(
        &mut hasher,
        request
            .expected_target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
    );
    hash_component(&mut hasher, &request.proposal_id);
    hash_component(&mut hasher, &request.approval_receipt_id);

    let mut fields = request.fields.iter().collect::<Vec<_>>();
    fields.sort_by(|left, right| left.key.as_str().cmp(right.key.as_str()));
    hasher.update((fields.len() as u64).to_be_bytes());
    for field in fields {
        hash_component(&mut hasher, field.key.as_str());
        hash_component(&mut hasher, &field.value);
        hash_component(&mut hasher, &field.expected_source_hash);
    }
    format!(
        "flex-attached-neutral-patch-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn hash_optional_component(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_component(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_component(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn summary_from_owner(
    owner: FlexAttachedTranslationExactLocaleSnapshot,
    expected_entity_type: &str,
) -> Result<TranslationResourceSummary, PortError> {
    if owner.entity_type != expected_entity_type {
        return Err(PortError::invariant_violation(
            "flex.attached_translation_resource_identity_invalid",
            "Flex attached owner returned a resource for another donor entity type",
        ));
    }
    Ok(TranslationResourceSummary {
        identity: attached_identity(expected_entity_type, owner.entity_id)?,
        display_label: attached_display_label(expected_entity_type, owner.entity_id),
        lifecycle: attached_lifecycle(owner.is_active),
        resource_revision: opaque_revision(owner.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(owner.exact_locales)?,
    })
}

fn application_receipt(
    owner: &FlexAttachedTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
    entity_id: Uuid,
    expected_entity_type: &str,
) -> Result<TranslationApplicationReceipt, PortError> {
    if owner.entity_type != expected_entity_type
        || owner.entity_id != entity_id
        || owner.operation_id.is_nil()
    {
        return Err(PortError::invariant_violation(
            "flex.attached_translation_receipt_identity_invalid",
            "Flex attached owner receipt is not bound to the requested donor operation",
        ));
    }
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("flex-attached:{}", owner.operation_id),
        resource_revision: opaque_revision(owner.resource_revision.clone(), "resource_revision")?,
        target_revision: opaque_revision(owner.target_revision.clone(), "target_revision")?,
        applied_field_keys: request
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect(),
    })
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    parse_non_nil_uuid(
        &context.tenant_id,
        "flex.invalid_tenant_id",
        "Flex attached translation target context must carry a non-nil UUID tenant_id",
    )
}

fn parse_identity(
    identity: &TranslationResourceIdentity,
    expected_entity_type: &str,
) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.as_ref().map(ResourceId::as_str) != Some(expected_entity_type)
    {
        return Err(PortError::validation(
            "flex.attached_translation_identity_invalid",
            format!(
                "Flex attached translation identity must address flex/attached_localized_value/{expected_entity_type}"
            ),
        ));
    }
    parse_non_nil_uuid(
        identity.resource_id.as_str(),
        "flex.attached_translation_resource_id_invalid",
        "Flex attached translation resource id must be a non-nil UUID",
    )
}

fn parse_cursor(value: &str, expected_entity_type: &str) -> Result<Uuid, PortError> {
    let Some((entity_type, entity_id)) = value.rsplit_once(':') else {
        return Err(PortError::validation(
            "flex.attached_translation_cursor_invalid",
            "Flex attached translation cursor must contain donor type and resource UUID",
        ));
    };
    if entity_type != expected_entity_type {
        return Err(PortError::validation(
            "flex.attached_translation_cursor_invalid",
            "Flex attached translation cursor belongs to another donor entity type",
        ));
    }
    parse_non_nil_uuid(
        entity_id,
        "flex.attached_translation_cursor_invalid",
        "Flex attached translation cursor resource id must be a non-nil UUID",
    )
}

fn parse_non_nil_uuid(value: &str, code: &str, message: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| PortError::validation(code.to_string(), message.to_string()))
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::FlexEntries, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "flex.attached_translation_permission_denied",
            format!("flex_entries:{action} permission is required"),
        ));
    }
    Ok(security)
}

fn ensure_distinct_locales(request: &ReadTranslationResourceRequest) -> Result<(), PortError> {
    if request.source_locale == request.target_locale {
        return Err(PortError::validation(
            "translation.equal_source_target_locale",
            "source and target locale must differ",
        ));
    }
    Ok(())
}

pub(crate) fn attached_identity(
    entity_type: &str,
    entity_id: Uuid,
) -> Result<TranslationResourceIdentity, PortError> {
    Ok(TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Flex owner slug must satisfy target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Flex resource kind must satisfy target contract"),
        resource_id: ResourceId::new(entity_id.to_string())
            .expect("resource UUID must satisfy resource id contract"),
        subresource_id: Some(ResourceId::new(entity_type.to_string()).map_err(|error| {
            PortError::invariant_violation(
                "flex.attached_translation_entity_type_invalid",
                error.to_string(),
            )
        })?),
    })
}

fn attached_display_label(entity_type: &str, entity_id: Uuid) -> String {
    format!("{entity_type}/{entity_id}")
}

fn attached_lifecycle(is_active: bool) -> TranslationResourceLifecycle {
    if is_active {
        TranslationResourceLifecycle::Active
    } else {
        TranslationResourceLifecycle::Archived
    }
}

fn attached_cursor(entity_type: &str, entity_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{entity_type}:{entity_id}")).map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_cursor_invalid",
            error.to_string(),
        )
    })
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "flex.attached_translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_revision_invalid",
            format!("Flex attached {field} is invalid: {error}"),
        )
    })
}

pub(crate) fn owner_error_to_port_error(error: FlexAttachedTranslationError) -> PortError {
    match error {
        FlexAttachedTranslationError::Invalid(_) => PortError::validation(
            "flex.attached_translation_owner_validation",
            "Flex rejected the attached translation request",
        ),
        FlexAttachedTranslationError::EntityNotFound { .. } => PortError::not_found(
            "flex.attached_translation_resource_not_found",
            "Attached Flex donor resource was not found",
        ),
        FlexAttachedTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "flex.attached_translation_source_not_found",
            "Exact source attached Flex locale was not found",
        ),
        FlexAttachedTranslationError::RevisionConflict { .. } => PortError::conflict(
            "flex.attached_translation_revision_conflict",
            "Attached Flex translation state conflicts with the request",
        ),
        FlexAttachedTranslationError::Operation(error) => error,
        FlexAttachedTranslationError::Storage(_) => PortError::unavailable(
            "flex.attached_translation_owner_unavailable",
            "Attached Flex translation storage is temporarily unavailable",
        ),
        FlexAttachedTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.attached_translation_owner_invariant",
            "Attached Flex translation owner state is invalid",
        ),
    }
}

fn only_field_issues(validation: TranslationPatchValidation) -> TranslationPatchValidation {
    let issues = validation
        .issues
        .into_iter()
        .filter(|issue| {
            matches!(
                issue.code.as_str(),
                "field_not_supported" | "source_hash_conflict"
            )
        })
        .collect::<Vec<_>>();
    TranslationPatchValidation {
        accepted: issues.is_empty(),
        issues,
    }
}

fn accepted_validation() -> TranslationPatchValidation {
    TranslationPatchValidation {
        accepted: true,
        issues: Vec::new(),
    }
}

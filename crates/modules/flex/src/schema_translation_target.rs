use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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
    FlexSchemaTranslationError, FlexSchemaTranslationExactLocaleApply,
    FlexSchemaTranslationExactLocaleApplyReceipt, FlexSchemaTranslationExactLocaleSnapshot,
    FlexSchemaTranslationLeaf, FlexSchemaTranslationOperationContext,
    FlexSchemaTranslationOwnerPort, FlexSchemaTranslationTargetValue,
};

const TRANSLATION_OWNER_SLUG: &str = "flex";
const TRANSLATION_RESOURCE_KIND: &str = "schema_copy";
const FIELD_KEY_NAMESPACE: &str = "rustok-flex/schema-copy-field-key/v1";
const PATCH_FINGERPRINT_NAMESPACE: &str = "rustok-flex/schema-copy-neutral-patch/v1";

#[derive(Clone)]
pub struct FlexSchemaTranslationTargetProvider {
    owner: Arc<dyn FlexSchemaTranslationOwnerPort>,
}

impl FlexSchemaTranslationTargetProvider {
    pub fn new(owner: Arc<dyn FlexSchemaTranslationOwnerPort>) -> Self {
        Self { owner }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Flex owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Flex schema-copy kind must satisfy the target contract"),
            display_name: "Flex schema copy".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["flex_schemas:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["flex_schemas:update".to_string()]),
        }
    }

    async fn load_owner_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<FlexSchemaTranslationExactLocaleSnapshot, PortError> {
        let schema_id = parse_identity(&request.identity)?;
        self.owner
            .read_exact_locale(
                tenant_id,
                schema_id,
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
        neutralize_snapshot(owner, request).map(|neutral| neutral.snapshot)
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexSchemaTranslationTargetProvider {
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
            .map(|cursor| parse_cursor(cursor.as_str()))
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
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = page.next_after.map(schema_cursor).transpose()?;
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
        let schema_id = parse_identity(&request.identity)?;
        let request_fingerprint = neutral_request_fingerprint(&request);
        let read_request = read_request_from_patch(&request);
        let owner_snapshot = self.load_owner_snapshot(tenant_id, &read_request).await?;
        let neutral = neutralize_snapshot(owner_snapshot.clone(), &read_request)?;
        let source_revision_matches =
            request.expected_source_revision == neutral.snapshot.source_revision;

        // With the exact proposal source still live, enforce the neutral field mapping and
        // per-field source hashes. If the source revision already moved, only the owner can
        // distinguish an idempotent retry from a genuinely stale new operation: durable
        // admission replays the former, authoritative source CAS rejects the latter.
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
                schema_id,
                FlexSchemaTranslationExactLocaleApply {
                    operation: FlexSchemaTranslationOperationContext {
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
        application_receipt(&applied, &request, schema_id)
    }
}

struct NeutralizedSnapshot {
    snapshot: TranslationResourceSnapshot,
    leaf_by_key: BTreeMap<String, FlexSchemaTranslationLeaf>,
}

fn neutralize_snapshot(
    owner: FlexSchemaTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<NeutralizedSnapshot, PortError> {
    if owner.source_locale != request.source_locale.as_str()
        || owner.target_locale != request.target_locale.as_str()
    {
        return Err(PortError::invariant_violation(
            "flex.schema_translation_locale_identity_invalid",
            "Flex owner returned a snapshot for different exact locales",
        ));
    }
    let expected_schema_id = parse_identity(&request.identity)?;
    if owner.schema_id != expected_schema_id {
        return Err(PortError::invariant_violation(
            "flex.schema_translation_resource_identity_invalid",
            "Flex owner returned a different schema identity",
        ));
    }

    let display_label = source_schema_name(&owner)?;
    let summary = TranslationResourceSummary {
        identity: schema_identity(owner.schema_id),
        display_label,
        lifecycle: schema_lifecycle(owner.is_active),
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
                    "flex.schema_translation_field_key_collision",
                    "two Flex schema-copy leaves resolved to the same neutral field key",
                ));
            }
            return Err(PortError::invariant_violation(
                "flex.schema_translation_duplicate_leaf",
                "Flex owner returned the same schema-copy leaf more than once",
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
            "flex.schema_translation_snapshot_invalid",
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
    leaf: &crate::FlexSchemaTranslationLeafSnapshot,
) -> TranslationFieldSnapshot {
    let (profile, required, max_characters) = match &leaf.leaf {
        FlexSchemaTranslationLeaf::SchemaName => {
            (TranslationValueProfile::PlainText, true, Some(255))
        }
        FlexSchemaTranslationLeaf::SchemaDescription => {
            (TranslationValueProfile::PlainText, false, None)
        }
        FlexSchemaTranslationLeaf::FieldLabel { .. } => {
            (TranslationValueProfile::LocalizedScalar, true, None)
        }
        FlexSchemaTranslationLeaf::FieldDescription { .. }
        | FlexSchemaTranslationLeaf::FieldValidationErrorMessage { .. } => {
            (TranslationValueProfile::LocalizedScalar, false, None)
        }
        FlexSchemaTranslationLeaf::FieldOptionLabel { .. } => {
            (TranslationValueProfile::LocalizedScalar, true, None)
        }
    };
    TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key,
            profile,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::TenantPrivate,
            required,
            ai_export_allowed: true,
            max_characters,
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
    owner: &FlexSchemaTranslationExactLocaleSnapshot,
    leaf_by_key: &BTreeMap<String, FlexSchemaTranslationLeaf>,
    enforce_complete_target: bool,
) -> Result<Vec<FlexSchemaTranslationTargetValue>, PortError> {
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
                "flex.schema_translation_field_mapping_missing",
                "neutral Flex schema-copy field mapping is incomplete",
            )
        })?;
        if mapped != &leaf.leaf {
            return Err(PortError::invariant_violation(
                "flex.schema_translation_field_mapping_invalid",
                "neutral Flex schema-copy field mapping changed during apply",
            ));
        }
        let value = patch_values
            .get(key.as_str())
            .map(|value| (*value).to_string())
            .or_else(|| leaf.target_value.clone());
        let value = if enforce_complete_target {
            normalize_target_value(&leaf.leaf, value)?
        } else {
            value
        };
        target_values.push(FlexSchemaTranslationTargetValue {
            leaf: leaf.leaf.clone(),
            value,
        });
    }
    Ok(target_values)
}

fn normalize_target_value(
    leaf: &FlexSchemaTranslationLeaf,
    value: Option<String>,
) -> Result<Option<String>, PortError> {
    if leaf_is_required(leaf) {
        return required_target_value(value, "Flex schema-copy field").map(Some);
    }
    Ok(value.and_then(normalize_optional_target_value))
}

fn leaf_is_required(leaf: &FlexSchemaTranslationLeaf) -> bool {
    matches!(
        leaf,
        FlexSchemaTranslationLeaf::SchemaName
            | FlexSchemaTranslationLeaf::FieldLabel { .. }
            | FlexSchemaTranslationLeaf::FieldOptionLabel { .. }
    )
}

fn field_key_for_leaf(leaf: &FlexSchemaTranslationLeaf) -> Result<FieldKey, PortError> {
    let value = match leaf {
        FlexSchemaTranslationLeaf::SchemaName => "schema_name".to_string(),
        FlexSchemaTranslationLeaf::SchemaDescription => "schema_description".to_string(),
        FlexSchemaTranslationLeaf::FieldLabel { field_key } => {
            dynamic_field_key("label", &[field_key])
        }
        FlexSchemaTranslationLeaf::FieldDescription { field_key } => {
            dynamic_field_key("description", &[field_key])
        }
        FlexSchemaTranslationLeaf::FieldValidationErrorMessage { field_key } => {
            dynamic_field_key("validation_error", &[field_key])
        }
        FlexSchemaTranslationLeaf::FieldOptionLabel {
            field_key,
            option_value,
        } => dynamic_field_key("option_label", &[field_key, option_value]),
    };
    FieldKey::new(value).map_err(|error| {
        PortError::invariant_violation(
            "flex.schema_translation_field_key_invalid",
            error.to_string(),
        )
    })
}

fn dynamic_field_key(kind: &str, components: &[&String]) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, FIELD_KEY_NAMESPACE);
    hash_component(&mut hasher, kind);
    for component in components {
        hash_component(&mut hasher, component);
    }
    format!("flex_leaf_v1:{kind}:{}", hex::encode(hasher.finalize()))
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
    format!("flex-neutral-patch-v1:{}", hex::encode(hasher.finalize()))
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

fn source_schema_name(
    owner: &FlexSchemaTranslationExactLocaleSnapshot,
) -> Result<String, PortError> {
    owner
        .leaves
        .iter()
        .find(|leaf| leaf.leaf == FlexSchemaTranslationLeaf::SchemaName)
        .map(|leaf| leaf.source_value.clone())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "flex.schema_translation_source_name_missing",
                "Flex owner snapshot is missing the required exact source schema name",
            )
        })
}

fn summary_from_owner(
    owner: FlexSchemaTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: schema_identity(owner.schema_id),
        display_label: source_schema_name(&owner)?,
        lifecycle: schema_lifecycle(owner.is_active),
        resource_revision: opaque_revision(owner.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(owner.exact_locales)?,
    })
}

fn application_receipt(
    owner: &FlexSchemaTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
    schema_id: Uuid,
) -> Result<TranslationApplicationReceipt, PortError> {
    if owner.schema_id != schema_id || owner.operation_id.is_nil() {
        return Err(PortError::invariant_violation(
            "flex.schema_translation_receipt_identity_invalid",
            "Flex owner receipt is not bound to the requested schema operation",
        ));
    }
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("flex-schema:{}", owner.operation_id),
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
        "Flex schema translation target context must carry a non-nil UUID tenant_id",
    )
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "flex.schema_translation_identity_invalid",
            "Flex schema translation identity must address flex/schema_copy without a subresource",
        ));
    }
    parse_non_nil_uuid(
        identity.resource_id.as_str(),
        "flex.schema_translation_resource_id_invalid",
        "Flex schema translation resource id must be a non-nil UUID",
    )
}

fn parse_cursor(value: &str) -> Result<Uuid, PortError> {
    parse_non_nil_uuid(
        value,
        "flex.schema_translation_cursor_invalid",
        "Flex schema translation cursor must be a non-nil schema UUID",
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
    if security.get_scope(Resource::FlexSchemas, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "flex.schema_translation_permission_denied",
            format!("flex_schemas:{action} permission is required"),
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

fn schema_identity(schema_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Flex owner slug must satisfy target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Flex resource kind must satisfy target contract"),
        resource_id: ResourceId::new(schema_id.to_string())
            .expect("schema UUID must satisfy resource id contract"),
        subresource_id: None,
    }
}

fn schema_lifecycle(is_active: bool) -> TranslationResourceLifecycle {
    if is_active {
        TranslationResourceLifecycle::Active
    } else {
        TranslationResourceLifecycle::Archived
    }
}

fn schema_cursor(schema_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(schema_id.to_string()).map_err(|error| {
        PortError::invariant_violation("flex.schema_translation_cursor_invalid", error.to_string())
    })
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "flex.schema_translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "flex.schema_translation_revision_invalid",
            format!("Flex schema {field} is invalid: {error}"),
        )
    })
}

fn owner_error_to_port_error(error: FlexSchemaTranslationError) -> PortError {
    match error {
        FlexSchemaTranslationError::Invalid(_) => PortError::validation(
            "flex.schema_translation_owner_validation",
            "Flex rejected the schema translation request",
        ),
        FlexSchemaTranslationError::SchemaNotFound(_) => PortError::not_found(
            "flex.schema_translation_resource_not_found",
            "Flex schema translation resource was not found",
        ),
        FlexSchemaTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "flex.schema_translation_source_not_found",
            "Exact source Flex schema locale was not found",
        ),
        FlexSchemaTranslationError::RevisionConflict { .. } => PortError::conflict(
            "flex.schema_translation_revision_conflict",
            "Flex schema translation state conflicts with the request",
        ),
        FlexSchemaTranslationError::Operation(error) => error,
        FlexSchemaTranslationError::Database(_) => PortError::unavailable(
            "flex.schema_translation_owner_unavailable",
            "Flex schema translation storage is temporarily unavailable",
        ),
        FlexSchemaTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.schema_translation_owner_invariant",
            "Flex schema translation owner state is invalid",
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

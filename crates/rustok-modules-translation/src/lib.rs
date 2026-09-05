//! Owner adapter boundary between static module Settings and the neutral
//! Translation target SPI.
//!
//! This crate deliberately contains no persistence access and does not register
//! a `TranslationTargetProvider`. It maps stable owner identities, conservative
//! field descriptors, opaque revision facts, and neutral patch preconditions
//! into deterministic owner apply commands. Execution remains behind
//! `rustok-modules` owner services until runtime provider registration is proven.

use std::collections::BTreeMap;

use rustok_modules::{
    ModuleCommandContext, StaticLocalizedSettingApplyCommand, StaticSettingsLocalizationRegistry,
    is_valid_static_module_slug,
    static_settings_translation_read::{
        StaticSettingsExactLocaleField, StaticSettingsExactLocaleSnapshot,
    },
};
use rustok_translation_targets::{
    FieldKey, OpaqueRevision, OwnerSlug, ResourceId, ResourceKind,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationPatchIssue,
    TranslationPatchIssueSeverity, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourceIdentity, TranslationStrategy, TranslationTargetContractError,
    TranslationValueProfile, provider_support::field_hash,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const STATIC_SETTINGS_TRANSLATION_OWNER_SLUG: &str = "modules";
pub const STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND: &str = "static_settings";
const RESOURCE_REVISION_PREFIX: &str = "settings-owner-v1";
const SOURCE_REVISION_PREFIX: &str = "settings-source-v1";
const TARGET_REVISION_PREFIX: &str = "settings-target-v1";
const APPLY_STEP_IDEMPOTENCY_PREFIX: &str = "settings-apply-step-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticSettingsTranslationIdentity {
    resource: TranslationResourceIdentity,
    field_keys: Vec<FieldKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticSettingsTranslationRevisions {
    pub resource_revision: OpaqueRevision,
    pub source_revision: OpaqueRevision,
    pub target_revision: Option<OpaqueRevision>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticSettingsTranslationApplyPlan {
    pub revisions: StaticSettingsTranslationRevisions,
    pub commands: Vec<StaticLocalizedSettingApplyCommand>,
    pub final_owner_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticSettingsTranslationPrepareResult {
    Ready(StaticSettingsTranslationApplyPlan),
    Rejected(TranslationPatchValidation),
}

impl StaticSettingsTranslationIdentity {
    /// Maps one validated static Settings localization registry to one stable
    /// Translation resource. The module slug is the resource ID and the
    /// registry's stable localized field IDs are the only field identities.
    pub fn from_registry(
        registry: &StaticSettingsLocalizationRegistry,
    ) -> Result<Self, StaticSettingsTranslationIdentityError> {
        let resource = TranslationResourceIdentity {
            owner_slug: OwnerSlug::new(STATIC_SETTINGS_TRANSLATION_OWNER_SLUG)?,
            resource_kind: ResourceKind::new(STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND)?,
            resource_id: ResourceId::new(registry.module_slug())?,
            subresource_id: None,
        };
        let field_keys = registry
            .localized_fields()
            .keys()
            .map(|field_id| FieldKey::new(field_id.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            resource,
            field_keys,
        })
    }

    pub fn resource(&self) -> &TranslationResourceIdentity {
        &self.resource
    }

    pub fn field_keys(&self) -> &[FieldKey] {
        &self.field_keys
    }

    /// Returns the neutral descriptor policy for every owner-admitted field.
    ///
    /// A descriptor becomes a Translation unit only when the owner source
    /// snapshot actually contains that field. At that point exact target copy
    /// is required for completeness, matching the owner progress contract.
    /// Settings copy is tenant-private and explicitly localizable, but AI export
    /// stays denied until a future owner metadata contract opts a field in.
    /// Owner validation remains authoritative for concrete min/max constraints,
    /// so this adapter does not fabricate a weaker or rounded character limit.
    pub fn field_descriptors(&self) -> Vec<TranslationFieldDescriptor> {
        self.field_keys
            .iter()
            .cloned()
            .map(descriptor_for_key)
            .collect()
    }

    pub fn descriptor_for_field(&self, field: &FieldKey) -> Option<TranslationFieldDescriptor> {
        self.field_keys
            .binary_search(field)
            .ok()
            .map(|index| descriptor_for_key(self.field_keys[index].clone()))
    }

    /// Maps one stable exact owner snapshot into the neutral opaque revision
    /// contract without reading persistence or inventing a numeric aggregate
    /// target revision.
    pub fn revisions_for_snapshot(
        &self,
        snapshot: &StaticSettingsExactLocaleSnapshot,
    ) -> Result<StaticSettingsTranslationRevisions, StaticSettingsTranslationIdentityError> {
        let fields = self.validated_sorted_snapshot_fields(snapshot)?;

        let resource_revision = OpaqueRevision::new(format!(
            "{RESOURCE_REVISION_PREFIX}:{}",
            snapshot.owner_revision
        ))?;

        let mut source_hasher = Sha256::new();
        hash_part(&mut source_hasher, SOURCE_REVISION_PREFIX.as_bytes());
        hash_part(&mut source_hasher, snapshot.module_slug.as_bytes());
        hash_part(&mut source_hasher, snapshot.source_locale.as_bytes());
        for field in &fields {
            hash_part(&mut source_hasher, field.field_id.as_bytes());
            hash_part(&mut source_hasher, field.source_value.as_bytes());
        }
        let source_revision = digest_revision(SOURCE_REVISION_PREFIX, source_hasher)?;

        let has_exact_target = fields.iter().any(|field| field.target_revision.is_some());
        let target_revision = if has_exact_target {
            let mut target_hasher = Sha256::new();
            hash_part(&mut target_hasher, TARGET_REVISION_PREFIX.as_bytes());
            hash_part(&mut target_hasher, snapshot.module_slug.as_bytes());
            hash_part(&mut target_hasher, snapshot.target_locale.as_bytes());
            for field in &fields {
                hash_part(&mut target_hasher, field.field_id.as_bytes());
                match field.target_revision {
                    Some(revision) => {
                        hash_part(&mut target_hasher, b"exact");
                        hash_part(&mut target_hasher, revision.to_string().as_bytes());
                    }
                    None => hash_part(&mut target_hasher, b"missing"),
                }
            }
            Some(digest_revision(TARGET_REVISION_PREFIX, target_hasher)?)
        } else {
            None
        };

        Ok(StaticSettingsTranslationRevisions {
            resource_revision,
            source_revision,
            target_revision,
        })
    }

    /// Validates a neutral patch against one stable owner snapshot without
    /// performing a write. Conflict issues deliberately mirror the target SPI
    /// precondition vocabulary while source field hashes remain per-field.
    pub fn validate_patch_against_snapshot(
        &self,
        request: &TranslationPatchRequest,
        snapshot: &StaticSettingsExactLocaleSnapshot,
    ) -> Result<TranslationPatchValidation, StaticSettingsTranslationIdentityError> {
        request.validate()?;
        let revisions = self.revisions_for_snapshot(snapshot)?;
        let mut issues = Vec::new();

        if request.identity != self.resource {
            issues.push(validation_issue(
                None,
                "resource_identity_conflict",
                "translation patch addresses another Settings resource",
            ));
        }
        if request.source_locale.as_str() != snapshot.source_locale {
            issues.push(validation_issue(
                None,
                "source_locale_conflict",
                "translation patch source locale no longer matches owner state",
            ));
        }
        if request.target_locale.as_str() != snapshot.target_locale {
            issues.push(validation_issue(
                None,
                "target_locale_conflict",
                "translation patch target locale no longer matches owner state",
            ));
        }
        if request.expected_resource_revision != revisions.resource_revision {
            issues.push(validation_issue(
                None,
                "resource_revision_conflict",
                "shared Settings owner revision no longer matches the proposal",
            ));
        }
        if request.expected_source_revision != revisions.source_revision {
            issues.push(validation_issue(
                None,
                "source_revision_conflict",
                "Settings source copy no longer matches the proposal",
            ));
        }
        if request.expected_target_revision != revisions.target_revision {
            issues.push(validation_issue(
                None,
                "target_revision_conflict",
                "exact Settings target state no longer matches the proposal",
            ));
        }

        let source_fields = snapshot
            .fields
            .iter()
            .map(|field| (field.field_id.as_str(), field))
            .collect::<BTreeMap<_, _>>();
        for patch in &request.fields {
            match source_fields.get(patch.key.as_str()) {
                Some(field) if field_hash(&field.source_value) != patch.expected_source_hash => {
                    issues.push(validation_issue(
                        Some(patch.key.clone()),
                        "source_hash_conflict",
                        "Settings source field no longer matches the proposal",
                    ));
                }
                Some(_) if self.contains_field(&request.identity, &patch.key) => {}
                Some(_) | None => issues.push(validation_issue(
                    Some(patch.key.clone()),
                    "field_not_supported",
                    "field is not exposed by this Settings translation resource",
                )),
            }
        }

        let validation = TranslationPatchValidation {
            accepted: issues.is_empty(),
            issues,
        };
        validation.validate()?;
        Ok(validation)
    }

    /// Converts an accepted neutral patch into deterministic sequential owner
    /// commands. This method performs no persistence writes.
    ///
    /// Each patched field keeps its own current target-row CAS revision. The
    /// shared owner revision is incremented once per command in deterministic
    /// field-key order because `apply_exact` advances that aggregate on every
    /// localized write. A unique deterministic owner idempotency UUID is derived
    /// per field step from the caller's base operation UUID, so multiple fields
    /// never collide in the owner receipt ledger.
    pub fn prepare_apply_plan(
        &self,
        request: &TranslationPatchRequest,
        snapshot: &StaticSettingsExactLocaleSnapshot,
        context: &ModuleCommandContext,
    ) -> Result<StaticSettingsTranslationPrepareResult, StaticSettingsTranslationIdentityError>
    {
        let validation = self.validate_patch_against_snapshot(request, snapshot)?;
        if !validation.accepted {
            return Ok(StaticSettingsTranslationPrepareResult::Rejected(validation));
        }
        context.validate().map_err(|error| {
            StaticSettingsTranslationIdentityError::InvalidCommandContext(error.to_string())
        })?;
        if context.tenant_id != Some(snapshot.tenant_id) {
            return Err(StaticSettingsTranslationIdentityError::InvalidCommandContext(
                "owner command tenant must match the stable Settings snapshot".to_string(),
            ));
        }

        let revisions = self.revisions_for_snapshot(snapshot)?;
        let source_fields = snapshot
            .fields
            .iter()
            .map(|field| (field.field_id.as_str(), field))
            .collect::<BTreeMap<_, _>>();
        let mut patches = request.fields.iter().collect::<Vec<_>>();
        patches.sort_by(|left, right| left.key.cmp(&right.key));

        let mut expected_owner_revision = snapshot.owner_revision;
        let mut commands = Vec::with_capacity(patches.len());
        for (index, patch) in patches.into_iter().enumerate() {
            let field = source_fields.get(patch.key.as_str()).ok_or_else(|| {
                StaticSettingsTranslationIdentityError::UnadmittedSnapshotField(
                    patch.key.as_str().to_string(),
                )
            })?;
            let mut step_context = context.clone();
            step_context.idempotency_key = derive_step_idempotency_key(
                context.idempotency_key,
                snapshot,
                patch.key.as_str(),
                index,
            );
            commands.push(StaticLocalizedSettingApplyCommand {
                tenant_id: snapshot.tenant_id,
                field_id: patch.key.as_str().to_string(),
                locale: snapshot.target_locale.clone(),
                value: patch.value.clone(),
                expected_owner_revision,
                expected_target_revision: field.target_revision.unwrap_or(0),
                context: step_context,
            });
            expected_owner_revision = expected_owner_revision.checked_add(1).ok_or(
                StaticSettingsTranslationIdentityError::OwnerRevisionOverflow,
            )?;
        }

        Ok(StaticSettingsTranslationPrepareResult::Ready(
            StaticSettingsTranslationApplyPlan {
                revisions,
                commands,
                final_owner_revision: expected_owner_revision,
            },
        ))
    }

    /// Resolves a neutral target identity back to its owner module slug while
    /// rejecting foreign owners, resource kinds, and subresource identities.
    pub fn module_slug_from_identity(
        identity: &TranslationResourceIdentity,
    ) -> Result<&str, StaticSettingsTranslationIdentityError> {
        if identity.owner_slug.as_str() != STATIC_SETTINGS_TRANSLATION_OWNER_SLUG
            || identity.resource_kind.as_str() != STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND
            || identity.subresource_id.is_some()
            || !is_valid_static_module_slug(identity.resource_id.as_str())
        {
            return Err(StaticSettingsTranslationIdentityError::ForeignIdentity);
        }
        Ok(identity.resource_id.as_str())
    }

    /// Rejects stale/wrong resource or field identities before any owner read or
    /// mutation adapter is allowed to resolve them.
    pub fn contains_field(
        &self,
        identity: &TranslationResourceIdentity,
        field: &FieldKey,
    ) -> bool {
        identity == &self.resource && self.field_keys.binary_search(field).is_ok()
    }

    fn validated_sorted_snapshot_fields<'a>(
        &self,
        snapshot: &'a StaticSettingsExactLocaleSnapshot,
    ) -> Result<Vec<&'a StaticSettingsExactLocaleField>, StaticSettingsTranslationIdentityError>
    {
        if snapshot.module_slug != self.resource.resource_id.as_str() {
            return Err(StaticSettingsTranslationIdentityError::SnapshotIdentityMismatch);
        }
        if snapshot.owner_revision == 0 {
            return Err(StaticSettingsTranslationIdentityError::InvalidSnapshot(
                "owner revision must be positive".to_string(),
            ));
        }
        let mut fields = snapshot.fields.iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.field_id.cmp(&right.field_id));
        validate_snapshot_fields(self, snapshot, &fields)?;
        Ok(fields)
    }
}

fn descriptor_for_key(key: FieldKey) -> TranslationFieldDescriptor {
    TranslationFieldDescriptor {
        key,
        profile: TranslationValueProfile::LocalizedScalar,
        strategy: TranslationStrategy::Translate,
        classification: TranslationDataClassification::TenantPrivate,
        required: true,
        ai_export_allowed: false,
        max_characters: None,
        preserves_whitespace: false,
    }
}

fn validate_snapshot_fields(
    identity: &StaticSettingsTranslationIdentity,
    snapshot: &StaticSettingsExactLocaleSnapshot,
    fields: &[&StaticSettingsExactLocaleField],
) -> Result<(), StaticSettingsTranslationIdentityError> {
    let mut previous: Option<&str> = None;
    for field in fields {
        if previous == Some(field.field_id.as_str()) {
            return Err(StaticSettingsTranslationIdentityError::InvalidSnapshot(
                format!("duplicate source field `{}`", field.field_id),
            ));
        }
        previous = Some(field.field_id.as_str());

        let key = FieldKey::new(field.field_id.clone())?;
        if !identity.contains_field(identity.resource(), &key) {
            return Err(StaticSettingsTranslationIdentityError::UnadmittedSnapshotField(
                field.field_id.clone(),
            ));
        }

        match (
            field.exact_target_value.is_some(),
            field.target_revision,
            field.target_owner_revision,
        ) {
            (false, None, None) => {}
            (true, Some(target_revision), Some(target_owner_revision)) => {
                if target_revision == 0
                    || target_owner_revision == 0
                    || target_owner_revision > snapshot.owner_revision
                {
                    return Err(StaticSettingsTranslationIdentityError::InvalidSnapshot(
                        format!(
                            "invalid exact target revision state for `{}`",
                            field.field_id
                        ),
                    ));
                }
            }
            _ => {
                return Err(StaticSettingsTranslationIdentityError::InvalidSnapshot(
                    format!(
                        "incomplete exact target revision state for `{}`",
                        field.field_id
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validation_issue(
    field: Option<FieldKey>,
    code: &str,
    message: &str,
) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Error,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn derive_step_idempotency_key(
    base: Uuid,
    snapshot: &StaticSettingsExactLocaleSnapshot,
    field_key: &str,
    index: usize,
) -> Uuid {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, APPLY_STEP_IDEMPOTENCY_PREFIX.as_bytes());
    hash_part(&mut hasher, base.as_bytes());
    hash_part(&mut hasher, snapshot.module_slug.as_bytes());
    hash_part(&mut hasher, snapshot.target_locale.as_bytes());
    hash_part(&mut hasher, field_key.as_bytes());
    hash_part(&mut hasher, index.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("usize must fit in u64 on supported targets");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

fn digest_revision(
    prefix: &str,
    hasher: Sha256,
) -> Result<OpaqueRevision, TranslationTargetContractError> {
    OpaqueRevision::new(format!("{prefix}:{}", hex::encode(hasher.finalize())))
}

#[derive(Debug, Error)]
pub enum StaticSettingsTranslationIdentityError {
    #[error("static Settings Translation identity belongs to another owner or resource kind")]
    ForeignIdentity,
    #[error("static Settings Translation snapshot belongs to another resource")]
    SnapshotIdentityMismatch,
    #[error("static Settings Translation snapshot contains unadmitted field `{0}`")]
    UnadmittedSnapshotField(String),
    #[error("static Settings Translation snapshot is inconsistent: {0}")]
    InvalidSnapshot(String),
    #[error("static Settings Translation owner command context is invalid: {0}")]
    InvalidCommandContext(String),
    #[error("static Settings Translation owner revision overflow")]
    OwnerRevisionOverflow,
    #[error(transparent)]
    TargetContract(#[from] TranslationTargetContractError),
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use rustok_api::TenantLocale;
    use rustok_modules::{
        ModuleSettingSpec,
        static_settings_translation_read::{
            StaticSettingsExactLocaleField, StaticSettingsExactLocaleSnapshot,
        },
    };
    use rustok_translation_targets::{
        FieldKey, OwnerSlug, ResourceId, ResourceKind, TranslationDataClassification,
        TranslationFieldPatch, TranslationPatchRequest, TranslationStrategy,
        TranslationValueProfile, provider_support::field_hash,
    };

    use super::*;

    fn registry() -> StaticSettingsLocalizationRegistry {
        StaticSettingsLocalizationRegistry::new(
            "storefront",
            HashMap::from([(
                "hero".to_string(),
                ModuleSettingSpec {
                    value_type: "object".to_string(),
                    properties: HashMap::from([
                        (
                            "title".to_string(),
                            ModuleSettingSpec {
                                value_type: "string".to_string(),
                                required: true,
                                max: Some(80.0),
                                ..Default::default()
                            },
                        ),
                        (
                            "subtitle".to_string(),
                            ModuleSettingSpec {
                                value_type: "string".to_string(),
                                ..Default::default()
                            },
                        ),
                    ]),
                    ..Default::default()
                },
            )]),
            BTreeMap::from([
                (
                    "storefront.hero.subtitle".to_string(),
                    "hero.subtitle".to_string(),
                ),
                (
                    "storefront.hero.title".to_string(),
                    "hero.title".to_string(),
                ),
            ]),
            BTreeSet::new(),
        )
        .expect("registry")
    }

    fn snapshot() -> StaticSettingsExactLocaleSnapshot {
        StaticSettingsExactLocaleSnapshot {
            tenant_id: Uuid::new_v4(),
            module_slug: "storefront".to_string(),
            source_locale: "en".to_string(),
            target_locale: "de".to_string(),
            owner_revision: 9,
            owner_change_seq: Some(17),
            fields: vec![
                StaticSettingsExactLocaleField {
                    field_id: "storefront.hero.title".to_string(),
                    source_value: "Welcome".to_string(),
                    exact_target_value: Some("Willkommen".to_string()),
                    target_revision: Some(2),
                    target_owner_revision: Some(8),
                },
                StaticSettingsExactLocaleField {
                    field_id: "storefront.hero.subtitle".to_string(),
                    source_value: "New season".to_string(),
                    exact_target_value: None,
                    target_revision: None,
                    target_owner_revision: None,
                },
            ],
        }
    }

    fn patch(
        identity: &StaticSettingsTranslationIdentity,
        snapshot: &StaticSettingsExactLocaleSnapshot,
    ) -> TranslationPatchRequest {
        let revisions = identity.revisions_for_snapshot(snapshot).unwrap();
        TranslationPatchRequest {
            identity: identity.resource().clone(),
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            expected_resource_revision: revisions.resource_revision,
            expected_source_revision: revisions.source_revision,
            expected_target_revision: revisions.target_revision,
            fields: vec![
                TranslationFieldPatch {
                    key: FieldKey::new("storefront.hero.title").unwrap(),
                    value: "Hallo".to_string(),
                    expected_source_hash: field_hash("Welcome"),
                },
                TranslationFieldPatch {
                    key: FieldKey::new("storefront.hero.subtitle").unwrap(),
                    value: "Neue Saison".to_string(),
                    expected_source_hash: field_hash("New season"),
                },
            ],
            proposal_id: "proposal-1".to_string(),
            approval_receipt_id: "approval-1".to_string(),
        }
    }

    fn command_context(tenant_id: Uuid) -> ModuleCommandContext {
        ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            trace_id: "trace-settings-translation".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        }
    }

    #[test]
    fn registry_maps_to_one_stable_resource_and_sorted_field_keys() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        assert_eq!(identity.resource().owner_slug.as_str(), "modules");
        assert_eq!(identity.resource().resource_kind.as_str(), "static_settings");
        assert_eq!(identity.resource().resource_id.as_str(), "storefront");
        assert!(identity.resource().subresource_id.is_none());
        assert_eq!(
            identity
                .field_keys()
                .iter()
                .map(FieldKey::as_str)
                .collect::<Vec<_>>(),
            vec!["storefront.hero.subtitle", "storefront.hero.title"]
        );
    }

    #[test]
    fn descriptors_require_exact_present_copy_and_deny_ai_export_by_default() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        for descriptor in identity.field_descriptors() {
            assert_eq!(descriptor.profile, TranslationValueProfile::LocalizedScalar);
            assert_eq!(descriptor.strategy, TranslationStrategy::Translate);
            assert_eq!(
                descriptor.classification,
                TranslationDataClassification::TenantPrivate
            );
            assert!(descriptor.required);
            assert!(!descriptor.ai_export_allowed);
            assert_eq!(descriptor.max_characters, None);
            assert!(!descriptor.preserves_whitespace);
        }
    }

    #[test]
    fn revisions_preserve_separate_source_target_clocks() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let original = snapshot();
        let revisions = identity.revisions_for_snapshot(&original).unwrap();
        let mut target_only = original.clone();
        target_only.owner_revision = 10;
        target_only.fields[0].exact_target_value = Some("Hallo".to_string());
        target_only.fields[0].target_revision = Some(3);
        target_only.fields[0].target_owner_revision = Some(10);
        let target_only_revisions = identity.revisions_for_snapshot(&target_only).unwrap();
        assert_ne!(revisions.resource_revision, target_only_revisions.resource_revision);
        assert_eq!(revisions.source_revision, target_only_revisions.source_revision);
        assert_ne!(revisions.target_revision, target_only_revisions.target_revision);
    }

    #[test]
    fn patch_validation_rejects_stale_revisions_and_source_hashes() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let snapshot = snapshot();
        let mut request = patch(&identity, &snapshot);
        request.expected_source_revision = OpaqueRevision::new("stale").unwrap();
        request.fields[0].expected_source_hash = field_hash("old source");
        let validation = identity
            .validate_patch_against_snapshot(&request, &snapshot)
            .unwrap();
        assert!(!validation.accepted);
        let codes = validation
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"source_revision_conflict"));
        assert!(codes.contains(&"source_hash_conflict"));
    }

    #[test]
    fn prepare_apply_plan_sorts_fields_and_advances_owner_cas_per_step() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let snapshot = snapshot();
        let mut request = patch(&identity, &snapshot);
        request.fields.reverse();
        let context = command_context(snapshot.tenant_id);
        let prepared = identity
            .prepare_apply_plan(&request, &snapshot, &context)
            .unwrap();
        let StaticSettingsTranslationPrepareResult::Ready(plan) = prepared else {
            panic!("valid patch must prepare owner commands");
        };
        assert_eq!(plan.commands.len(), 2);
        assert_eq!(plan.commands[0].field_id, "storefront.hero.subtitle");
        assert_eq!(plan.commands[0].expected_owner_revision, 9);
        assert_eq!(plan.commands[0].expected_target_revision, 0);
        assert_eq!(plan.commands[1].field_id, "storefront.hero.title");
        assert_eq!(plan.commands[1].expected_owner_revision, 10);
        assert_eq!(plan.commands[1].expected_target_revision, 2);
        assert_eq!(plan.final_owner_revision, 11);
        assert_ne!(
            plan.commands[0].context.idempotency_key,
            plan.commands[1].context.idempotency_key
        );
        assert_eq!(plan.commands[0].context.actor_id, context.actor_id);
        assert_eq!(plan.commands[1].context.correlation_id, context.correlation_id);
    }

    #[test]
    fn prepare_apply_plan_returns_structured_rejection_without_owner_commands() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let snapshot = snapshot();
        let mut request = patch(&identity, &snapshot);
        request.expected_resource_revision = OpaqueRevision::new("stale").unwrap();
        let prepared = identity
            .prepare_apply_plan(&request, &snapshot, &command_context(snapshot.tenant_id))
            .unwrap();
        let StaticSettingsTranslationPrepareResult::Rejected(validation) = prepared else {
            panic!("stale patch must be rejected before owner command preparation");
        };
        assert!(!validation.accepted);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.code == "resource_revision_conflict"));
    }

    #[test]
    fn prepare_apply_plan_requires_matching_owner_tenant_context() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let snapshot = snapshot();
        let request = patch(&identity, &snapshot);
        let context = command_context(Uuid::new_v4());
        assert!(matches!(
            identity.prepare_apply_plan(&request, &snapshot, &context),
            Err(StaticSettingsTranslationIdentityError::InvalidCommandContext(_))
        ));
    }

    #[test]
    fn revision_mapping_rejects_unadmitted_or_inconsistent_target_state() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let mut foreign_field = snapshot();
        foreign_field.fields[0].field_id = "storefront.hero.unknown".to_string();
        assert!(matches!(
            identity.revisions_for_snapshot(&foreign_field),
            Err(StaticSettingsTranslationIdentityError::UnadmittedSnapshotField(_))
        ));

        let mut inconsistent = snapshot();
        inconsistent.fields[0].target_revision = None;
        assert!(matches!(
            identity.revisions_for_snapshot(&inconsistent),
            Err(StaticSettingsTranslationIdentityError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn reverse_mapping_rejects_foreign_or_subresource_identity() {
        let foreign = TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("pages").unwrap(),
            resource_kind: ResourceKind::new("static_settings").unwrap(),
            resource_id: ResourceId::new("storefront").unwrap(),
            subresource_id: None,
        };
        assert!(matches!(
            StaticSettingsTranslationIdentity::module_slug_from_identity(&foreign),
            Err(StaticSettingsTranslationIdentityError::ForeignIdentity)
        ));

        let nested = TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("modules").unwrap(),
            resource_kind: ResourceKind::new("static_settings").unwrap(),
            resource_id: ResourceId::new("storefront").unwrap(),
            subresource_id: Some(ResourceId::new("hero").unwrap()),
        };
        assert!(matches!(
            StaticSettingsTranslationIdentity::module_slug_from_identity(&nested),
            Err(StaticSettingsTranslationIdentityError::ForeignIdentity)
        ));
    }
}

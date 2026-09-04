//! Owner adapter boundary between static module Settings and the neutral
//! Translation target SPI.
//!
//! This crate deliberately contains no persistence access and does not register
//! a `TranslationTargetProvider`. It maps stable owner identities, conservative
//! field descriptors, and opaque revision facts from owner snapshots already
//! admitted by `rustok-modules`. Read/apply behavior stays behind owner services
//! until the remaining adapter contract is proven.

use rustok_modules::{
    StaticSettingsLocalizationRegistry, is_valid_static_module_slug,
    static_settings_translation_read::{
        StaticSettingsExactLocaleField, StaticSettingsExactLocaleSnapshot,
    },
};
use rustok_translation_targets::{
    FieldKey, OpaqueRevision, OwnerSlug, ResourceId, ResourceKind,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationResourceIdentity,
    TranslationStrategy, TranslationTargetContractError, TranslationValueProfile,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const STATIC_SETTINGS_TRANSLATION_OWNER_SLUG: &str = "modules";
pub const STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND: &str = "static_settings";
const RESOURCE_REVISION_PREFIX: &str = "settings-owner-v1";
const SOURCE_REVISION_PREFIX: &str = "settings-source-v1";
const TARGET_REVISION_PREFIX: &str = "settings-target-v1";

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
            .map(|key| TranslationFieldDescriptor {
                key,
                profile: TranslationValueProfile::LocalizedScalar,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::TenantPrivate,
                required: true,
                ai_export_allowed: false,
                max_characters: None,
                preserves_whitespace: false,
            })
            .collect()
    }

    pub fn descriptor_for_field(&self, field: &FieldKey) -> Option<TranslationFieldDescriptor> {
        self.field_keys
            .binary_search(field)
            .ok()
            .map(|index| TranslationFieldDescriptor {
                key: self.field_keys[index].clone(),
                profile: TranslationValueProfile::LocalizedScalar,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::TenantPrivate,
                required: true,
                ai_export_allowed: false,
                max_characters: None,
                preserves_whitespace: false,
            })
    }

    /// Maps one stable exact owner snapshot into the neutral opaque revision
    /// contract without reading persistence or inventing a numeric aggregate
    /// target revision.
    ///
    /// `resource_revision` is the shared static owner revision. `source_revision`
    /// is a content digest of the canonical source locale plus the sorted current
    /// source field IDs and values, so target-only writes do not invalidate it.
    /// `target_revision` is `None` until at least one exact target row exists;
    /// once present it is a digest of the canonical target locale plus the sorted
    /// source field inventory and each field's exact target revision (or missing
    /// marker). This changes on any admitted exact-target row create/update while
    /// preserving per-field revisions for the future owner apply CAS mapping.
    pub fn revisions_for_snapshot(
        &self,
        snapshot: &StaticSettingsExactLocaleSnapshot,
    ) -> Result<StaticSettingsTranslationRevisions, StaticSettingsTranslationIdentityError> {
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
    #[error(transparent)]
    TargetContract(#[from] TranslationTargetContractError),
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use rustok_modules::{
        ModuleSettingSpec,
        static_settings_translation_read::{
            StaticSettingsExactLocaleField, StaticSettingsExactLocaleSnapshot,
        },
    };
    use rustok_translation_targets::{
        FieldKey, OwnerSlug, ResourceId, ResourceKind, TranslationDataClassification,
        TranslationStrategy, TranslationValueProfile,
    };
    use uuid::Uuid;

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
        let descriptors = identity.field_descriptors();
        assert_eq!(descriptors.len(), 2);
        for descriptor in descriptors {
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
    fn descriptor_lookup_rejects_unadmitted_field() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        assert!(identity
            .descriptor_for_field(&FieldKey::new("storefront.hero.title").unwrap())
            .is_some());
        assert!(identity
            .descriptor_for_field(&FieldKey::new("storefront.hero.missing").unwrap())
            .is_none());
    }

    #[test]
    fn revisions_are_deterministic_and_preserve_separate_source_target_clocks() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let original = snapshot();
        let revisions = identity.revisions_for_snapshot(&original).unwrap();
        assert_eq!(revisions.resource_revision.as_str(), "settings-owner-v1:9");
        assert!(revisions.source_revision.as_str().starts_with("settings-source-v1:"));
        assert!(revisions
            .target_revision
            .as_ref()
            .is_some_and(|revision| revision.as_str().starts_with("settings-target-v1:")));

        let mut target_only = original.clone();
        target_only.owner_revision = 10;
        target_only.fields[0].exact_target_value = Some("Hallo".to_string());
        target_only.fields[0].target_revision = Some(3);
        target_only.fields[0].target_owner_revision = Some(10);
        let target_only_revisions = identity.revisions_for_snapshot(&target_only).unwrap();
        assert_ne!(
            revisions.resource_revision,
            target_only_revisions.resource_revision
        );
        assert_eq!(revisions.source_revision, target_only_revisions.source_revision);
        assert_ne!(revisions.target_revision, target_only_revisions.target_revision);
    }

    #[test]
    fn target_revision_is_none_until_an_exact_row_exists() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let mut no_target = snapshot();
        no_target.fields[0].exact_target_value = None;
        no_target.fields[0].target_revision = None;
        no_target.fields[0].target_owner_revision = None;
        let revisions = identity.revisions_for_snapshot(&no_target).unwrap();
        assert!(revisions.target_revision.is_none());
    }

    #[test]
    fn revision_digests_are_independent_of_snapshot_field_order() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        let first = snapshot();
        let mut reordered = first.clone();
        reordered.fields.reverse();
        assert_eq!(
            identity.revisions_for_snapshot(&first).unwrap(),
            identity.revisions_for_snapshot(&reordered).unwrap()
        );
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

    #[test]
    fn field_membership_requires_exact_resource_and_stable_field_id() {
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry()).unwrap();
        assert!(identity.contains_field(
            identity.resource(),
            &FieldKey::new("storefront.hero.title").unwrap()
        ));
        assert!(!identity.contains_field(
            identity.resource(),
            &FieldKey::new("storefront.hero.missing").unwrap()
        ));
    }
}

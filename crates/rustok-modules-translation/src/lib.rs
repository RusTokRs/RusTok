//! Owner adapter boundary between static module Settings and the neutral
//! Translation target SPI.
//!
//! This crate deliberately contains no persistence access and does not register
//! a `TranslationTargetProvider`. It maps stable owner identities and a
//! conservative field-descriptor policy already admitted by
//! `StaticSettingsLocalizationRegistry`. Read/apply behavior stays behind
//! `rustok-modules` owner services until the remaining adapter contract is
//! proven.

use rustok_modules::{StaticSettingsLocalizationRegistry, is_valid_static_module_slug};
use rustok_translation_targets::{
    FieldKey, OwnerSlug, ResourceId, ResourceKind, TranslationDataClassification,
    TranslationFieldDescriptor, TranslationResourceIdentity, TranslationStrategy,
    TranslationTargetContractError, TranslationValueProfile,
};
use thiserror::Error;

pub const STATIC_SETTINGS_TRANSLATION_OWNER_SLUG: &str = "modules";
pub const STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND: &str = "static_settings";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticSettingsTranslationIdentity {
    resource: TranslationResourceIdentity,
    field_keys: Vec<FieldKey>,
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

#[derive(Debug, Error)]
pub enum StaticSettingsTranslationIdentityError {
    #[error("static Settings Translation identity belongs to another owner or resource kind")]
    ForeignIdentity,
    #[error(transparent)]
    TargetContract(#[from] TranslationTargetContractError),
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use rustok_modules::ModuleSettingSpec;
    use rustok_translation_targets::{
        FieldKey, OwnerSlug, ResourceId, ResourceKind, TranslationDataClassification,
        TranslationStrategy, TranslationValueProfile,
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

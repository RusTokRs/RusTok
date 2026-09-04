//! Host-owned static Settings localization registry resolution.
//!
//! Translation must not parse module manifests or read Settings persistence.
//! The server host resolves the installed static package, reads only the package
//! localization metadata slice, materializes the owner-owned
//! `StaticModulePackageContract`, and asks the Settings owner registry
//! constructor to validate the resulting localization contract.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::modules::manifest::{
    ManifestError, ManifestManager, ManifestModuleSpec, builtin_module_catalog,
    module_package_manifest_path,
};

#[derive(Debug, Default, Deserialize)]
struct StaticSettingsLocalizationPackageSlice {
    #[serde(default)]
    settings_localization: StaticSettingsLocalizationPackageMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct StaticSettingsLocalizationPackageMetadata {
    #[serde(default)]
    localized_fields: BTreeMap<String, String>,
    #[serde(default)]
    sensitive_paths: BTreeSet<String>,
}

/// Resolves the authoritative Settings localization registry for one static
/// module slug without exposing manifest/filesystem access to Translation.
///
/// Unknown or non-localized valid module slugs resolve to an empty registry,
/// matching the existing `ManifestManager::module_settings_schema` behavior.
/// Invalid slugs or invalid localization metadata fail through the owner
/// registry constructor.
pub fn resolve_static_settings_localization_registry(
    module_slug: &str,
) -> Result<rustok_modules::StaticSettingsLocalizationRegistry, ManifestError> {
    let package_contract = rustok_modules::StaticModulePackageContract {
        settings_schema: ManifestManager::module_settings_schema(module_slug)?,
        ..Default::default()
    };
    let manifest = ManifestManager::load()?;
    let package_spec = manifest
        .modules
        .get(module_slug)
        .cloned()
        .or_else(|| builtin_module_catalog().remove(module_slug));
    let metadata = match package_spec.as_ref() {
        Some(spec) => read_package_localization_metadata(spec)?,
        None => StaticSettingsLocalizationPackageMetadata::default(),
    };

    resolve_package_settings_localization_registry(module_slug, package_contract, metadata)
        .map_err(|error| ManifestError::InvalidModuleSettingSchema {
            slug: module_slug.to_string(),
            key: "settings_localization".to_string(),
            reason: error.to_string(),
        })
}

fn resolve_package_settings_localization_registry(
    module_slug: &str,
    package_contract: rustok_modules::StaticModulePackageContract,
    metadata: StaticSettingsLocalizationPackageMetadata,
) -> Result<rustok_modules::StaticSettingsLocalizationRegistry, rustok_modules::StaticSettingsLocalizationError>
{
    rustok_modules::StaticSettingsLocalizationRegistry::new(
        module_slug,
        package_contract.settings_schema,
        metadata.localized_fields,
        metadata.sensitive_paths,
    )
}

fn read_package_localization_metadata(
    spec: &ManifestModuleSpec,
) -> Result<StaticSettingsLocalizationPackageMetadata, ManifestError> {
    let Some(path) = module_package_manifest_path(spec) else {
        return Ok(StaticSettingsLocalizationPackageMetadata::default());
    };
    if !path.exists() {
        return Ok(StaticSettingsLocalizationPackageMetadata::default());
    }

    let raw = fs::read_to_string(&path).map_err(|error| ManifestError::ModulePackageRead {
        path: path.display().to_string(),
        error: error.to_string(),
    })?;
    parse_package_localization_metadata(&raw).map_err(|error| ManifestError::ModulePackageParse {
        path: path.display().to_string(),
        error: error.to_string(),
    })
}

fn parse_package_localization_metadata(
    raw: &str,
) -> Result<StaticSettingsLocalizationPackageMetadata, toml::de::Error> {
    toml::from_str::<StaticSettingsLocalizationPackageSlice>(raw)
        .map(|slice| slice.settings_localization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn package_contract() -> rustok_modules::StaticModulePackageContract {
        rustok_modules::StaticModulePackageContract {
            settings_schema: HashMap::from([
                (
                    "title".to_string(),
                    rustok_modules::ModuleSettingSpec {
                        value_type: "string".to_string(),
                        ..Default::default()
                    },
                ),
                (
                    "secret".to_string(),
                    rustok_modules::ModuleSettingSpec {
                        value_type: "string".to_string(),
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        }
    }

    #[test]
    fn package_metadata_resolves_through_owner_package_contract() {
        let metadata = parse_package_localization_metadata(
            r#"
[settings_localization]
localized_fields = { "checkout.title" = "title" }
sensitive_paths = ["secret"]
"#,
        )
        .expect("valid settings localization package metadata");

        let registry = resolve_package_settings_localization_registry(
            "checkout",
            package_contract(),
            metadata,
        )
        .expect("owner package contract must resolve valid localization metadata");
        assert_eq!(registry.module_slug(), "checkout");
        assert_eq!(
            registry
                .localized_fields()
                .get("checkout.title")
                .map(String::as_str),
            Some("title")
        );
    }

    #[test]
    fn package_metadata_cannot_localize_a_sensitive_path() {
        let metadata = StaticSettingsLocalizationPackageMetadata {
            localized_fields: BTreeMap::from([(
                "checkout.title".to_string(),
                "title".to_string(),
            )]),
            sensitive_paths: BTreeSet::from(["title".to_string()]),
        };

        let error = resolve_package_settings_localization_registry(
            "checkout",
            package_contract(),
            metadata,
        )
        .expect_err("owner registry must keep sensitivity fences authoritative");
        assert!(error.to_string().contains("sensitive path"));
    }
}

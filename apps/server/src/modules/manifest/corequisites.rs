use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::validation::{
    default_manifest_path, module_package_manifest_path, resolve_module_specs,
};
use super::{ManifestError, ManifestManager, ManifestModuleSpec, ModulesManifest};

type ModulePolicyCoRequisites = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Deserialize, Default)]
struct PackageCoRequisiteManifest {
    #[serde(default)]
    co_requisites: BTreeMap<String, PackageCoRequisiteSpec>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageCoRequisiteSpec {
    #[serde(default)]
    version_req: String,
}

#[derive(Debug, Clone, Default)]
struct StaticSelectionModule {
    version: Option<String>,
    ordinary_dependencies: BTreeSet<String>,
    co_requisites: BTreeMap<String, String>,
}

impl ManifestManager {
    /// Returns package-owned co-requisite metadata for every installed static
    /// module. The modules owner consumes this as availability input without
    /// changing ordinary dependency or migration ordering.
    pub(crate) fn module_policy_corequisites(
        manifest: &ModulesManifest,
    ) -> Result<BTreeMap<String, BTreeMap<String, String>>, ManifestError> {
        module_policy_corequisites(manifest)
    }
}

fn module_policy_corequisites(
    manifest: &ModulesManifest,
) -> Result<ModulePolicyCoRequisites, ManifestError> {
    let resolved_specs = resolve_module_specs(manifest)?;
    let mut policy = BTreeMap::new();
    let mut slugs = resolved_specs.keys().cloned().collect::<Vec<_>>();
    slugs.sort();
    for slug in slugs {
        let spec = resolved_specs
            .get(&slug)
            .expect("resolved module slug must have a manifest spec");
        let co_requisites = load_package_corequisites(spec)?;
        if !co_requisites.is_empty() {
            policy.insert(slug, co_requisites);
        }
    }
    Ok(policy)
}

/// Validates deployment-selection co-requisites after ordinary static topology
/// validation has succeeded. Co-requisites constrain the selected deployment
/// set only; they never become dependency or migration-order edges.
pub(super) fn validate_default_corequisite_selection(
    manifest: &ModulesManifest,
) -> Result<(), ManifestError> {
    let resolved_specs = resolve_module_specs(manifest)?;
    let selected = manifest
        .settings
        .default_enabled
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut modules = resolved_specs
        .iter()
        .map(|(slug, spec)| {
            (
                slug.clone(),
                StaticSelectionModule {
                    version: spec.version.clone(),
                    ordinary_dependencies: spec.depends_on.iter().cloned().collect(),
                    co_requisites: BTreeMap::new(),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    for slug in &selected {
        let Some(spec) = resolved_specs.get(slug) else {
            // Canonical static topology validation reports unknown selections
            // before this deployment-only constraint runs.
            continue;
        };
        let co_requisites = load_package_corequisites(spec)?;
        if let Some(module) = modules.get_mut(slug) {
            module.co_requisites = co_requisites;
        }
    }

    validate_corequisite_selection(&modules, &selected).map_err(|error| ManifestError::Parse {
        path: default_manifest_path().display().to_string(),
        error,
    })
}

fn load_package_corequisites(
    spec: &ManifestModuleSpec,
) -> Result<BTreeMap<String, String>, ManifestError> {
    let Some(path) = module_package_manifest_path(spec) else {
        return Ok(BTreeMap::new());
    };
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let raw = std::fs::read_to_string(&path).map_err(|error| ManifestError::ModulePackageRead {
        path: path.display().to_string(),
        error: error.to_string(),
    })?;
    let package: PackageCoRequisiteManifest =
        toml::from_str(&raw).map_err(|error| ManifestError::ModulePackageParse {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;

    Ok(package
        .co_requisites
        .into_iter()
        .map(|(slug, spec)| (slug, spec.version_req))
        .collect())
}

fn validate_corequisite_selection(
    modules: &HashMap<String, StaticSelectionModule>,
    selected: &BTreeSet<String>,
) -> Result<(), String> {
    for slug in selected {
        let Some(module) = modules.get(slug) else {
            continue;
        };

        let mut missing = Vec::new();
        for (raw_co_requisite, raw_requirement) in &module.co_requisites {
            let co_requisite = raw_co_requisite.trim();
            if !valid_module_slug(co_requisite) || co_requisite == slug {
                return Err(format!(
                    "module '{slug}' declares invalid deployment co-requisite '{raw_co_requisite}'"
                ));
            }
            if module.ordinary_dependencies.contains(co_requisite) {
                return Err(format!(
                    "module '{slug}' declares '{co_requisite}' as both an ordinary dependency and a deployment co-requisite"
                ));
            }
            let version_req = raw_requirement.trim();
            if !version_req.is_empty() {
                VersionReq::parse(version_req).map_err(|_| {
                    format!(
                        "module '{slug}' deployment co-requisite '{co_requisite}' has invalid version requirement '{version_req}'"
                    )
                })?;
            }
            if !selected.contains(co_requisite) {
                missing.push(co_requisite.to_string());
            }
        }

        missing.sort();
        missing.dedup();
        if !missing.is_empty() {
            return Err(format!(
                "module '{slug}' is selected without deployment co-requisites: {}",
                missing.join(", ")
            ));
        }

        for (raw_co_requisite, raw_requirement) in &module.co_requisites {
            let co_requisite = raw_co_requisite.trim();
            let requirement = raw_requirement.trim();
            if requirement.is_empty() {
                continue;
            }
            let provider = modules.get(co_requisite).ok_or_else(|| {
                format!(
                    "module '{slug}' selected deployment co-requisite '{co_requisite}' is not installed"
                )
            })?;
            let installed = provider.version.as_deref().ok_or_else(|| {
                format!(
                    "module '{slug}' deployment co-requisite '{co_requisite}' requires '{requirement}', but the selected module has no version"
                )
            })?;
            let installed_version = Version::parse(installed).map_err(|_| {
                format!(
                    "selected deployment co-requisite '{co_requisite}' has invalid module version '{installed}'"
                )
            })?;
            let requirement = VersionReq::parse(requirement).expect(
                "deployment co-requisite requirement was validated before provider comparison",
            );
            if !requirement.matches(&installed_version) {
                return Err(format!(
                    "module '{slug}' requires deployment co-requisite '{co_requisite}' version '{}', but selected '{installed}'",
                    raw_requirement.trim()
                ));
            }
        }
    }

    Ok(())
}

fn valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(
        version: &str,
        ordinary_dependencies: &[&str],
        co_requisites: &[(&str, &str)],
    ) -> StaticSelectionModule {
        StaticSelectionModule {
            version: Some(version.to_string()),
            ordinary_dependencies: ordinary_dependencies
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            co_requisites: co_requisites
                .iter()
                .map(|(slug, requirement)| ((*slug).to_string(), (*requirement).to_string()))
                .collect(),
        }
    }

    #[test]
    fn selection_requires_corequisites_without_creating_dependency_edges() {
        let modules = HashMap::from([
            (
                "product".to_string(),
                module(
                    "0.1.0",
                    &["taxonomy"],
                    &[("inventory", ">=0.1.0"), ("pricing", ">=0.1.0")],
                ),
            ),
            ("inventory".to_string(), module("0.1.0", &["product"], &[])),
            ("pricing".to_string(), module("0.1.0", &["product"], &[])),
            ("taxonomy".to_string(), module("0.1.0", &[], &[])),
        ]);

        let incomplete = BTreeSet::from(["product".to_string(), "taxonomy".to_string()]);
        let error = validate_corequisite_selection(&modules, &incomplete)
            .expect_err("Product-only selection must be rejected");
        assert!(error.contains("inventory"));
        assert!(error.contains("pricing"));

        let complete = BTreeSet::from([
            "product".to_string(),
            "inventory".to_string(),
            "pricing".to_string(),
            "taxonomy".to_string(),
        ]);
        assert_eq!(validate_corequisite_selection(&modules, &complete), Ok(()));
    }

    #[test]
    fn selection_rejects_incompatible_corequisite_version() {
        let modules = HashMap::from([
            (
                "product".to_string(),
                module("0.1.0", &[], &[("inventory", ">=0.2.0")]),
            ),
            ("inventory".to_string(), module("0.1.0", &["product"], &[])),
        ]);
        let selected = BTreeSet::from(["product".to_string(), "inventory".to_string()]);
        let error = validate_corequisite_selection(&modules, &selected)
            .expect_err("incompatible selected owner version must be rejected");
        assert!(error.contains("requires deployment co-requisite 'inventory' version"));
    }

    #[test]
    fn selection_rejects_corequisite_dependency_overlap() {
        let modules = HashMap::from([
            (
                "product".to_string(),
                module("0.1.0", &["inventory"], &[("inventory", ">=0.1.0")]),
            ),
            ("inventory".to_string(), module("0.1.0", &["product"], &[])),
        ]);
        let selected = BTreeSet::from(["product".to_string(), "inventory".to_string()]);
        let error = validate_corequisite_selection(&modules, &selected)
            .expect_err("co-requisite must remain separate from ordering dependencies");
        assert!(error.contains("both an ordinary dependency and a deployment co-requisite"));
    }
}

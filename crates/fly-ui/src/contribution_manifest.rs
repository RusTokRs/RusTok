use crate::{
    ContributionAssemblyDiagnostic, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionAssemblySeverity, ContributionDescriptor, ContributionProviderHealth,
    ContributionSurface, UiError,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Module-owned contribution metadata with an explicit, version-pinned target-provider allowlist.
///
/// `owner_provider` identifies the module that owns lifecycle, policy and health. A contribution's
/// `provider` identifies the component contract that its renderer/editor extends. When
/// `target_providers` is empty, only the owner provider is allowed. Cross-provider extensions must
/// therefore be declared intentionally, for example `rustok.pages -> fly.builtin@1`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleContributionManifest {
    pub module_id: String,
    pub owner_provider: String,
    pub owner_version: String,
    #[serde(default)]
    pub target_providers: BTreeMap<String, String>,
    #[serde(default)]
    pub dependencies: BTreeSet<String>,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub admin: Vec<ContributionDescriptor>,
    #[serde(default)]
    pub storefront: Vec<ContributionDescriptor>,
}

impl ModuleContributionManifest {
    pub fn allows_target_provider(&self, provider: &str, version: &str) -> bool {
        let provider = provider.trim();
        let version = version.trim();
        if provider == self.owner_provider && version == self.owner_version {
            return true;
        }
        self.target_providers
            .get(provider)
            .is_some_and(|allowed| allowed == version)
    }
}

pub fn build_admin_contribution_registry_from_manifests(
    manifests: impl IntoIterator<Item = ModuleContributionManifest>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    assemble_contribution_manifests(ContributionSurface::Admin, manifests, policy)
}

pub fn build_storefront_contribution_registry_from_manifests(
    manifests: impl IntoIterator<Item = ModuleContributionManifest>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    assemble_contribution_manifests(ContributionSurface::Storefront, manifests, policy)
}

pub fn assemble_contribution_manifests(
    surface: ContributionSurface,
    manifests: impl IntoIterator<Item = ModuleContributionManifest>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    let mut result = ContributionAssemblyResult::default();
    let mut discovered = BTreeMap::new();

    for manifest in manifests {
        let manifest = match normalize_manifest(manifest) {
            Ok(manifest) => manifest,
            Err(message) => {
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Error,
                    "contribution_manifest_invalid",
                    None,
                    None,
                    message,
                ));
                continue;
            }
        };
        if discovered.contains_key(&manifest.module_id) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&manifest, surface).len());
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Error,
                "contribution_manifest_duplicate",
                Some(&manifest.module_id),
                None,
                format!("module contribution manifest `{}` is duplicated", manifest.module_id),
            ));
            continue;
        }
        discovered.insert(manifest.module_id.clone(), manifest);
    }

    let mut selected = BTreeMap::new();
    for (module_id, manifest) in discovered {
        if let Some((code, message, severity)) = manifest_filter(&manifest, policy) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&manifest, surface).len());
            result.diagnostics.push(diagnostic(
                severity,
                code,
                Some(&manifest.module_id),
                None,
                message,
            ));
            continue;
        }
        if policy
            .provider_health
            .get(&manifest.owner_provider)
            .is_some_and(|health| {
                *health == ContributionProviderHealth::Degraded
                    && policy.allow_degraded_providers
            })
        {
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Warning,
                "contribution_owner_provider_degraded",
                Some(&manifest.module_id),
                None,
                format!("owner provider `{}` is degraded", manifest.owner_provider),
            ));
        }
        selected.insert(module_id, manifest);
    }

    remove_missing_manifest_dependencies(&mut selected, surface, &mut result);
    let order = manifest_dependency_order(&selected, surface, &mut result);

    for module_id in order {
        let Some(manifest) = selected.get(&module_id) else {
            continue;
        };
        for contribution in surface_contributions(manifest, surface) {
            if let Some((code, message)) = contribution_filter(contribution, policy) {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Info,
                    code,
                    Some(&manifest.module_id),
                    Some(contribution.id.trim()),
                    message,
                ));
                continue;
            }

            let target_provider = contribution.provider.trim();
            let target_version = contribution.provider_version.trim();
            if !manifest.allows_target_provider(target_provider, target_version) {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                let allowed = manifest
                    .target_providers
                    .iter()
                    .map(|(provider, version)| format!("{provider}@{version}"))
                    .chain(std::iter::once(format!(
                        "{}@{}",
                        manifest.owner_provider, manifest.owner_version
                    )))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ");
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Error,
                    "contribution_target_provider_forbidden",
                    Some(&manifest.module_id),
                    Some(contribution.id.trim()),
                    format!(
                        "contribution targets `{target_provider}@{target_version}`; allowed targets: {allowed}"
                    ),
                ));
                continue;
            }

            match target_provider_filter(target_provider, policy) {
                Some((code, message, severity, skip)) => {
                    result.diagnostics.push(diagnostic(
                        severity,
                        code,
                        Some(&manifest.module_id),
                        Some(contribution.id.trim()),
                        message,
                    ));
                    if skip {
                        result.skipped_contributions =
                            result.skipped_contributions.saturating_add(1);
                        continue;
                    }
                }
                None => {}
            }

            match result.registry.register(contribution.clone()) {
                Ok(()) => {
                    result.registered_contributions =
                        result.registered_contributions.saturating_add(1);
                }
                Err(error) => {
                    result.skipped_contributions =
                        result.skipped_contributions.saturating_add(1);
                    result.diagnostics.push(registration_diagnostic(
                        &manifest.module_id,
                        contribution.id.trim(),
                        error,
                    ));
                }
            }
        }
    }

    result
}

fn normalize_manifest(
    mut manifest: ModuleContributionManifest,
) -> Result<ModuleContributionManifest, String> {
    manifest.module_id = required(&manifest.module_id, "module_id")?;
    manifest.owner_provider = required(&manifest.owner_provider, "owner_provider")?;
    manifest.owner_version = required(&manifest.owner_version, "owner_version")?;
    manifest.dependencies = normalize_set(manifest.dependencies, "dependency")?;
    manifest.required_permissions =
        normalize_set(manifest.required_permissions, "required permission")?;

    let mut target_providers = BTreeMap::new();
    for (provider, version) in manifest.target_providers {
        let provider = required(&provider, "target provider")?;
        let version = required(&version, "target provider version")?;
        if provider == manifest.owner_provider {
            if version != manifest.owner_version {
                return Err(format!(
                    "target provider `{provider}@{version}` conflicts with owner version `{}`",
                    manifest.owner_version
                ));
            }
            continue;
        }
        if target_providers.insert(provider.clone(), version).is_some() {
            return Err(format!(
                "target provider `{provider}` is duplicated after normalization"
            ));
        }
    }
    manifest.target_providers = target_providers;
    Ok(manifest)
}

fn required(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_set(values: BTreeSet<String>, label: &str) -> Result<BTreeSet<String>, String> {
    let mut normalized = BTreeSet::new();
    for value in values {
        let value = required(&value, label)?;
        if !normalized.insert(value.clone()) {
            return Err(format!("{label} `{value}` is duplicated after normalization"));
        }
    }
    Ok(normalized)
}

fn manifest_filter(
    manifest: &ModuleContributionManifest,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String, ContributionAssemblySeverity)> {
    if !policy.enabled_modules.is_empty()
        && !policy.enabled_modules.contains(&manifest.module_id)
    {
        return Some((
            "contribution_module_disabled",
            format!("module `{}` is not enabled for this tenant", manifest.module_id),
            ContributionAssemblySeverity::Info,
        ));
    }
    if !policy.enabled_providers.is_empty()
        && !policy.enabled_providers.contains(&manifest.owner_provider)
    {
        return Some((
            "contribution_owner_provider_disabled",
            format!(
                "owner provider `{}` is not enabled by policy",
                manifest.owner_provider
            ),
            ContributionAssemblySeverity::Info,
        ));
    }
    let missing_permissions = manifest
        .required_permissions
        .difference(&policy.permissions)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_permissions.is_empty() {
        return Some((
            "contribution_permission_missing",
            format!(
                "module `{}` requires permissions: {}",
                manifest.module_id,
                missing_permissions.join(", ")
            ),
            ContributionAssemblySeverity::Info,
        ));
    }
    match policy.provider_health.get(&manifest.owner_provider) {
        Some(ContributionProviderHealth::Unavailable) => Some((
            "contribution_owner_provider_unavailable",
            format!("owner provider `{}` is unavailable", manifest.owner_provider),
            ContributionAssemblySeverity::Warning,
        )),
        Some(ContributionProviderHealth::Degraded) if !policy.allow_degraded_providers => Some((
            "contribution_owner_provider_degraded",
            format!(
                "owner provider `{}` is degraded and degraded providers are disabled",
                manifest.owner_provider
            ),
            ContributionAssemblySeverity::Warning,
        )),
        _ => None,
    }
}

fn target_provider_filter(
    provider: &str,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String, ContributionAssemblySeverity, bool)> {
    match policy.provider_health.get(provider) {
        Some(ContributionProviderHealth::Unavailable) => Some((
            "contribution_target_provider_unavailable",
            format!("target provider `{provider}` is unavailable"),
            ContributionAssemblySeverity::Warning,
            true,
        )),
        Some(ContributionProviderHealth::Degraded) if !policy.allow_degraded_providers => Some((
            "contribution_target_provider_degraded",
            format!(
                "target provider `{provider}` is degraded and degraded providers are disabled"
            ),
            ContributionAssemblySeverity::Warning,
            true,
        )),
        Some(ContributionProviderHealth::Degraded) => Some((
            "contribution_target_provider_degraded",
            format!("target provider `{provider}` is degraded"),
            ContributionAssemblySeverity::Warning,
            false,
        )),
        _ => None,
    }
}

fn contribution_filter(
    contribution: &ContributionDescriptor,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String)> {
    let contribution_id = contribution.id.trim();
    if policy.denied_contributions.contains(contribution_id) {
        return Some((
            "contribution_denied",
            format!("contribution `{contribution_id}` is denied by policy"),
        ));
    }
    if !policy.allowed_contributions.is_empty()
        && !policy.allowed_contributions.contains(contribution_id)
    {
        return Some((
            "contribution_not_allowed",
            format!("contribution `{contribution_id}` is not allowlisted"),
        ));
    }
    let missing = contribution
        .required_capabilities
        .difference(&policy.capabilities)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Some((
            "contribution_capability_missing",
            format!(
                "contribution `{contribution_id}` requires capabilities: {}",
                missing.join(", ")
            ),
        ));
    }
    None
}

fn remove_missing_manifest_dependencies(
    selected: &mut BTreeMap<String, ModuleContributionManifest>,
    surface: ContributionSurface,
    result: &mut ContributionAssemblyResult,
) {
    loop {
        let available = selected.keys().cloned().collect::<BTreeSet<_>>();
        let missing = selected
            .iter()
            .filter_map(|(module_id, manifest)| {
                let missing = manifest
                    .dependencies
                    .difference(&available)
                    .cloned()
                    .collect::<Vec<_>>();
                (!missing.is_empty()).then_some((module_id.clone(), missing))
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            break;
        }
        for (module_id, dependencies) in missing {
            let Some(manifest) = selected.remove(&module_id) else {
                continue;
            };
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&manifest, surface).len());
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Error,
                "contribution_dependency_missing",
                Some(&module_id),
                None,
                format!(
                    "module contribution dependencies are missing: {}",
                    dependencies.join(", ")
                ),
            ));
        }
    }
}

fn manifest_dependency_order(
    selected: &BTreeMap<String, ModuleContributionManifest>,
    surface: ContributionSurface,
    result: &mut ContributionAssemblyResult,
) -> Vec<String> {
    let mut indegree = selected
        .iter()
        .map(|(module_id, manifest)| (module_id.clone(), manifest.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, BTreeSet<String>>::new();
    for (module_id, manifest) in selected {
        for dependency in &manifest.dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .insert(module_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(module_id, degree)| (*degree == 0).then_some(module_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(selected.len());
    while let Some(module_id) = ready.pop_first() {
        order.push(module_id.clone());
        if let Some(children) = dependents.get(&module_id) {
            for child in children {
                let Some(degree) = indegree.get_mut(child) else {
                    continue;
                };
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    for (module_id, degree) in indegree {
        if degree == 0 {
            continue;
        }
        if let Some(manifest) = selected.get(&module_id) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(manifest, surface).len());
        }
        result.diagnostics.push(diagnostic(
            ContributionAssemblySeverity::Error,
            "contribution_dependency_cycle",
            Some(&module_id),
            None,
            "module contribution dependency participates in a cycle".to_string(),
        ));
    }
    order
}

fn surface_contributions(
    manifest: &ModuleContributionManifest,
    surface: ContributionSurface,
) -> &[ContributionDescriptor] {
    match surface {
        ContributionSurface::Admin => &manifest.admin,
        ContributionSurface::Storefront => &manifest.storefront,
    }
}

fn registration_diagnostic(
    module_id: &str,
    contribution_id: &str,
    error: UiError,
) -> ContributionAssemblyDiagnostic {
    let code = match &error {
        UiError::DuplicateContribution(_) => "contribution_duplicate",
        UiError::DuplicateRenderer(_) => "contribution_renderer_duplicate",
        UiError::DuplicatePropertyEditor(_) => "contribution_property_editor_duplicate",
        UiError::MissingContributionProvider(_) => "contribution_provider_missing",
        UiError::InvalidContribution { .. } => "contribution_invalid",
        _ => "contribution_registration_failed",
    };
    diagnostic(
        ContributionAssemblySeverity::Error,
        code,
        Some(module_id),
        Some(contribution_id),
        error.to_string(),
    )
}

fn diagnostic(
    severity: ContributionAssemblySeverity,
    code: impl Into<String>,
    module_id: Option<&str>,
    contribution_id: Option<&str>,
    message: String,
) -> ContributionAssemblyDiagnostic {
    ContributionAssemblyDiagnostic {
        severity,
        code: code.into(),
        module_id: module_id.map(ToString::to_string),
        contribution_id: contribution_id.map(ToString::to_string),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn contribution(id: &str, provider: &str, version: &str) -> ContributionDescriptor {
        ContributionDescriptor {
            id: id.to_string(),
            provider: provider.to_string(),
            provider_version: version.to_string(),
            required_capabilities: BTreeSet::new(),
            blocks: vec!["fly.hero".to_string()],
            renderers: Vec::new(),
            property_editors: Vec::new(),
            messages: BTreeMap::new(),
            metadata: Map::new(),
        }
    }

    fn manifest(admin: Vec<ContributionDescriptor>) -> ModuleContributionManifest {
        ModuleContributionManifest {
            module_id: "pages".to_string(),
            owner_provider: "rustok.pages".to_string(),
            owner_version: "1".to_string(),
            target_providers: BTreeMap::new(),
            dependencies: BTreeSet::new(),
            required_permissions: BTreeSet::new(),
            admin,
            storefront: Vec::new(),
        }
    }

    #[test]
    fn owner_provider_is_the_only_implicit_target() {
        let result = build_admin_contribution_registry_from_manifests(
            [manifest(vec![contribution("pages.blocks", "fly.builtin", "1")])],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(!result.is_valid());
        assert!(result.registry.is_empty());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "contribution_target_provider_forbidden"
        }));
    }

    #[test]
    fn explicit_versioned_target_provider_is_allowed() {
        let mut manifest = manifest(vec![contribution("pages.blocks", "fly.builtin", "1")]);
        manifest
            .target_providers
            .insert("fly.builtin".to_string(), "1".to_string());
        let result = build_admin_contribution_registry_from_manifests(
            [manifest],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 1);
        assert!(result.registry.get("pages.blocks").is_some());
    }

    #[test]
    fn target_provider_version_mismatch_is_rejected() {
        let mut manifest = manifest(vec![contribution("pages.blocks", "fly.builtin", "2")]);
        manifest
            .target_providers
            .insert("fly.builtin".to_string(), "1".to_string());
        let result = build_admin_contribution_registry_from_manifests(
            [manifest],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(!result.is_valid());
        assert!(result.registry.is_empty());
    }

    #[test]
    fn admin_and_storefront_surfaces_remain_separate() {
        let mut manifest = manifest(vec![contribution(
            "pages.admin.blocks",
            "rustok.pages",
            "1",
        )]);
        manifest.storefront = vec![contribution(
            "pages.storefront.renderer",
            "rustok.pages",
            "1",
        )];
        let admin = build_admin_contribution_registry_from_manifests(
            [manifest.clone()],
            &ContributionAssemblyPolicy::default(),
        );
        let storefront = build_storefront_contribution_registry_from_manifests(
            [manifest],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(admin.registry.get("pages.admin.blocks").is_some());
        assert!(admin.registry.get("pages.storefront.renderer").is_none());
        assert!(storefront.registry.get("pages.storefront.renderer").is_some());
        assert!(storefront.registry.get("pages.admin.blocks").is_none());
    }

    #[test]
    fn target_provider_health_can_block_cross_provider_extensions() {
        let mut manifest = manifest(vec![contribution("pages.blocks", "fly.builtin", "1")]);
        manifest
            .target_providers
            .insert("fly.builtin".to_string(), "1".to_string());
        let result = build_admin_contribution_registry_from_manifests(
            [manifest],
            &ContributionAssemblyPolicy {
                provider_health: BTreeMap::from([(
                    "fly.builtin".to_string(),
                    ContributionProviderHealth::Unavailable,
                )]),
                ..ContributionAssemblyPolicy::default()
            },
        );
        assert!(result.registry.is_empty());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "contribution_target_provider_unavailable"
        }));
    }
}

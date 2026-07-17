use super::ModuleContributionManifest;
use crate::{
    ContributionAssemblyDiagnostic, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionAssemblySeverity, ContributionDescriptor, ContributionProviderHealth,
    ContributionSurface, UiError,
};
use std::collections::{BTreeMap, BTreeSet};

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
    let discovered = discover_manifests(manifests, surface, &mut result);
    let mut selected = filter_manifests(discovered, surface, policy, &mut result);
    remove_missing_dependencies(&mut selected, surface, &mut result);
    let order = dependency_order(&selected, surface, &mut result);

    for module_id in order {
        let Some(manifest) = selected.get(&module_id) else {
            continue;
        };
        register_surface_contributions(manifest, surface, policy, &mut result);
    }

    result
}

fn discover_manifests(
    manifests: impl IntoIterator<Item = ModuleContributionManifest>,
    surface: ContributionSurface,
    result: &mut ContributionAssemblyResult,
) -> BTreeMap<String, ModuleContributionManifest> {
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
                format!(
                    "module contribution manifest `{}` is duplicated",
                    manifest.module_id
                ),
            ));
            continue;
        }
        discovered.insert(manifest.module_id.clone(), manifest);
    }
    discovered
}

fn filter_manifests(
    discovered: BTreeMap<String, ModuleContributionManifest>,
    surface: ContributionSurface,
    policy: &ContributionAssemblyPolicy,
    result: &mut ContributionAssemblyResult,
) -> BTreeMap<String, ModuleContributionManifest> {
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
        if provider_is_degraded(&manifest.owner_provider, policy) {
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
    selected
}

fn register_surface_contributions(
    manifest: &ModuleContributionManifest,
    surface: ContributionSurface,
    policy: &ContributionAssemblyPolicy,
    result: &mut ContributionAssemblyResult,
) {
    for contribution in surface_contributions(manifest, surface) {
        if let Some((code, message)) = contribution_filter(contribution, policy) {
            skip_contribution(
                result,
                ContributionAssemblySeverity::Info,
                code,
                manifest,
                contribution,
                message,
            );
            continue;
        }

        let target_provider = contribution.provider.trim();
        if !manifest.allows_target_provider(target_provider) {
            let allowed = allowed_target_summary(manifest);
            skip_contribution(
                result,
                ContributionAssemblySeverity::Error,
                "contribution_target_provider_forbidden",
                manifest,
                contribution,
                format!("contribution targets `{target_provider}`; allowed targets: {allowed}"),
            );
            continue;
        }

        if let Some(decision) = target_provider_filter(target_provider, policy) {
            result.diagnostics.push(diagnostic(
                decision.severity,
                decision.code,
                Some(&manifest.module_id),
                Some(contribution.id.trim()),
                decision.message,
            ));
            if decision.skip {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                continue;
            }
        }

        match result.registry.register(contribution.clone()) {
            Ok(()) => {
                result.registered_contributions = result.registered_contributions.saturating_add(1);
            }
            Err(error) => {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                result.diagnostics.push(registration_diagnostic(
                    &manifest.module_id,
                    contribution.id.trim(),
                    error,
                ));
            }
        }
    }
}

fn skip_contribution(
    result: &mut ContributionAssemblyResult,
    severity: ContributionAssemblySeverity,
    code: &'static str,
    manifest: &ModuleContributionManifest,
    contribution: &ContributionDescriptor,
    message: String,
) {
    result.skipped_contributions = result.skipped_contributions.saturating_add(1);
    result.diagnostics.push(diagnostic(
        severity,
        code,
        Some(&manifest.module_id),
        Some(contribution.id.trim()),
        message,
    ));
}

fn allowed_target_summary(manifest: &ModuleContributionManifest) -> String {
    manifest
        .target_providers
        .iter()
        .cloned()
        .chain(std::iter::once(manifest.owner_provider.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalize_manifest(
    mut manifest: ModuleContributionManifest,
) -> Result<ModuleContributionManifest, String> {
    manifest.module_id = required(&manifest.module_id, "module_id")?;
    manifest.owner_provider = required(&manifest.owner_provider, "owner_provider")?;
    manifest.dependencies = normalize_set(manifest.dependencies, "dependency")?;
    manifest.required_permissions =
        normalize_set(manifest.required_permissions, "required permission")?;
    manifest.target_providers = normalize_set(manifest.target_providers, "target provider")?;
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
            return Err(format!(
                "{label} `{value}` is duplicated after normalization"
            ));
        }
    }
    Ok(normalized)
}

fn manifest_filter(
    manifest: &ModuleContributionManifest,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String, ContributionAssemblySeverity)> {
    if !policy.enabled_modules.is_empty() && !policy.enabled_modules.contains(&manifest.module_id) {
        return Some((
            "contribution_module_disabled",
            format!(
                "module `{}` is not enabled for this tenant",
                manifest.module_id
            ),
            ContributionAssemblySeverity::Info,
        ));
    }
    if !provider_is_enabled(&manifest.owner_provider, policy) {
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
            format!(
                "owner provider `{}` is unavailable",
                manifest.owner_provider
            ),
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

struct ProviderDecision {
    code: &'static str,
    message: String,
    severity: ContributionAssemblySeverity,
    skip: bool,
}

fn target_provider_filter(
    provider: &str,
    policy: &ContributionAssemblyPolicy,
) -> Option<ProviderDecision> {
    if !provider_is_enabled(provider, policy) {
        return Some(ProviderDecision {
            code: "contribution_target_provider_disabled",
            message: format!("target provider `{provider}` is not enabled by policy"),
            severity: ContributionAssemblySeverity::Info,
            skip: true,
        });
    }
    match policy.provider_health.get(provider) {
        Some(ContributionProviderHealth::Unavailable) => Some(ProviderDecision {
            code: "contribution_target_provider_unavailable",
            message: format!("target provider `{provider}` is unavailable"),
            severity: ContributionAssemblySeverity::Warning,
            skip: true,
        }),
        Some(ContributionProviderHealth::Degraded) if !policy.allow_degraded_providers => {
            Some(ProviderDecision {
                code: "contribution_target_provider_degraded",
                message: format!(
                    "target provider `{provider}` is degraded and degraded providers are disabled"
                ),
                severity: ContributionAssemblySeverity::Warning,
                skip: true,
            })
        }
        Some(ContributionProviderHealth::Degraded) => Some(ProviderDecision {
            code: "contribution_target_provider_degraded",
            message: format!("target provider `{provider}` is degraded"),
            severity: ContributionAssemblySeverity::Warning,
            skip: false,
        }),
        _ => None,
    }
}

fn provider_is_enabled(provider: &str, policy: &ContributionAssemblyPolicy) -> bool {
    policy.enabled_providers.is_empty() || policy.enabled_providers.contains(provider)
}

fn provider_is_degraded(provider: &str, policy: &ContributionAssemblyPolicy) -> bool {
    policy.allow_degraded_providers
        && policy.provider_health.get(provider) == Some(&ContributionProviderHealth::Degraded)
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

fn remove_missing_dependencies(
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

fn dependency_order(
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

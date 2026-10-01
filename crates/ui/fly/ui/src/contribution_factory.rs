use crate::{ContributionDescriptor, ContributionRegistry, UiError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContributionSurface {
    Admin,
    Storefront,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContributionProviderHealth {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleContributionMetadata {
    pub module_id: String,
    pub provider: String,
    #[serde(default)]
    pub dependencies: BTreeSet<String>,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub admin: Vec<ContributionDescriptor>,
    #[serde(default)]
    pub storefront: Vec<ContributionDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContributionAssemblyPolicy {
    /// Empty means every discovered module is tenant-enabled.
    #[serde(default)]
    pub enabled_modules: BTreeSet<String>,
    /// Empty means every discovered provider is policy-enabled.
    #[serde(default)]
    pub enabled_providers: BTreeSet<String>,
    /// Empty means all non-denied contribution ids are allowed.
    #[serde(default)]
    pub allowed_contributions: BTreeSet<String>,
    #[serde(default)]
    pub denied_contributions: BTreeSet<String>,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    #[serde(default)]
    pub permissions: BTreeSet<String>,
    #[serde(default)]
    pub provider_health: BTreeMap<String, ContributionProviderHealth>,
    #[serde(default)]
    pub allow_degraded_providers: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContributionAssemblySeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContributionAssemblyDiagnostic {
    pub severity: ContributionAssemblySeverity,
    pub code: String,
    pub module_id: Option<String>,
    pub contribution_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContributionAssemblyResult {
    pub registry: ContributionRegistry,
    pub diagnostics: Vec<ContributionAssemblyDiagnostic>,
    pub registered_contributions: usize,
    pub skipped_contributions: usize,
}

impl ContributionAssemblyResult {
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ContributionAssemblySeverity::Error)
    }
}

pub fn build_admin_contribution_registry(
    modules: impl IntoIterator<Item = ModuleContributionMetadata>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    assemble_contribution_registry(ContributionSurface::Admin, modules, policy)
}

pub fn build_storefront_contribution_registry(
    modules: impl IntoIterator<Item = ModuleContributionMetadata>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    assemble_contribution_registry(ContributionSurface::Storefront, modules, policy)
}

pub fn assemble_contribution_registry(
    surface: ContributionSurface,
    modules: impl IntoIterator<Item = ModuleContributionMetadata>,
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    let mut result = ContributionAssemblyResult::default();
    let mut discovered = BTreeMap::new();

    for module in modules {
        let module = match normalize_module(module) {
            Ok(module) => module,
            Err(message) => {
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Error,
                    "contribution_module_invalid",
                    None,
                    None,
                    message,
                ));
                continue;
            }
        };
        if discovered.contains_key(&module.module_id) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&module, surface).len());
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Error,
                "contribution_module_duplicate",
                Some(&module.module_id),
                None,
                format!("module metadata `{}` is duplicated", module.module_id),
            ));
            continue;
        }
        discovered.insert(module.module_id.clone(), module);
    }

    let mut selected = BTreeMap::new();
    for (module_id, module) in discovered {
        if let Some((code, message, severity)) = module_filter(&module, policy) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&module, surface).len());
            result.diagnostics.push(diagnostic(
                severity,
                code,
                Some(&module.module_id),
                None,
                message,
            ));
            continue;
        }
        if policy
            .provider_health
            .get(&module.provider)
            .is_some_and(|health| {
                *health == ContributionProviderHealth::Degraded && policy.allow_degraded_providers
            })
        {
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Warning,
                "contribution_provider_degraded",
                Some(&module.module_id),
                None,
                format!("provider `{}` is degraded", module.provider),
            ));
        }
        selected.insert(module_id, module);
    }

    remove_missing_dependencies(&mut selected, surface, &mut result);
    let order = dependency_order(&selected, surface, &mut result);

    for module_id in order {
        let Some(module) = selected.get(&module_id) else {
            continue;
        };
        for contribution in surface_contributions(module, surface) {
            if let Some((code, message)) = contribution_filter(contribution, policy) {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Info,
                    code,
                    Some(&module.module_id),
                    Some(contribution.id.trim()),
                    message,
                ));
                continue;
            }
            if contribution.provider.trim() != module.provider {
                result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                result.diagnostics.push(diagnostic(
                    ContributionAssemblySeverity::Error,
                    "contribution_provider_metadata_mismatch",
                    Some(&module.module_id),
                    Some(contribution.id.trim()),
                    format!(
                        "contribution provider `{}` does not match module `{}`",
                        contribution.provider.trim(),
                        module.provider
                    ),
                ));
                continue;
            }
            match result.registry.register(contribution.clone()) {
                Ok(()) => {
                    result.registered_contributions =
                        result.registered_contributions.saturating_add(1);
                }
                Err(error) => {
                    result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                    result.diagnostics.push(registration_diagnostic(
                        &module.module_id,
                        contribution.id.trim(),
                        error,
                    ));
                }
            }
        }
    }

    result
}

fn module_filter(
    module: &ModuleContributionMetadata,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String, ContributionAssemblySeverity)> {
    if !policy.enabled_modules.is_empty() && !policy.enabled_modules.contains(&module.module_id) {
        return Some((
            "contribution_module_tenant_disabled",
            "module is disabled for the tenant".to_string(),
            ContributionAssemblySeverity::Info,
        ));
    }
    if !policy.enabled_providers.is_empty() && !policy.enabled_providers.contains(&module.provider)
    {
        return Some((
            "contribution_provider_policy_disabled",
            "provider is disabled by policy".to_string(),
            ContributionAssemblySeverity::Info,
        ));
    }
    let missing_permissions = module
        .required_permissions
        .difference(&policy.permissions)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_permissions.is_empty() {
        return Some((
            "contribution_permission_missing",
            format!(
                "required permissions are missing: {}",
                missing_permissions.join(", ")
            ),
            ContributionAssemblySeverity::Info,
        ));
    }
    match policy
        .provider_health
        .get(&module.provider)
        .copied()
        .unwrap_or(ContributionProviderHealth::Healthy)
    {
        ContributionProviderHealth::Healthy => None,
        ContributionProviderHealth::Degraded if policy.allow_degraded_providers => None,
        ContributionProviderHealth::Degraded => Some((
            "contribution_provider_degraded_blocked",
            "degraded provider is blocked by policy".to_string(),
            ContributionAssemblySeverity::Info,
        )),
        ContributionProviderHealth::Unavailable => Some((
            "contribution_provider_unavailable",
            "provider is unavailable".to_string(),
            ContributionAssemblySeverity::Info,
        )),
    }
}

fn contribution_filter(
    contribution: &ContributionDescriptor,
    policy: &ContributionAssemblyPolicy,
) -> Option<(&'static str, String)> {
    let id = contribution.id.trim();
    if !policy.allowed_contributions.is_empty() && !policy.allowed_contributions.contains(id) {
        return Some((
            "contribution_not_allowlisted",
            "contribution is not allowlisted".to_string(),
        ));
    }
    if policy.denied_contributions.contains(id) {
        return Some((
            "contribution_denied",
            "contribution is denied by policy".to_string(),
        ));
    }
    let missing = contribution
        .required_capabilities
        .difference(&policy.capabilities)
        .cloned()
        .collect::<Vec<_>>();
    (!missing.is_empty()).then(|| {
        (
            "contribution_capability_missing",
            format!("required capabilities are missing: {}", missing.join(", ")),
        )
    })
}

fn normalize_module(
    mut module: ModuleContributionMetadata,
) -> Result<ModuleContributionMetadata, String> {
    module.module_id = required(&module.module_id, "module_id")?;
    module.provider = required(&module.provider, "provider")?;
    module.dependencies = normalize_set(module.dependencies, "dependency")?;
    module.required_permissions =
        normalize_set(module.required_permissions, "required permission")?;
    Ok(module)
}

fn normalize_set(values: BTreeSet<String>, label: &str) -> Result<BTreeSet<String>, String> {
    values
        .into_iter()
        .map(|value| required(&value, label))
        .collect()
}

fn required(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn surface_contributions(
    module: &ModuleContributionMetadata,
    surface: ContributionSurface,
) -> &[ContributionDescriptor] {
    match surface {
        ContributionSurface::Admin => &module.admin,
        ContributionSurface::Storefront => &module.storefront,
    }
}

fn remove_missing_dependencies(
    selected: &mut BTreeMap<String, ModuleContributionMetadata>,
    surface: ContributionSurface,
    result: &mut ContributionAssemblyResult,
) {
    loop {
        let missing = selected
            .iter()
            .filter_map(|(module_id, module)| {
                let dependencies = module
                    .dependencies
                    .iter()
                    .filter(|dependency| !selected.contains_key(*dependency))
                    .cloned()
                    .collect::<Vec<_>>();
                (!dependencies.is_empty()).then(|| (module_id.clone(), dependencies))
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return;
        }
        for (module_id, dependencies) in missing {
            let Some(module) = selected.remove(&module_id) else {
                continue;
            };
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(&module, surface).len());
            result.diagnostics.push(diagnostic(
                ContributionAssemblySeverity::Error,
                "contribution_dependency_missing",
                Some(&module_id),
                None,
                format!(
                    "required modules are missing or filtered: {}",
                    dependencies.join(", ")
                ),
            ));
        }
    }
}

fn dependency_order(
    selected: &BTreeMap<String, ModuleContributionMetadata>,
    surface: ContributionSurface,
    result: &mut ContributionAssemblyResult,
) -> Vec<String> {
    let mut indegree = selected
        .iter()
        .map(|(module_id, module)| (module_id.clone(), module.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, BTreeSet<String>>::new();
    for (module_id, module) in selected {
        for dependency in &module.dependencies {
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
        if let Some(module) = selected.get(&module_id) {
            result.skipped_contributions = result
                .skipped_contributions
                .saturating_add(surface_contributions(module, surface).len());
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
    use crate::{
        AccessibilityMetadata, Presentation, PropertyEditorDescriptor, RendererDescriptor,
    };
    use serde_json::{Map, json};

    fn accessibility(id: &str) -> AccessibilityMetadata {
        AccessibilityMetadata {
            label_message_id: id.to_string(),
            description_message_id: None,
            keyboard_hint_message_id: None,
        }
    }

    fn contribution(id: &str, provider: &str, capability: Option<&str>) -> ContributionDescriptor {
        ContributionDescriptor {
            id: id.to_string(),
            provider: provider.to_string(),
            required_capabilities: capability
                .map(|value| BTreeSet::from([value.to_string()]))
                .unwrap_or_default(),
            blocks: vec![format!("{id}.block")],
            renderers: vec![RendererDescriptor {
                id: format!("{id}.renderer"),
                component_type: id.to_string(),
                provider: provider.to_string(),
                presentations: BTreeSet::from([Presentation::Full]),
                accessibility: accessibility(&format!("{id}.label")),
            }],
            property_editors: vec![PropertyEditorDescriptor {
                id: format!("{id}.properties"),
                component_type: id.to_string(),
                provider: provider.to_string(),
                property_schema: json!({ "type": "object" }),
                accessibility: accessibility(&format!("{id}.properties.label")),
            }],
            messages: BTreeMap::new(),
            metadata: Map::new(),
        }
    }

    fn module(
        module_id: &str,
        provider: &str,
        dependencies: &[&str],
        admin: Vec<ContributionDescriptor>,
        storefront: Vec<ContributionDescriptor>,
    ) -> ModuleContributionMetadata {
        ModuleContributionMetadata {
            module_id: module_id.to_string(),
            provider: provider.to_string(),
            dependencies: dependencies
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            required_permissions: BTreeSet::new(),
            admin,
            storefront,
        }
    }

    #[test]
    fn admin_and_storefront_factories_are_separate() {
        let modules = vec![module(
            "pages",
            "rustok.pages",
            &[],
            vec![contribution("pages.admin", "rustok.pages", None)],
            vec![contribution("pages.storefront", "rustok.pages", None)],
        )];
        let admin = build_admin_contribution_registry(
            modules.clone(),
            &ContributionAssemblyPolicy::default(),
        );
        let storefront =
            build_storefront_contribution_registry(modules, &ContributionAssemblyPolicy::default());
        assert!(admin.registry.get("pages.admin").is_some());
        assert!(admin.registry.get("pages.storefront").is_none());
        assert!(storefront.registry.get("pages.storefront").is_some());
        assert!(storefront.registry.get("pages.admin").is_none());
    }

    #[test]
    fn assembly_filters_tenant_permissions_capabilities_and_health() {
        let mut pages = module(
            "pages",
            "rustok.pages",
            &[],
            vec![contribution(
                "pages.admin",
                "rustok.pages",
                Some("pages.read"),
            )],
            Vec::new(),
        );
        pages
            .required_permissions
            .insert("pages.manage".to_string());
        let result = build_admin_contribution_registry(
            [pages],
            &ContributionAssemblyPolicy {
                enabled_modules: BTreeSet::from(["pages".to_string()]),
                enabled_providers: BTreeSet::from(["rustok.pages".to_string()]),
                capabilities: BTreeSet::from(["pages.read".to_string()]),
                permissions: BTreeSet::from(["pages.manage".to_string()]),
                provider_health: BTreeMap::from([(
                    "rustok.pages".to_string(),
                    ContributionProviderHealth::Healthy,
                )]),
                ..ContributionAssemblyPolicy::default()
            },
        );
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 1);

        let unavailable = build_admin_contribution_registry(
            [module(
                "pages",
                "rustok.pages",
                &[],
                vec![contribution("pages.admin", "rustok.pages", None)],
                Vec::new(),
            )],
            &ContributionAssemblyPolicy {
                provider_health: BTreeMap::from([(
                    "rustok.pages".to_string(),
                    ContributionProviderHealth::Unavailable,
                )]),
                ..ContributionAssemblyPolicy::default()
            },
        );
        assert!(unavailable.registry.is_empty());
        assert_eq!(unavailable.skipped_contributions, 1);
    }

    #[test]
    fn missing_dependencies_and_cycles_are_diagnosed() {
        let missing = build_admin_contribution_registry(
            [module(
                "blog",
                "rustok.blog",
                &["media"],
                vec![contribution("blog.admin", "rustok.blog", None)],
                Vec::new(),
            )],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(!missing.is_valid());
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "contribution_dependency_missing" })
        );

        let cycle = build_admin_contribution_registry(
            [
                module(
                    "a",
                    "provider.a",
                    &["b"],
                    vec![contribution("a.admin", "provider.a", None)],
                    Vec::new(),
                ),
                module(
                    "b",
                    "provider.b",
                    &["a"],
                    vec![contribution("b.admin", "provider.b", None)],
                    Vec::new(),
                ),
            ],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(!cycle.is_valid());
        assert_eq!(
            cycle
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "contribution_dependency_cycle")
                .count(),
            2
        );
    }

    #[test]
    fn duplicate_nested_contracts_are_reported_without_partial_registration() {
        let first = contribution("shared", "provider.shared", None);
        let mut second = contribution("alternative", "provider.shared", None);
        second.renderers[0].component_type = "shared".to_string();
        second.property_editors[0].component_type = "other".to_string();
        let result = build_admin_contribution_registry(
            [module(
                "shared",
                "provider.shared",
                &[],
                vec![first, second],
                Vec::new(),
            )],
            &ContributionAssemblyPolicy::default(),
        );
        assert_eq!(result.registered_contributions, 1);
        assert_eq!(result.skipped_contributions, 1);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "contribution_renderer_duplicate" })
        );
    }

    #[test]
    fn dependency_order_is_deterministic() {
        let result = build_admin_contribution_registry(
            [
                module(
                    "blog",
                    "rustok.blog",
                    &["media"],
                    vec![contribution("blog.admin", "rustok.blog", None)],
                    Vec::new(),
                ),
                module(
                    "media",
                    "rustok.media",
                    &[],
                    vec![contribution("media.admin", "rustok.media", None)],
                    Vec::new(),
                ),
            ],
            &ContributionAssemblyPolicy::default(),
        );
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 2);
        assert_eq!(
            result.registry.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec!["blog.admin", "media.admin"]
        );
    }
}

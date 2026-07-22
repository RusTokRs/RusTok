use super::*;
use crate::{ContributionAssemblyPolicy, ContributionDescriptor, ContributionProviderHealth};
use serde_json::Map;
use std::collections::{BTreeMap, BTreeSet};

fn contribution(id: &str, provider: &str) -> ContributionDescriptor {
    ContributionDescriptor {
        id: id.to_string(),
        provider: provider.to_string(),
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
        target_providers: BTreeSet::new(),
        dependencies: BTreeSet::new(),
        required_permissions: BTreeSet::new(),
        admin,
        storefront: Vec::new(),
    }
}

fn cross_provider_manifest() -> ModuleContributionManifest {
    let mut manifest = manifest(vec![contribution("pages.blocks", "fly.builtin")]);
    manifest.target_providers.insert("fly.builtin".to_string());
    manifest
}

#[test]
fn owner_provider_is_the_only_implicit_target() {
    let result = build_admin_contribution_registry_from_manifests(
        [manifest(vec![contribution("pages.blocks", "fly.builtin")])],
        &ContributionAssemblyPolicy::default(),
    );
    assert!(!result.is_valid());
    assert!(result.registry.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "contribution_target_provider_forbidden" })
    );
}

#[test]
fn explicit_target_provider_is_allowed() {
    let result = build_admin_contribution_registry_from_manifests(
        [cross_provider_manifest()],
        &ContributionAssemblyPolicy::default(),
    );
    assert!(result.is_valid());
    assert_eq!(result.registered_contributions, 1);
    assert!(result.registry.get("pages.blocks").is_some());
}

#[test]
fn target_provider_must_be_tenant_enabled() {
    let result = build_admin_contribution_registry_from_manifests(
        [cross_provider_manifest()],
        &ContributionAssemblyPolicy {
            enabled_providers: BTreeSet::from(["rustok.pages".to_string()]),
            ..ContributionAssemblyPolicy::default()
        },
    );
    assert!(result.is_valid());
    assert!(result.registry.is_empty());
    assert_eq!(result.skipped_contributions, 1);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "contribution_target_provider_disabled" })
    );
}

#[test]
fn owner_and_target_provider_allowlist_enables_cross_provider_extension() {
    let result = build_admin_contribution_registry_from_manifests(
        [cross_provider_manifest()],
        &ContributionAssemblyPolicy {
            enabled_providers: BTreeSet::from([
                "rustok.pages".to_string(),
                "fly.builtin".to_string(),
            ]),
            ..ContributionAssemblyPolicy::default()
        },
    );
    assert_eq!(result.registered_contributions, 1);
    assert!(result.registry.get("pages.blocks").is_some());
}

#[test]
fn admin_and_storefront_surfaces_remain_separate() {
    let mut manifest = manifest(vec![contribution("pages.admin.blocks", "rustok.pages")]);
    manifest.storefront = vec![contribution("pages.storefront.renderer", "rustok.pages")];
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
    assert!(
        storefront
            .registry
            .get("pages.storefront.renderer")
            .is_some()
    );
    assert!(storefront.registry.get("pages.admin.blocks").is_none());
}

#[test]
fn target_provider_health_can_block_cross_provider_extensions() {
    let result = build_admin_contribution_registry_from_manifests(
        [cross_provider_manifest()],
        &ContributionAssemblyPolicy {
            provider_health: BTreeMap::from([(
                "fly.builtin".to_string(),
                ContributionProviderHealth::Unavailable,
            )]),
            ..ContributionAssemblyPolicy::default()
        },
    );
    assert!(result.registry.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "contribution_target_provider_unavailable" })
    );
}

#[test]
fn direct_target_lookup_trims_provider_names() {
    let manifest = ModuleContributionManifest {
        module_id: "pages".to_string(),
        owner_provider: " rustok.pages ".to_string(),
        target_providers: BTreeSet::from(["fly.builtin".to_string()]),
        dependencies: BTreeSet::new(),
        required_permissions: BTreeSet::new(),
        admin: Vec::new(),
        storefront: Vec::new(),
    };
    assert!(manifest.allows_target_provider("rustok.pages"));
    assert!(manifest.allows_target_provider("fly.builtin"));
}

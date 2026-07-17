use crate::ContributionAssemblyResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Effective access to palette block ids for one assembled editor surface.
///
/// Without a contribution assembly the policy is deliberately permissive for backwards
/// compatibility. With an assembly, unnamespaced Fly primitives remain available while namespaced
/// templates/plugins must be declared by an active contribution. This makes tenant, permission,
/// capability and provider-health filtering effective for both visible controls and remote intents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaletteBlockAccess {
    restrict_namespaced_blocks: bool,
    contributed_blocks: BTreeMap<String, BTreeSet<String>>,
}

impl Default for PaletteBlockAccess {
    fn default() -> Self {
        Self::unrestricted()
    }
}

impl PaletteBlockAccess {
    pub fn unrestricted() -> Self {
        Self {
            restrict_namespaced_blocks: false,
            contributed_blocks: BTreeMap::new(),
        }
    }

    pub fn from_assembly(assembly: &ContributionAssemblyResult) -> Self {
        let mut contributed_blocks = BTreeMap::<String, BTreeSet<String>>::new();
        for (contribution_id, contribution) in assembly.registry.iter() {
            for block_id in &contribution.blocks {
                contributed_blocks
                    .entry(block_id.clone())
                    .or_default()
                    .insert(contribution_id.to_string());
            }
        }
        Self {
            restrict_namespaced_blocks: true,
            contributed_blocks,
        }
    }

    pub fn from_optional_assembly(assembly: Option<&ContributionAssemblyResult>) -> Self {
        assembly
            .map(Self::from_assembly)
            .unwrap_or_else(Self::unrestricted)
    }

    pub fn restricts_namespaced_blocks(&self) -> bool {
        self.restrict_namespaced_blocks
    }

    pub fn allows(&self, block_id: &str) -> bool {
        let block_id = block_id.trim();
        if block_id.is_empty() {
            return false;
        }
        !self.restrict_namespaced_blocks
            || !is_namespaced_block(block_id)
            || self.contributed_blocks.contains_key(block_id)
    }

    pub fn contribution_ids(&self, block_id: &str) -> impl Iterator<Item = &str> {
        self.contributed_blocks
            .get(block_id.trim())
            .into_iter()
            .flat_map(|ids| ids.iter().map(String::as_str))
    }

    pub fn contributed_block_ids(&self) -> impl Iterator<Item = &str> {
        self.contributed_blocks.keys().map(String::as_str)
    }
}

fn is_namespaced_block(block_id: &str) -> bool {
    block_id.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContributionDescriptor, ContributionRegistry};
    use serde_json::Map;

    fn assembly(blocks: &[&str]) -> ContributionAssemblyResult {
        let mut registry = ContributionRegistry::default();
        registry
            .register(ContributionDescriptor {
                id: "pages.blocks".to_string(),
                provider: "fly.builtin".to_string(),
                provider_version: "1".to_string(),
                required_capabilities: BTreeSet::new(),
                blocks: blocks.iter().map(|block| (*block).to_string()).collect(),
                renderers: Vec::new(),
                property_editors: Vec::new(),
                messages: BTreeMap::new(),
                metadata: Map::new(),
            })
            .expect("registry");
        ContributionAssemblyResult {
            registry,
            registered_contributions: 1,
            ..ContributionAssemblyResult::default()
        }
    }

    #[test]
    fn legacy_surface_is_unrestricted() {
        let access = PaletteBlockAccess::unrestricted();
        assert!(access.allows("text"));
        assert!(access.allows("fly.hero"));
        assert!(!access.allows("  "));
    }

    #[test]
    fn assembled_surface_keeps_primitives_and_filters_namespaced_blocks() {
        let assembly = assembly(&["fly.hero"]);
        let access = PaletteBlockAccess::from_assembly(&assembly);
        assert!(access.allows("text"));
        assert!(access.allows("fly.hero"));
        assert!(!access.allows("fly.cta"));
        assert!(!access.allows("plugin.secret"));
    }

    #[test]
    fn block_provenance_is_deterministic() {
        let assembly = assembly(&["fly.hero"]);
        let access = PaletteBlockAccess::from_assembly(&assembly);
        assert_eq!(
            access.contribution_ids("fly.hero").collect::<Vec<_>>(),
            vec!["pages.blocks"]
        );
    }
}

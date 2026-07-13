use crate::{Presentation, UiError, UiResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibilityMetadata {
    pub label_message_id: String,
    pub description_message_id: Option<String>,
    pub keyboard_hint_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RendererDescriptor {
    pub id: String,
    pub component_type: String,
    pub provider: String,
    pub schema_version: String,
    pub presentations: BTreeSet<Presentation>,
    pub accessibility: AccessibilityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyEditorDescriptor {
    pub id: String,
    pub component_type: String,
    pub provider: String,
    pub schema_version: String,
    pub property_schema: Value,
    pub accessibility: AccessibilityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContributionDescriptor {
    pub id: String,
    pub provider: String,
    pub provider_version: String,
    #[serde(default)]
    pub required_capabilities: BTreeSet<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub renderers: Vec<RendererDescriptor>,
    #[serde(default)]
    pub property_editors: Vec<PropertyEditorDescriptor>,
    #[serde(default)]
    pub messages: BTreeMap<String, String>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContributionRegistry {
    contributions: BTreeMap<String, ContributionDescriptor>,
}

impl ContributionRegistry {
    pub fn register(&mut self, contribution: ContributionDescriptor) -> UiResult<()> {
        if self.contributions.contains_key(&contribution.id) {
            return Err(UiError::DuplicateContribution(contribution.id));
        }
        if contribution.provider.is_empty() {
            return Err(UiError::MissingContributionProvider(contribution.id));
        }
        self.contributions
            .insert(contribution.id.clone(), contribution);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ContributionDescriptor> {
        self.contributions.get(id)
    }

    pub fn available<'a>(
        &'a self,
        capabilities: &'a BTreeSet<String>,
    ) -> impl Iterator<Item = &'a ContributionDescriptor> + 'a {
        self.contributions.values().filter(move |contribution| {
            contribution
                .required_capabilities
                .iter()
                .all(|capability| capabilities.contains(capability))
        })
    }
}

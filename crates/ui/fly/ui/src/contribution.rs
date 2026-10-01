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
    pub presentations: BTreeSet<Presentation>,
    pub accessibility: AccessibilityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyEditorDescriptor {
    pub id: String,
    pub component_type: String,
    pub provider: String,
    pub property_schema: Value,
    pub accessibility: AccessibilityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContributionDescriptor {
    pub id: String,
    pub provider: String,
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

#[derive(Debug, Clone, Copy)]
pub struct ResolvedRenderer<'a> {
    pub contribution: &'a ContributionDescriptor,
    pub renderer: &'a RendererDescriptor,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedPropertyEditor<'a> {
    pub contribution: &'a ContributionDescriptor,
    pub property_editor: &'a PropertyEditorDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContributionRegistry {
    contributions: BTreeMap<String, ContributionDescriptor>,
}

impl ContributionRegistry {
    pub fn register(&mut self, contribution: ContributionDescriptor) -> UiResult<()> {
        let contribution = normalize_contribution(contribution)?;
        if self.contributions.contains_key(&contribution.id) {
            return Err(UiError::DuplicateContribution(contribution.id));
        }
        self.validate_renderer_conflicts(&contribution)?;
        self.validate_property_editor_conflicts(&contribution)?;
        self.contributions
            .insert(contribution.id.clone(), contribution);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ContributionDescriptor> {
        self.contributions.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &ContributionDescriptor)> {
        self.contributions
            .iter()
            .map(|(id, contribution)| (id.as_str(), contribution))
    }

    pub fn len(&self) -> usize {
        self.contributions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }

    pub fn available<'a>(
        &'a self,
        capabilities: &'a BTreeSet<String>,
    ) -> impl Iterator<Item = &'a ContributionDescriptor> + 'a {
        self.contributions
            .values()
            .filter(move |contribution| contribution_is_available(contribution, capabilities))
    }

    pub fn resolve_renderer<'a>(
        &'a self,
        provider: &str,
        component_type: &str,
        presentation: Presentation,
        capabilities: &BTreeSet<String>,
    ) -> Option<ResolvedRenderer<'a>> {
        let provider = provider.trim();
        let component_type = component_type.trim();
        self.contributions.values().find_map(|contribution| {
            if !contribution_is_available(contribution, capabilities) {
                return None;
            }
            contribution
                .renderers
                .iter()
                .find(|renderer| {
                    renderer.provider == provider
                        && renderer.component_type == component_type
                        && renderer.presentations.contains(&presentation)
                })
                .map(|renderer| ResolvedRenderer {
                    contribution,
                    renderer,
                })
        })
    }

    pub fn resolve_property_editor<'a>(
        &'a self,
        provider: &str,
        component_type: &str,
        capabilities: &BTreeSet<String>,
    ) -> Option<ResolvedPropertyEditor<'a>> {
        let provider = provider.trim();
        let component_type = component_type.trim();
        self.contributions.values().find_map(|contribution| {
            if !contribution_is_available(contribution, capabilities) {
                return None;
            }
            contribution
                .property_editors
                .iter()
                .find(|editor| {
                    editor.provider == provider && editor.component_type == component_type
                })
                .map(|property_editor| ResolvedPropertyEditor {
                    contribution,
                    property_editor,
                })
        })
    }

    fn validate_renderer_conflicts(&self, candidate: &ContributionDescriptor) -> UiResult<()> {
        for renderer in &candidate.renderers {
            for existing in self.contributions.values() {
                for registered in &existing.renderers {
                    if registered.id == renderer.id {
                        return Err(UiError::DuplicateRenderer(renderer.id.clone()));
                    }
                    for presentation in &renderer.presentations {
                        if registered.provider == renderer.provider
                            && registered.component_type == renderer.component_type
                            && registered.presentations.contains(presentation)
                        {
                            return Err(UiError::DuplicateRenderer(renderer_contract_id(
                                renderer,
                                *presentation,
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_property_editor_conflicts(
        &self,
        candidate: &ContributionDescriptor,
    ) -> UiResult<()> {
        for editor in &candidate.property_editors {
            for existing in self.contributions.values() {
                for registered in &existing.property_editors {
                    if registered.id == editor.id
                        || (registered.provider == editor.provider
                            && registered.component_type == editor.component_type)
                    {
                        return Err(UiError::DuplicatePropertyEditor(
                            property_editor_contract_id(editor),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn normalize_contribution(
    mut contribution: ContributionDescriptor,
) -> UiResult<ContributionDescriptor> {
    let error_id = contribution_id_for_error(&contribution.id);
    contribution.id = required_value(&contribution.id, &error_id, "id")?;
    contribution.provider = contribution.provider.trim().to_string();
    if contribution.provider.is_empty() {
        return Err(UiError::MissingContributionProvider(contribution.id));
    }
    contribution.required_capabilities =
        normalize_capabilities(contribution.required_capabilities, &contribution.id)?;
    contribution.blocks =
        normalize_unique_values(contribution.blocks, &contribution.id, "block id")?;
    contribution.messages = normalize_messages(contribution.messages, &contribution.id)?;

    let mut renderer_ids = BTreeSet::new();
    let mut renderer_contracts = BTreeSet::new();
    for renderer in &mut contribution.renderers {
        renderer.id = required_value(&renderer.id, &contribution.id, "renderer id")?;
        renderer.component_type = required_value(
            &renderer.component_type,
            &contribution.id,
            "renderer component_type",
        )?;
        renderer.provider =
            required_value(&renderer.provider, &contribution.id, "renderer provider")?;
        if renderer.provider != contribution.provider {
            return invalid_contribution(
                &contribution.id,
                format!(
                    "renderer `{}` belongs to provider `{}`, expected `{}`",
                    renderer.id, renderer.provider, contribution.provider
                ),
            );
        }
        if renderer.presentations.is_empty() {
            return invalid_contribution(
                &contribution.id,
                format!("renderer `{}` has no presentation", renderer.id),
            );
        }
        normalize_accessibility(
            &mut renderer.accessibility,
            &contribution.id,
            &format!("renderer `{}`", renderer.id),
        )?;
        if !renderer_ids.insert(renderer.id.clone()) {
            return Err(UiError::DuplicateRenderer(renderer.id.clone()));
        }
        for presentation in &renderer.presentations {
            let contract = renderer_contract_id(renderer, *presentation);
            if !renderer_contracts.insert(contract.clone()) {
                return Err(UiError::DuplicateRenderer(contract));
            }
        }
    }

    let mut editor_ids = BTreeSet::new();
    let mut editor_contracts = BTreeSet::new();
    for editor in &mut contribution.property_editors {
        editor.id = required_value(&editor.id, &contribution.id, "property editor id")?;
        editor.component_type = required_value(
            &editor.component_type,
            &contribution.id,
            "property editor component_type",
        )?;
        editor.provider = required_value(
            &editor.provider,
            &contribution.id,
            "property editor provider",
        )?;
        if editor.provider != contribution.provider {
            return invalid_contribution(
                &contribution.id,
                format!(
                    "property editor `{}` belongs to provider `{}`, expected `{}`",
                    editor.id, editor.provider, contribution.provider
                ),
            );
        }
        normalize_accessibility(
            &mut editor.accessibility,
            &contribution.id,
            &format!("property editor `{}`", editor.id),
        )?;
        if !editor_ids.insert(editor.id.clone()) {
            return Err(UiError::DuplicatePropertyEditor(editor.id.clone()));
        }
        let contract = property_editor_contract_id(editor);
        if !editor_contracts.insert(contract.clone()) {
            return Err(UiError::DuplicatePropertyEditor(contract));
        }
    }

    Ok(contribution)
}

fn normalize_capabilities(
    capabilities: BTreeSet<String>,
    contribution_id: &str,
) -> UiResult<BTreeSet<String>> {
    capabilities
        .into_iter()
        .map(|capability| required_value(&capability, contribution_id, "required capability"))
        .collect()
}

fn normalize_unique_values(
    values: Vec<String>,
    contribution_id: &str,
    label: &str,
) -> UiResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let value = required_value(&value, contribution_id, label)?;
        if !seen.insert(value.clone()) {
            return invalid_contribution(contribution_id, format!("duplicate {label} `{value}`"));
        }
        normalized.push(value);
    }
    Ok(normalized)
}

fn normalize_messages(
    messages: BTreeMap<String, String>,
    contribution_id: &str,
) -> UiResult<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();
    for (message_id, message) in messages {
        let message_id = required_value(&message_id, contribution_id, "message id")?;
        let message = required_value(&message, contribution_id, "message")?;
        if normalized.insert(message_id.clone(), message).is_some() {
            return invalid_contribution(
                contribution_id,
                format!("duplicate message id `{message_id}` after normalization"),
            );
        }
    }
    Ok(normalized)
}

fn normalize_accessibility(
    accessibility: &mut AccessibilityMetadata,
    contribution_id: &str,
    owner: &str,
) -> UiResult<()> {
    accessibility.label_message_id = required_value(
        &accessibility.label_message_id,
        contribution_id,
        &format!("{owner} accessibility label_message_id"),
    )?;
    accessibility.description_message_id =
        normalize_optional(accessibility.description_message_id.take());
    accessibility.keyboard_hint_message_id =
        normalize_optional(accessibility.keyboard_hint_message_id.take());
    Ok(())
}

fn contribution_is_available(
    contribution: &ContributionDescriptor,
    capabilities: &BTreeSet<String>,
) -> bool {
    contribution
        .required_capabilities
        .iter()
        .all(|capability| capabilities.contains(capability))
}

fn renderer_contract_id(renderer: &RendererDescriptor, presentation: Presentation) -> String {
    format!(
        "{}:{}:{}",
        renderer.provider,
        renderer.component_type,
        presentation.as_str()
    )
}

fn property_editor_contract_id(editor: &PropertyEditorDescriptor) -> String {
    format!("{}:{}", editor.provider, editor.component_type)
}

fn required_value(value: &str, contribution_id: &str, label: &str) -> UiResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return invalid_contribution(contribution_id, format!("{label} must not be empty"));
    }
    Ok(value.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn contribution_id_for_error(id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        "<unnamed>".to_string()
    } else {
        id.to_string()
    }
}

fn invalid_contribution<T>(contribution: &str, message: impl Into<String>) -> UiResult<T> {
    Err(UiError::InvalidContribution {
        contribution: contribution.to_string(),
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn accessibility(label: &str) -> AccessibilityMetadata {
        AccessibilityMetadata {
            label_message_id: label.to_string(),
            description_message_id: None,
            keyboard_hint_message_id: None,
        }
    }

    fn contribution(id: &str, capability: Option<&str>) -> ContributionDescriptor {
        ContributionDescriptor {
            id: id.to_string(),
            provider: "rustok.pages".to_string(),
            required_capabilities: capability
                .map(|capability| BTreeSet::from([capability.to_string()]))
                .unwrap_or_default(),
            blocks: vec![format!("{id}.hero")],
            renderers: vec![RendererDescriptor {
                id: format!("{id}.renderer"),
                component_type: "hero".to_string(),
                provider: "rustok.pages".to_string(),
                presentations: BTreeSet::from([Presentation::Full, Presentation::Preview]),
                accessibility: accessibility("hero.label"),
            }],
            property_editors: vec![PropertyEditorDescriptor {
                id: format!("{id}.properties"),
                component_type: "hero".to_string(),
                provider: "rustok.pages".to_string(),
                property_schema: json!({ "type": "object" }),
                accessibility: accessibility("hero.properties.label"),
            }],
            messages: BTreeMap::from([
                ("hero.label".to_string(), "Hero".to_string()),
                (
                    "hero.properties.label".to_string(),
                    "Hero properties".to_string(),
                ),
            ]),
            metadata: Map::new(),
        }
    }

    #[test]
    fn registry_resolves_capability_gated_renderer_and_property_editor() {
        let mut registry = ContributionRegistry::default();
        registry
            .register(contribution("rustok.pages.hero", Some("pages.read")))
            .expect("register");
        let capabilities = BTreeSet::from(["pages.read".to_string()]);
        let renderer = registry
            .resolve_renderer("rustok.pages", "hero", Presentation::Preview, &capabilities)
            .expect("renderer");
        assert_eq!(renderer.contribution.id, "rustok.pages.hero");
        assert_eq!(renderer.renderer.id, "rustok.pages.hero.renderer");
        let editor = registry
            .resolve_property_editor("rustok.pages", "hero", &capabilities)
            .expect("property editor");
        assert_eq!(editor.property_editor.id, "rustok.pages.hero.properties");
        assert!(
            registry
                .resolve_renderer("rustok.pages", "hero", Presentation::Inline, &capabilities,)
                .is_none()
        );
        assert!(
            registry
                .resolve_renderer("rustok.pages", "hero", Presentation::Full, &BTreeSet::new(),)
                .is_none()
        );
    }

    #[test]
    fn duplicate_renderer_contract_is_rejected_atomically() {
        let mut registry = ContributionRegistry::default();
        registry
            .register(contribution("rustok.pages.hero", None))
            .expect("first");
        let mut duplicate = contribution("rustok.pages.hero.alternative", None);
        duplicate.renderers[0].id = "alternative.renderer".to_string();
        duplicate.property_editors.clear();
        let error = registry
            .register(duplicate)
            .expect_err("duplicate renderer");
        assert!(matches!(error, UiError::DuplicateRenderer(_)));
        assert_eq!(registry.len(), 1);
        assert!(registry.get("rustok.pages.hero.alternative").is_none());
    }

    #[test]
    fn duplicate_property_editor_contract_is_rejected_atomically() {
        let mut registry = ContributionRegistry::default();
        registry
            .register(contribution("rustok.pages.hero", None))
            .expect("first");
        let mut duplicate = contribution("rustok.pages.hero.properties.alt", None);
        duplicate.renderers.clear();
        duplicate.property_editors[0].id = "alternative.properties".to_string();
        let error = registry.register(duplicate).expect_err("duplicate editor");
        assert!(matches!(error, UiError::DuplicatePropertyEditor(_)));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn provider_ownership_and_accessibility_labels_are_required() {
        let mut registry = ContributionRegistry::default();
        let mut wrong_provider = contribution("rustok.pages.hero", None);
        wrong_provider.renderers[0].provider = "other.provider".to_string();
        assert!(matches!(
            registry.register(wrong_provider),
            Err(UiError::InvalidContribution { .. })
        ));
        let mut missing_label = contribution("rustok.pages.hero", None);
        missing_label.renderers[0].accessibility.label_message_id = "  ".to_string();
        assert!(matches!(
            registry.register(missing_label),
            Err(UiError::InvalidContribution { .. })
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn registration_normalizes_identity_and_optional_accessibility_ids() {
        let mut registry = ContributionRegistry::default();
        let mut descriptor = contribution("  rustok.pages.hero  ", None);
        descriptor.provider = "  rustok.pages  ".to_string();
        descriptor.renderers[0].provider = "  rustok.pages  ".to_string();
        descriptor.renderers[0].accessibility.description_message_id = Some("  ".to_string());
        descriptor.property_editors[0].provider = "  rustok.pages  ".to_string();
        registry.register(descriptor).expect("register");
        let stored = registry.get("rustok.pages.hero").expect("stored");
        assert_eq!(stored.provider, "rustok.pages");
        assert!(
            stored.renderers[0]
                .accessibility
                .description_message_id
                .is_none()
        );
    }
}

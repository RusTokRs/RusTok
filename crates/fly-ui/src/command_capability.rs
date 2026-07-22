use crate::{CapabilityState, EditorCapability};
use fly::{ComponentPatch, EditorCommand, PageCommand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CommandCapabilityRequirement {
    capabilities: BTreeSet<EditorCapability>,
}

impl CommandCapabilityRequirement {
    pub fn for_command(command: &EditorCommand) -> Self {
        let mut requirement = Self::default();
        requirement.extend_command(command);
        requirement
    }

    pub fn capabilities(&self) -> impl Iterator<Item = EditorCapability> + '_ {
        self.capabilities.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    pub fn is_satisfied_by(&self, state: CapabilityState) -> bool {
        self.missing(state).is_empty()
    }

    pub fn missing(&self, state: CapabilityState) -> Vec<EditorCapability> {
        self.capabilities
            .iter()
            .copied()
            .filter(|capability| !state.allows(*capability))
            .collect()
    }

    pub fn first_missing(&self, state: CapabilityState) -> Option<EditorCapability> {
        self.capabilities
            .iter()
            .copied()
            .find(|capability| !state.allows(*capability))
    }

    fn insert(&mut self, capability: EditorCapability) {
        self.capabilities.insert(capability);
    }

    fn extend_command(&mut self, command: &EditorCommand) {
        match command {
            EditorCommand::Select { .. } => {}
            EditorCommand::Insert { .. }
            | EditorCommand::Remove { .. }
            | EditorCommand::Move { .. } => self.insert(EditorCapability::Edit),
            EditorCommand::Patch { patch, .. } => self.extend_component_patch(patch),
            EditorCommand::Asset { .. } => self.insert(EditorCapability::Assets),
            EditorCommand::StyleRule { .. } => self.insert(EditorCapability::Styles),
            EditorCommand::Page { command } => match command {
                PageCommand::Patch { .. } => self.insert(EditorCapability::Properties),
                PageCommand::Add { .. } | PageCommand::Remove { .. } | PageCommand::Move { .. } => {
                    self.insert(EditorCapability::Edit)
                }
            },
            EditorCommand::Dynamic { .. }
            | EditorCommand::Binding { .. }
            | EditorCommand::Context { .. }
            | EditorCommand::Translation { .. } => self.insert(EditorCapability::Properties),
            EditorCommand::RestoreSnapshot { .. } => self.insert(EditorCapability::History),
            EditorCommand::Batch { commands } => {
                for command in commands {
                    self.extend_command(command);
                }
            }
        }
    }

    fn extend_component_patch(&mut self, patch: &ComponentPatch) {
        let changes_properties = !patch.attributes.is_empty()
            || !patch.remove_attributes.is_empty()
            || !patch.fields.is_empty()
            || !patch.remove_fields.is_empty();
        let changes_styles = patch.style.is_some()
            || patch.replace_style
            || patch.clear_style
            || !patch.remove_style_properties.is_empty();
        if changes_properties {
            self.insert(EditorCapability::Properties);
        }
        if changes_styles {
            self.insert(EditorCapability::Styles);
        }
        if !changes_properties && !changes_styles {
            self.insert(EditorCapability::Edit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{AssetCommand, EditorCommand, PageLocator, PagePatch};
    use serde_json::{Map, json};

    #[test]
    fn property_and_style_patches_require_independent_capabilities() {
        let property = CommandCapabilityRequirement::for_command(&EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                fields: Map::from_iter([("content".to_string(), json!("Hello"))]),
                ..ComponentPatch::default()
            },
        });
        assert_eq!(
            property.capabilities().collect::<Vec<_>>(),
            vec![EditorCapability::Properties]
        );

        let style = CommandCapabilityRequirement::for_command(&EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                style: Some(json!({ "color": "red" })),
                ..ComponentPatch::default()
            },
        });
        assert_eq!(
            style.capabilities().collect::<Vec<_>>(),
            vec![EditorCapability::Styles]
        );
    }

    #[test]
    fn mixed_and_batch_commands_require_every_specialized_capability() {
        let command = EditorCommand::batch([
            EditorCommand::Asset {
                command: AssetCommand::Remove {
                    asset_id: "logo".to_string(),
                },
            },
            EditorCommand::Page {
                command: PageCommand::Patch {
                    locator: PageLocator::by_id("home"),
                    patch: PagePatch::default(),
                },
            },
        ]);
        let requirement = CommandCapabilityRequirement::for_command(&command);
        assert_eq!(
            requirement.capabilities().collect::<Vec<_>>(),
            vec![EditorCapability::Properties, EditorCapability::Assets]
        );
        let limited = CapabilityState {
            assets: false,
            ..CapabilityState::full()
        };
        assert_eq!(
            requirement.first_missing(limited),
            Some(EditorCapability::Assets)
        );
        assert_eq!(requirement.missing(limited), vec![EditorCapability::Assets]);
    }

    #[test]
    fn missing_returns_every_denied_capability_in_stable_order() {
        let command = EditorCommand::batch([
            EditorCommand::Asset {
                command: AssetCommand::Remove {
                    asset_id: "logo".to_string(),
                },
            },
            EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    fields: Map::from_iter([("src".to_string(), json!("asset:logo"))]),
                    ..ComponentPatch::default()
                },
            },
        ]);
        let requirement = CommandCapabilityRequirement::for_command(&command);
        let limited = CapabilityState {
            properties: false,
            assets: false,
            ..CapabilityState::full()
        };
        assert_eq!(
            requirement.missing(limited),
            vec![EditorCapability::Properties, EditorCapability::Assets]
        );
        assert!(!requirement.is_satisfied_by(limited));
    }

    #[test]
    fn structural_commands_require_edit_and_select_is_read_only_safe() {
        let insert = CommandCapabilityRequirement::for_command(&EditorCommand::Insert {
            parent_id: None,
            index: 0,
            component: fly::ComponentNode::object("section"),
        });
        assert_eq!(
            insert.capabilities().collect::<Vec<_>>(),
            vec![EditorCapability::Edit]
        );
        assert!(
            CommandCapabilityRequirement::for_command(&EditorCommand::Select {
                component_id: None,
            })
            .is_empty()
        );
    }
}

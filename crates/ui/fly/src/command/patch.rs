use crate::{ComponentObject, FlyError, FlyResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const COMPONENT_TYPE_FIELD: &str = "type";
const TAG_NAME_FIELD: &str = "tagName";
const PROVIDER_FIELD: &str = "provider";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ComponentPatch {
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Value>,
    #[serde(default)]
    pub replace_style: bool,
    #[serde(default)]
    pub clear_style: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_style_properties: Vec<String>,
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_fields: Vec<String>,
}

impl ComponentPatch {
    pub fn set_component_type(mut self, value: impl Into<String>) -> Self {
        self.set_reserved_string(COMPONENT_TYPE_FIELD, value.into());
        self
    }

    pub fn clear_component_type(mut self) -> Self {
        self.remove_reserved_field(COMPONENT_TYPE_FIELD);
        self
    }

    pub fn set_tag_name(mut self, value: impl Into<String>) -> Self {
        self.set_reserved_string(TAG_NAME_FIELD, value.into());
        self
    }

    pub fn clear_tag_name(mut self) -> Self {
        self.remove_reserved_field(TAG_NAME_FIELD);
        self
    }

    pub fn set_provider(mut self, value: impl Into<String>) -> Self {
        self.set_reserved_string(PROVIDER_FIELD, value.into());
        self
    }

    pub fn clear_provider(mut self) -> Self {
        self.remove_reserved_field(PROVIDER_FIELD);
        self
    }
    pub fn set_field(mut self, name: impl Into<String>, value: Value) -> Self {
        let name = name.into();
        self.remove_fields.retain(|candidate| candidate != &name);
        self.fields.insert(name, value);
        self
    }

    pub fn remove_field(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.fields.remove(&name);
        push_unique(&mut self.remove_fields, name);
        self
    }

    pub fn set_attribute(mut self, name: impl Into<String>, value: Value) -> Self {
        let name = name.into();
        self.remove_attributes
            .retain(|candidate| candidate != &name);
        self.attributes.insert(name, value);
        self
    }

    pub fn remove_attribute(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.attributes.remove(&name);
        push_unique(&mut self.remove_attributes, name);
        self
    }

    pub fn merge_style(mut self, style: Value) -> Self {
        // Consecutive merges compose rather than discarding earlier pending properties.
        // A merge following clear/replace still replaces the original component style.
        let touched = style
            .as_object()
            .map(|properties| properties.keys().cloned().collect::<Vec<_>>());
        // A non-object merge replaces the entire style slot. Preserve that
        // replacement boundary even if a later merge supplies an object:
        // otherwise applying the composed patch resurrects original properties.
        self.replace_style |= !style.is_object()
            || self.style.as_ref().is_some_and(|pending| !pending.is_object());
        match (&mut self.style, style) {
            (Some(Value::Object(current)), Value::Object(patch)) => current.extend(patch),
            (slot, patch) => *slot = Some(patch),
        }
        self.replace_style |= self.clear_style;
        self.clear_style = false;
        if let Some(touched) = touched {
            self.remove_style_properties
                .retain(|property| !touched.contains(property));
        } else {
            self.remove_style_properties.clear();
        }
        self
    }

    pub fn replace_style(mut self, style: Value) -> Self {
        self.style = Some(style);
        self.replace_style = true;
        self.clear_style = false;
        self.remove_style_properties.clear();
        self
    }

    pub fn clear_style(mut self) -> Self {
        self.style = None;
        self.replace_style = false;
        self.clear_style = true;
        self.remove_style_properties.clear();
        self
    }

    pub fn remove_style_property(mut self, name: impl Into<String>) -> Self {
        // Clearing the entire style already removes every property. Do not undo a
        // preceding clear_style() just because a later removal is redundant.
        if !self.clear_style {
            push_unique(&mut self.remove_style_properties, name.into());
        }
        self
    }

    pub(super) fn apply(self, component: &mut ComponentObject) -> FlyResult<()> {
        self.validate_reserved_fields()?;

        for attribute in self.remove_attributes {
            component.attributes.remove(&attribute);
        }
        component.attributes.extend(self.attributes);

        if self.clear_style {
            component.style = None;
        } else {
            if let Some(style) = self.style {
                if self.replace_style {
                    component.style = Some(style);
                } else {
                    merge_style(&mut component.style, style);
                }
            }
            // A serialized patch can contain conflicting sets/removals. Removal is
            // authoritative, as with attributes, regardless of JSON field order.
            if !self.remove_style_properties.is_empty()
                && let Some(Value::Object(style)) = component.style.as_mut()
            {
                for property in self.remove_style_properties {
                    style.remove(&property);
                }
            }
        }

        for field in self.remove_fields {
            match field.as_str() {
                COMPONENT_TYPE_FIELD => component.component_type = None,
                TAG_NAME_FIELD => component.tag_name = None,
                PROVIDER_FIELD => component.provider = None,
                _ => {
                    component.remove_extension_field(&field);
                }
            }
        }
        for (key, value) in self.fields {
            match key.as_str() {
                COMPONENT_TYPE_FIELD => {
                    component.component_type = value.as_str().map(ToString::to_string)
                }
                TAG_NAME_FIELD => component.tag_name = value.as_str().map(ToString::to_string),
                PROVIDER_FIELD => component.provider = value.as_str().map(ToString::to_string),
                _ => {
                    component.set_extension_field(key, value);
                }
            }
        }
        Ok(())
    }

    fn validate_reserved_fields(&self) -> FlyResult<()> {
        for field in self.fields.keys().chain(self.remove_fields.iter()) {
            if is_unpatchable_reserved_field(field) {
                return Err(FlyError::InvalidComponentPatch(format!(
                    "field `{field}` is owned by the component model and must use the dedicated command surface"
                )));
            }
        }
        for field in [COMPONENT_TYPE_FIELD, TAG_NAME_FIELD, PROVIDER_FIELD] {
            if self.fields.get(field).is_some_and(|value| !value.is_string()) {
                return Err(FlyError::InvalidComponentPatch(format!(
                    "field `{field}` must be a string; use remove_field to clear it"
                )));
            }
        }
        Ok(())
    }

    fn set_reserved_string(&mut self, field: &str, value: String) {
        self.remove_fields.retain(|candidate| candidate != field);
        self.fields.insert(field.to_string(), Value::String(value));
    }

    fn remove_reserved_field(&mut self, field: &str) {
        self.fields.remove(field);
        push_unique(&mut self.remove_fields, field.to_string());
    }
}

fn is_unpatchable_reserved_field(field: &str) -> bool {
    matches!(
        field,
        "id" | "attributes" | "style" | "traits" | "components"
    )
}

fn merge_style(current: &mut Option<Value>, patch: Value) {
    match (current.as_mut(), patch) {
        (Some(Value::Object(current)), Value::Object(patch)) => current.extend(patch),
        (_, patch) => *current = Some(patch),
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComponentNode;
    use serde_json::json;

    fn component() -> Box<ComponentObject> {
        let ComponentNode::Object(component) = ComponentNode::object("section") else {
            unreachable!()
        };
        component
    }

    #[test]
    fn typed_reserved_fields_set_and_clear_without_extension_leaks() {
        let mut component = component();
        ComponentPatch::default()
            .set_component_type("button")
            .set_tag_name("button")
            .set_provider("provider.demo")
            .apply(&mut component)
            .expect("apply reserved fields");
        assert_eq!(component.component_type.as_deref(), Some("button"));
        assert_eq!(component.tag_name.as_deref(), Some("button"));
        assert_eq!(component.provider.as_deref(), Some("provider.demo"));
        assert!(!component.extensions.contains_key(COMPONENT_TYPE_FIELD));

        ComponentPatch::default()
            .clear_component_type()
            .clear_tag_name()
            .clear_provider()
            .apply(&mut component)
            .expect("clear reserved fields");
        assert!(component.component_type.is_none());
        assert!(component.tag_name.is_none());
        assert!(component.provider.is_none());
    }

    #[test]
    fn typed_builders_resolve_set_remove_conflicts_deterministically() {
        let patch = ComponentPatch::default()
            .remove_attribute("aria-label")
            .set_attribute("aria-label", json!("Hero"))
            .remove_field("content")
            .set_field("content", json!("Welcome"));
        assert!(patch.remove_attributes.is_empty());
        assert!(patch.remove_fields.is_empty());
        assert_eq!(patch.attributes["aria-label"], "Hero");
        assert_eq!(patch.fields["content"], "Welcome");
    }

    #[test]
    fn structural_fields_cannot_be_written_as_flattened_extensions() {
        let mut component = component();
        let error = ComponentPatch::default()
            .set_field("components", json!([]))
            .apply(&mut component)
            .expect_err("reserved field");
        assert!(matches!(error, FlyError::InvalidComponentPatch(_)));
    }

    #[test]
    fn style_patch_builder_preserves_ordered_merge_replace_remove_and_clear() {
        let mut component = component();
        component.style = Some(json!({"color": "red", "padding": "1px"}));
        ComponentPatch::default()
            .merge_style(json!({"width": "100px", "color": "blue"}))
            .merge_style(json!({"height": "20px"}))
            .remove_style_property("color")
            .apply(&mut component)
            .expect("merge and remove");
        assert_eq!(component.style, Some(json!({
            "padding": "1px", "width": "100px", "height": "20px"
        })));

        ComponentPatch::default()
            .remove_style_property("color")
            .replace_style(json!({"color": "green"}))
            .merge_style(json!({"width": "200px"}))
            .apply(&mut component)
            .expect("replace then merge");
        assert_eq!(component.style, Some(json!({"color": "green", "width": "200px"})));

        ComponentPatch::default()
            .clear_style()
            .remove_style_property("color")
            .apply(&mut component)
            .expect("clear must survive redundant removal");
        assert_eq!(component.style, None);
    }

    #[test]
    fn composed_style_merges_do_not_resurrect_properties_after_scalar_replacement() {
        let original = json!({"color": "red", "padding": "1px"});
        for replacement in [json!("color: blue"), json!(null), json!([])] {
            let mut sequential = component();
            sequential.style = Some(original.clone());
            let mut composed = sequential.clone();

            ComponentPatch::default()
                .merge_style(replacement.clone())
                .apply(&mut sequential)
                .expect("replace with non-object");
            ComponentPatch::default()
                .merge_style(json!({"width": "100px"}))
                .apply(&mut sequential)
                .expect("merge object after replacement");

            let patch = ComponentPatch::default()
                .merge_style(json!({"height": "20px"}))
                .remove_style_property("padding")
                .merge_style(replacement)
                .merge_style(json!({"width": "100px"}));
            let patch: ComponentPatch =
                serde_json::from_value(serde_json::to_value(patch).expect("serialize patch"))
                    .expect("deserialize patch");
            patch.apply(&mut composed).expect("apply composed patch");

            assert_eq!(composed.style, sequential.style);
            assert_eq!(composed.style, Some(json!({"width": "100px"})));
        }
    }

    #[test]
    fn serialized_style_removal_wins_over_conflicting_set() {
        let mut component = component();
        let patch: ComponentPatch = serde_json::from_value(json!({
            "style": {"color": "blue", "padding": "8px"},
            "remove_style_properties": ["color"]
        }))
        .expect("patch");
        patch.apply(&mut component).expect("apply");
        assert_eq!(component.style, Some(json!({"padding": "8px"})));
    }

    #[test]
    fn malformed_reserved_field_does_not_silently_erase_existing_value() {
        let mut component = component();
        component.provider = Some("provider.original".to_string());
        for malformed in [json!(null), json!(42), json!({})] {
            let error = ComponentPatch::default()
                .set_field("provider", malformed)
                .apply(&mut component)
                .expect_err("invalid provider type");
            assert!(matches!(error, FlyError::InvalidComponentPatch(_)));
            assert_eq!(component.provider.as_deref(), Some("provider.original"));
        }
    }

    #[test]
    fn content_patch_replaces_grapesjs_scalar_text_children() {
        let mut component = component();
        component.components = crate::ComponentChildren::Nodes(vec![
            ComponentNode::Opaque(json!("Old text")),
            ComponentNode::object("span"),
        ]);
        ComponentPatch::default()
            .set_field("content", json!("New text"))
            .apply(&mut component)
            .expect("content patch");
        assert_eq!(component.extensions["content"], "New text");
        assert_eq!(component.children().len(), 1);
        assert!(component.children()[0].as_object().is_some());
    }
}

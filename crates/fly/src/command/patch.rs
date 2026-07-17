use crate::ComponentObject;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const COMPONENT_TYPE_FIELD: &str = "type";
const TAG_NAME_FIELD: &str = "tagName";
const PROVIDER_FIELD: &str = "provider";
const SCHEMA_VERSION_FIELD: &str = "schemaVersion";

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

    pub fn set_schema_version(mut self, value: impl Into<String>) -> Self {
        self.set_reserved_string(SCHEMA_VERSION_FIELD, value.into());
        self
    }

    pub fn clear_schema_version(mut self) -> Self {
        self.remove_reserved_field(SCHEMA_VERSION_FIELD);
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
        self.style = Some(style);
        self.replace_style = false;
        self.clear_style = false;
        self
    }

    pub fn replace_style(mut self, style: Value) -> Self {
        self.style = Some(style);
        self.replace_style = true;
        self.clear_style = false;
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
        self.clear_style = false;
        push_unique(&mut self.remove_style_properties, name.into());
        self
    }

    pub(super) fn apply(self, component: &mut ComponentObject) {
        for attribute in self.remove_attributes {
            component.attributes.remove(&attribute);
        }
        component.attributes.extend(self.attributes);

        if self.clear_style {
            component.style = None;
        } else {
            if !self.remove_style_properties.is_empty() {
                if let Some(Value::Object(style)) = component.style.as_mut() {
                    for property in self.remove_style_properties {
                        style.remove(&property);
                    }
                }
            }
            if let Some(style) = self.style {
                if self.replace_style {
                    component.style = Some(style);
                } else {
                    merge_style(&mut component.style, style);
                }
            }
        }

        for field in self.remove_fields {
            match field.as_str() {
                COMPONENT_TYPE_FIELD => component.component_type = None,
                TAG_NAME_FIELD => component.tag_name = None,
                PROVIDER_FIELD => component.provider = None,
                SCHEMA_VERSION_FIELD => component.schema_version = None,
                _ => {
                    component.extensions.remove(&field);
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
                SCHEMA_VERSION_FIELD => {
                    component.schema_version = value.as_str().map(ToString::to_string)
                }
                _ => {
                    component.extensions.insert(key, value);
                }
            }
        }
    }

    fn set_reserved_string(&mut self, field: &str, value: String) {
        self.remove_fields.retain(|candidate| candidate != field);
        self.fields
            .insert(field.to_string(), Value::String(value));
    }

    fn remove_reserved_field(&mut self, field: &str) {
        self.fields.remove(field);
        push_unique(&mut self.remove_fields, field.to_string());
    }
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
            .set_schema_version("2")
            .apply(&mut component);
        assert_eq!(component.component_type.as_deref(), Some("button"));
        assert_eq!(component.tag_name.as_deref(), Some("button"));
        assert_eq!(component.provider.as_deref(), Some("provider.demo"));
        assert_eq!(component.schema_version.as_deref(), Some("2"));
        assert!(!component.extensions.contains_key(COMPONENT_TYPE_FIELD));

        ComponentPatch::default()
            .clear_component_type()
            .clear_tag_name()
            .clear_provider()
            .clear_schema_version()
            .apply(&mut component);
        assert!(component.component_type.is_none());
        assert!(component.tag_name.is_none());
        assert!(component.provider.is_none());
        assert!(component.schema_version.is_none());
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
}
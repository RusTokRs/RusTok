use crate::{ComponentObject, ComponentPatch, FlyError, FlyResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraitValueKind {
    Text,
    Multiline,
    Boolean,
    Number,
    Select,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum TraitTarget {
    Attribute { name: String },
    Field { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraitOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraitSchema {
    pub id: String,
    pub label: String,
    pub value_type: TraitValueKind,
    pub target: TraitTarget,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub options: Vec<TraitOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraitSnapshot {
    pub schema: TraitSchema,
    pub value: Option<Value>,
}

impl TraitSchema {
    pub fn applies_to_component(&self, component_type: &str) -> bool {
        self.applies_to.is_empty()
            || self
                .applies_to
                .iter()
                .any(|candidate| candidate == component_type || candidate == "*")
    }

    pub fn read(&self, component: &ComponentObject) -> Option<Value> {
        match &self.target {
            TraitTarget::Attribute { name } => component.attributes.get(name).cloned(),
            TraitTarget::Field { name } => match name.as_str() {
                "tagName" => component.tag_name.clone().map(Value::String),
                "provider" => component.provider.clone().map(Value::String),
                "schemaVersion" => component.schema_version.clone().map(Value::String),
                _ => component.extensions.get(name).cloned(),
            },
        }
    }

    pub fn patch_from_text(&self, raw: &str) -> FlyResult<ComponentPatch> {
        let raw = raw.trim();
        if raw.is_empty() && !self.required {
            return Ok(self.remove_patch());
        }
        if raw.is_empty() {
            return Err(FlyError::InvalidTraitValue {
                trait_id: self.id.clone(),
                message: "value is required".to_string(),
            });
        }
        let value = match self.value_type {
            TraitValueKind::Text | TraitValueKind::Multiline => Value::String(raw.to_string()),
            TraitValueKind::Url => {
                if !trait_url_allowed(raw) {
                    return Err(FlyError::InvalidTraitValue {
                        trait_id: self.id.clone(),
                        message: "URL must be relative, http, https, mailto, tel, hash, or data:image"
                            .to_string(),
                    });
                }
                Value::String(raw.to_string())
            }
            TraitValueKind::Boolean => Value::Bool(parse_boolean(raw).ok_or_else(|| {
                FlyError::InvalidTraitValue {
                    trait_id: self.id.clone(),
                    message: "expected true or false".to_string(),
                }
            })?),
            TraitValueKind::Number => {
                let number = raw.parse::<f64>().map_err(|_| FlyError::InvalidTraitValue {
                    trait_id: self.id.clone(),
                    message: "expected a number".to_string(),
                })?;
                Value::Number(Number::from_f64(number).ok_or_else(|| {
                    FlyError::InvalidTraitValue {
                        trait_id: self.id.clone(),
                        message: "number must be finite".to_string(),
                    }
                })?)
            }
            TraitValueKind::Select => {
                if !self.options.iter().any(|option| option.value == raw) {
                    return Err(FlyError::InvalidTraitValue {
                        trait_id: self.id.clone(),
                        message: format!(
                            "expected one of: {}",
                            self.options
                                .iter()
                                .map(|option| option.value.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    });
                }
                Value::String(raw.to_string())
            }
        };
        Ok(self.set_patch(value))
    }

    pub fn set_patch(&self, value: Value) -> ComponentPatch {
        match &self.target {
            TraitTarget::Attribute { name } => ComponentPatch {
                attributes: Map::from_iter([(name.clone(), value)]),
                ..ComponentPatch::default()
            },
            TraitTarget::Field { name } => ComponentPatch {
                fields: Map::from_iter([(name.clone(), value)]),
                ..ComponentPatch::default()
            },
        }
    }

    pub fn remove_patch(&self) -> ComponentPatch {
        match &self.target {
            TraitTarget::Attribute { name } => ComponentPatch {
                remove_attributes: vec![name.clone()],
                ..ComponentPatch::default()
            },
            TraitTarget::Field { name } => ComponentPatch {
                remove_fields: vec![name.clone()],
                ..ComponentPatch::default()
            },
        }
    }
}

pub fn builtin_trait_schemas() -> Vec<TraitSchema> {
    vec![
        attribute_trait("fly.trait.id", "Element id", TraitValueKind::Text, "id", &["*"]),
        attribute_trait("fly.trait.class", "CSS classes", TraitValueKind::Text, "class", &["*"]),
        attribute_trait("fly.trait.title", "Title", TraitValueKind::Text, "title", &["*"]),
        attribute_trait("fly.trait.aria_label", "ARIA label", TraitValueKind::Text, "aria-label", &["*"]),
        field_trait("fly.trait.content", "Content", TraitValueKind::Multiline, "content", &["text", "heading", "link", "button", "label", "option", "submit"]),
        field_trait("fly.trait.tag_name", "HTML tag", TraitValueKind::Text, "tagName", &["section", "container", "row", "column", "grid", "text", "heading", "link", "button", "media", "form"]),
        attribute_trait("fly.trait.href", "Link URL", TraitValueKind::Url, "href", &["link", "button"]),
        select_trait("fly.trait.target", "Link target", "target", &["link", "button"], &[("Same frame", "_self"), ("New tab", "_blank"), ("Parent frame", "_parent"), ("Top frame", "_top")]),
        attribute_trait("fly.trait.rel", "Link relation", TraitValueKind::Text, "rel", &["link", "button"]),
        attribute_trait("fly.trait.src", "Source URL", TraitValueKind::Url, "src", &["image", "video"]),
        attribute_trait("fly.trait.alt", "Alternative text", TraitValueKind::Text, "alt", &["image"]),
        attribute_trait("fly.trait.poster", "Poster URL", TraitValueKind::Url, "poster", &["video"]),
        attribute_trait("fly.trait.controls", "Show controls", TraitValueKind::Boolean, "controls", &["video"]),
        attribute_trait("fly.trait.autoplay", "Autoplay", TraitValueKind::Boolean, "autoplay", &["video"]),
        attribute_trait("fly.trait.loop", "Loop", TraitValueKind::Boolean, "loop", &["video"]),
        attribute_trait("fly.trait.muted", "Muted", TraitValueKind::Boolean, "muted", &["video"]),
        select_trait("fly.trait.input_type", "Input type", "type", &["input"], &[("Text", "text"), ("Email", "email"), ("Password", "password"), ("Number", "number"), ("Telephone", "tel"), ("URL", "url"), ("Date", "date"), ("Hidden", "hidden")]),
        attribute_trait("fly.trait.name", "Field name", TraitValueKind::Text, "name", &["input", "textarea", "select", "checkbox"]),
        attribute_trait("fly.trait.placeholder", "Placeholder", TraitValueKind::Text, "placeholder", &["input", "textarea"]),
        attribute_trait("fly.trait.value", "Value", TraitValueKind::Text, "value", &["input", "option", "checkbox"]),
        attribute_trait("fly.trait.required", "Required", TraitValueKind::Boolean, "required", &["input", "textarea", "select", "checkbox"]),
        attribute_trait("fly.trait.disabled", "Disabled", TraitValueKind::Boolean, "disabled", &["input", "textarea", "select", "checkbox", "button", "submit"]),
        attribute_trait("fly.trait.checked", "Checked", TraitValueKind::Boolean, "checked", &["checkbox"]),
        attribute_trait("fly.trait.rows", "Rows", TraitValueKind::Number, "rows", &["textarea"]),
        attribute_trait("fly.trait.action", "Form action", TraitValueKind::Url, "action", &["form"]),
        select_trait("fly.trait.method", "Form method", "method", &["form"], &[("GET", "get"), ("POST", "post")]),
        attribute_trait("fly.trait.enctype", "Form encoding", TraitValueKind::Text, "enctype", &["form"]),
    ]
}

pub fn trait_snapshots<'a>(
    component: &ComponentObject,
    schemas: impl IntoIterator<Item = &'a TraitSchema>,
) -> Vec<TraitSnapshot> {
    schemas
        .into_iter()
        .filter(|schema| schema.applies_to_component(component.component_type()))
        .map(|schema| TraitSnapshot {
            schema: schema.clone(),
            value: schema.read(component),
        })
        .collect()
}

fn attribute_trait(
    id: &str,
    label: &str,
    value_type: TraitValueKind,
    name: &str,
    applies_to: &[&str],
) -> TraitSchema {
    TraitSchema {
        id: id.to_string(),
        label: label.to_string(),
        value_type,
        target: TraitTarget::Attribute {
            name: name.to_string(),
        },
        required: false,
        applies_to: applies_to.iter().map(|value| (*value).to_string()).collect(),
        options: Vec::new(),
        placeholder: None,
    }
}

fn field_trait(
    id: &str,
    label: &str,
    value_type: TraitValueKind,
    name: &str,
    applies_to: &[&str],
) -> TraitSchema {
    TraitSchema {
        id: id.to_string(),
        label: label.to_string(),
        value_type,
        target: TraitTarget::Field {
            name: name.to_string(),
        },
        required: false,
        applies_to: applies_to.iter().map(|value| (*value).to_string()).collect(),
        options: Vec::new(),
        placeholder: None,
    }
}

fn select_trait(
    id: &str,
    label: &str,
    name: &str,
    applies_to: &[&str],
    options: &[(&str, &str)],
) -> TraitSchema {
    TraitSchema {
        id: id.to_string(),
        label: label.to_string(),
        value_type: TraitValueKind::Select,
        target: TraitTarget::Attribute {
            name: name.to_string(),
        },
        required: false,
        applies_to: applies_to.iter().map(|value| (*value).to_string()).collect(),
        options: options
            .iter()
            .map(|(label, value)| TraitOption {
                label: (*label).to_string(),
                value: (*value).to_string(),
            })
            .collect(),
        placeholder: None,
    }
}

fn parse_boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn trait_url_allowed(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with('/')
        || value.starts_with('#')
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("mailto:")
        || value.starts_with("tel:")
        || value.starts_with("data:image/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trait_patch_targets_attributes_and_fields() {
        let href = builtin_trait_schemas()
            .into_iter()
            .find(|schema| schema.id == "fly.trait.href")
            .expect("href trait");
        let patch = href
            .patch_from_text("https://example.com")
            .expect("href patch");
        assert_eq!(patch.attributes["href"], "https://example.com");

        let content = builtin_trait_schemas()
            .into_iter()
            .find(|schema| schema.id == "fly.trait.content")
            .expect("content trait");
        let patch = content.patch_from_text("Hello").expect("content patch");
        assert_eq!(patch.fields["content"], "Hello");
    }

    #[test]
    fn empty_optional_trait_removes_target() {
        let alt = builtin_trait_schemas()
            .into_iter()
            .find(|schema| schema.id == "fly.trait.alt")
            .expect("alt trait");
        let patch = alt.patch_from_text("").expect("remove patch");
        assert_eq!(patch.remove_attributes, vec!["alt"]);
    }

    #[test]
    fn select_and_url_traits_validate_input() {
        let method = builtin_trait_schemas()
            .into_iter()
            .find(|schema| schema.id == "fly.trait.method")
            .expect("method trait");
        assert!(method.patch_from_text("patch").is_err());
        assert!(method.patch_from_text("post").is_ok());

        let href = builtin_trait_schemas()
            .into_iter()
            .find(|schema| schema.id == "fly.trait.href")
            .expect("href trait");
        assert!(href.patch_from_text("javascript:alert(1)").is_err());
        assert_eq!(
            href.patch_from_text("#contact")
                .expect("hash link")
                .attributes["href"],
            json!("#contact")
        );
    }
}

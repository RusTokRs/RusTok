use fly::ComponentPatch;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum StyleGroup {
    Layout,
    Size,
    Spacing,
    Typography,
    Background,
    Border,
    Effects,
    Position,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleValueKind {
    Text,
    Length,
    Number,
    Color,
    Keyword,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StylePropertyDescriptor {
    pub property: String,
    pub label: String,
    pub group: StyleGroup,
    pub value_kind: StyleValueKind,
    pub keywords: Vec<String>,
    pub allow_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyleEntry {
    pub property: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyleInputError {
    pub property: String,
    pub message: String,
}

pub fn builtin_style_properties() -> Vec<StylePropertyDescriptor> {
    vec![
        property(
            "display",
            "Display",
            StyleGroup::Layout,
            StyleValueKind::Keyword,
            &["block", "inline", "inline-block", "flex", "grid", "none"],
        ),
        property(
            "flex-direction",
            "Flex direction",
            StyleGroup::Layout,
            StyleValueKind::Keyword,
            &["row", "column", "row-reverse", "column-reverse"],
        ),
        property(
            "justify-content",
            "Justify content",
            StyleGroup::Layout,
            StyleValueKind::Keyword,
            &["start", "center", "end", "space-between", "space-around"],
        ),
        property(
            "align-items",
            "Align items",
            StyleGroup::Layout,
            StyleValueKind::Keyword,
            &["stretch", "start", "center", "end", "baseline"],
        ),
        property(
            "grid-template-columns",
            "Grid columns",
            StyleGroup::Layout,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "gap",
            "Gap",
            StyleGroup::Spacing,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "width",
            "Width",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "height",
            "Height",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "min-width",
            "Minimum width",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "min-height",
            "Minimum height",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "max-width",
            "Maximum width",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "max-height",
            "Maximum height",
            StyleGroup::Size,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "margin",
            "Margin",
            StyleGroup::Spacing,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "padding",
            "Padding",
            StyleGroup::Spacing,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "font-family",
            "Font family",
            StyleGroup::Typography,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "font-size",
            "Font size",
            StyleGroup::Typography,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "font-weight",
            "Font weight",
            StyleGroup::Typography,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "line-height",
            "Line height",
            StyleGroup::Typography,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "text-align",
            "Text align",
            StyleGroup::Typography,
            StyleValueKind::Keyword,
            &["left", "center", "right", "justify"],
        ),
        property(
            "color",
            "Text color",
            StyleGroup::Typography,
            StyleValueKind::Color,
            &[],
        ),
        property(
            "background",
            "Background",
            StyleGroup::Background,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "background-color",
            "Background color",
            StyleGroup::Background,
            StyleValueKind::Color,
            &[],
        ),
        property(
            "border",
            "Border",
            StyleGroup::Border,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "border-radius",
            "Border radius",
            StyleGroup::Border,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "box-shadow",
            "Box shadow",
            StyleGroup::Effects,
            StyleValueKind::Text,
            &[],
        ),
        property(
            "opacity",
            "Opacity",
            StyleGroup::Effects,
            StyleValueKind::Number,
            &[],
        ),
        property(
            "position",
            "Position",
            StyleGroup::Position,
            StyleValueKind::Keyword,
            &["static", "relative", "absolute", "sticky", "fixed"],
        ),
        property(
            "top",
            "Top",
            StyleGroup::Position,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "right",
            "Right",
            StyleGroup::Position,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "bottom",
            "Bottom",
            StyleGroup::Position,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "left",
            "Left",
            StyleGroup::Position,
            StyleValueKind::Length,
            &[],
        ),
        property(
            "z-index",
            "Z index",
            StyleGroup::Position,
            StyleValueKind::Number,
            &[],
        ),
    ]
}

pub fn style_patch(
    entries: impl IntoIterator<Item = StyleEntry>,
) -> Result<ComponentPatch, Vec<StyleInputError>> {
    let descriptors = builtin_style_properties();
    let mut style = Map::new();
    let mut remove_style_properties = Vec::new();
    let mut errors = Vec::new();

    for entry in entries {
        let property = entry.property.trim().to_ascii_lowercase();
        let value = entry.value.trim();
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.property == property);
        if !safe_property_name(&property) {
            errors.push(StyleInputError {
                property,
                message: "invalid CSS property name".to_string(),
            });
            continue;
        }
        if value.is_empty() {
            remove_style_properties.push(property);
            continue;
        }
        if !safe_style_value(value) {
            errors.push(StyleInputError {
                property,
                message: "style value contains a forbidden construct".to_string(),
            });
            continue;
        }
        if let Some(descriptor) = descriptor {
            if let Err(message) = validate_value(descriptor, value) {
                errors.push(StyleInputError { property, message });
                continue;
            }
        }
        style.insert(property, Value::String(value.to_string()));
    }

    if errors.is_empty() {
        Ok(ComponentPatch {
            style: (!style.is_empty()).then_some(Value::Object(style)),
            remove_style_properties,
            ..ComponentPatch::default()
        })
    } else {
        Err(errors)
    }
}

pub fn style_entries(style: Option<&Value>) -> Vec<StyleEntry> {
    style
        .and_then(Value::as_object)
        .map(|style| {
            style
                .iter()
                .filter_map(|(property, value)| {
                    scalar_to_string(value).map(|value| StyleEntry {
                        property: property.clone(),
                        value,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn property(
    property: &str,
    label: &str,
    group: StyleGroup,
    value_kind: StyleValueKind,
    keywords: &[&str],
) -> StylePropertyDescriptor {
    StylePropertyDescriptor {
        property: property.to_string(),
        label: label.to_string(),
        group,
        value_kind,
        keywords: keywords.iter().map(|value| (*value).to_string()).collect(),
        allow_empty: true,
    }
}

fn validate_value(descriptor: &StylePropertyDescriptor, value: &str) -> Result<(), String> {
    match descriptor.value_kind {
        StyleValueKind::Length => validate_length(value),
        StyleValueKind::Number => value
            .parse::<f64>()
            .map(|_| ())
            .map_err(|_| "expected a numeric value".to_string()),
        StyleValueKind::Color => validate_color(value),
        StyleValueKind::Keyword if !descriptor.keywords.is_empty() => {
            if descriptor.keywords.iter().any(|keyword| keyword == value) {
                Ok(())
            } else {
                Err(format!(
                    "expected one of: {}",
                    descriptor.keywords.join(", ")
                ))
            }
        }
        StyleValueKind::Text | StyleValueKind::Keyword => Ok(()),
    }
}

fn validate_length(value: &str) -> Result<(), String> {
    if matches!(
        value,
        "auto" | "min-content" | "max-content" | "fit-content"
    ) {
        return Ok(());
    }
    let units = [
        "px", "rem", "em", "%", "vw", "vh", "vmin", "vmax", "ch", "ex",
    ];
    let unit = units.iter().find(|unit| value.ends_with(**unit)).copied();
    let numeric = unit.map_or(value, |unit| &value[..value.len() - unit.len()]);
    if numeric.trim().parse::<f64>().is_ok() && (unit.is_some() || numeric.trim() == "0") {
        Ok(())
    } else {
        Err("expected a CSS length such as 24px, 2rem, 50%, auto, or 0".to_string())
    }
}

fn validate_color(value: &str) -> Result<(), String> {
    let normalized = value.to_ascii_lowercase();
    let named = [
        "transparent",
        "currentcolor",
        "black",
        "white",
        "red",
        "green",
        "blue",
        "inherit",
    ];
    if normalized.starts_with('#')
        || normalized.starts_with("rgb(")
        || normalized.starts_with("rgba(")
        || normalized.starts_with("hsl(")
        || normalized.starts_with("hsla(")
        || normalized.starts_with("var(")
        || named.contains(&normalized.as_str())
    {
        Ok(())
    } else {
        Err("expected a CSS color".to_string())
    }
}

fn safe_property_name(property: &str) -> bool {
    !property.is_empty()
        && property
            .chars()
            .all(|character| character.is_ascii_lowercase() || character == '-')
}

fn safe_style_value(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    !normalized.contains("expression(")
        && !normalized.contains("javascript:")
        && !normalized.contains("url(")
        && !value.contains('<')
        && !value.contains('>')
        && !value.contains(';')
}

fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_style_patch_merges_valid_entries_and_removes_empty_values() {
        let patch = style_patch([
            StyleEntry {
                property: "width".to_string(),
                value: "320px".to_string(),
            },
            StyleEntry {
                property: "color".to_string(),
                value: "".to_string(),
            },
        ])
        .expect("patch");
        assert_eq!(patch.style.expect("style")["width"], "320px");
        assert_eq!(patch.remove_style_properties, vec!["color"]);
    }

    #[test]
    fn typed_style_patch_rejects_urls_and_invalid_lengths() {
        assert!(
            style_patch([StyleEntry {
                property: "background".to_string(),
                value: "url(https://example.com/a.png)".to_string()
            }])
            .is_err()
        );
        assert!(
            style_patch([StyleEntry {
                property: "width".to_string(),
                value: "wide".to_string()
            }])
            .is_err()
        );
    }
}

use crate::{FlyError, FlyResult, ProjectDocument};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const FLY_COMPONENT_RULE_FIELD: &str = "flyComponentId";
pub const FLY_RULE_ID_FIELD: &str = "flyRuleId";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum StyleRuleScope {
    Base,
    Media { query: String },
}

impl StyleRuleScope {
    pub fn media_query(&self) -> Option<&str> {
        match self {
            Self::Base => None,
            Self::Media { query } => Some(query),
        }
    }

    pub fn stable_key(&self) -> String {
        match self {
            Self::Base => "base".to_string(),
            Self::Media { query } => format!("media:{}", normalize_query(query)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StyleRuleDescriptor {
    pub id: String,
    pub component_id: Option<String>,
    pub selector_names: Vec<String>,
    pub declarations: Map<String, Value>,
    pub scope: StyleRuleScope,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum StyleRuleCommand {
    UpsertComponentRule {
        component_id: String,
        scope: StyleRuleScope,
        #[serde(default)]
        declarations: Map<String, Value>,
        #[serde(default)]
        remove_properties: Vec<String>,
    },
    RemoveComponentRule {
        component_id: String,
        scope: StyleRuleScope,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StyleRuleCatalog {
    pub rules: Vec<StyleRuleDescriptor>,
    pub unknown_entries: Vec<Value>,
}

impl StyleRuleCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let mut catalog = Self::default();
        for raw in &document.project.styles {
            match StyleRuleDescriptor::from_value(raw.clone()) {
                Some(rule) => catalog.rules.push(rule),
                None => catalog.unknown_entries.push(raw.clone()),
            }
        }
        catalog
    }

    pub fn component_rule(
        &self,
        component_id: &str,
        scope: &StyleRuleScope,
    ) -> Option<&StyleRuleDescriptor> {
        self.rules
            .iter()
            .find(|rule| rule.component_id.as_deref() == Some(component_id) && rule.scope == *scope)
    }

    pub fn component_rules<'a>(
        &'a self,
        component_id: &'a str,
    ) -> impl Iterator<Item = &'a StyleRuleDescriptor> {
        self.rules
            .iter()
            .filter(move |rule| rule.component_id.as_deref() == Some(component_id))
    }
}

impl StyleRuleDescriptor {
    pub fn from_value(raw: Value) -> Option<Self> {
        let object = raw.as_object()?;
        let declarations = object
            .get("style")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let selector_names = selector_names(object.get("selectors"));
        let component_id = object
            .get(FLY_COMPONENT_RULE_FIELD)
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| component_id_from_selectors(object.get("selectors")));
        let scope = match object
            .get("atRuleType")
            .and_then(Value::as_str)
            .map(|value| value.to_ascii_lowercase())
        {
            Some(kind) if kind == "media" => StyleRuleScope::Media {
                query: object
                    .get("mediaText")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            },
            _ => StyleRuleScope::Base,
        };
        let id = object
            .get(FLY_RULE_ID_FIELD)
            .or_else(|| object.get("id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| stable_rule_id(component_id.as_deref(), &selector_names, &scope));
        Some(Self {
            id,
            component_id,
            selector_names,
            declarations,
            scope,
            raw,
        })
    }
}

pub fn apply_style_rule_command(
    document: &mut ProjectDocument,
    command: &StyleRuleCommand,
) -> FlyResult<()> {
    match command {
        StyleRuleCommand::UpsertComponentRule {
            component_id,
            scope,
            declarations,
            remove_properties,
        } => {
            if !document.contains_component(component_id) {
                return Err(FlyError::ComponentNotFound(component_id.clone()));
            }
            let existing = find_component_rule_index(document, component_id, scope);
            if let Some(index) = existing {
                let object = document.project.styles[index]
                    .as_object_mut()
                    .ok_or_else(|| {
                        FlyError::Encode("style rule must remain an object".to_string())
                    })?;
                let style = object
                    .entry("style".to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                let style = style.as_object_mut().ok_or_else(|| {
                    FlyError::Encode("style rule declarations must be an object".to_string())
                })?;
                for property in remove_properties {
                    style.remove(property);
                }
                style.extend(declarations.clone());
                synchronize_rule_identity(object, component_id, scope);
            } else if !declarations.is_empty() {
                document.project.styles.push(component_rule_value(
                    component_id,
                    scope,
                    declarations.clone(),
                ));
            }
            remove_empty_component_rules(document);
            Ok(())
        }
        StyleRuleCommand::RemoveComponentRule {
            component_id,
            scope,
        } => {
            let before = document.project.styles.len();
            document.project.styles.retain(|raw| {
                StyleRuleDescriptor::from_value(raw.clone()).is_none_or(|rule| {
                    rule.component_id.as_deref() != Some(component_id) || rule.scope != *scope
                })
            });
            if document.project.styles.len() == before {
                return Err(FlyError::StyleRuleNotFound(format!(
                    "{}:{}",
                    component_id,
                    scope.stable_key()
                )));
            }
            Ok(())
        }
    }
}

pub fn component_rule_value(
    component_id: &str,
    scope: &StyleRuleScope,
    declarations: Map<String, Value>,
) -> Value {
    let mut object = Map::from_iter([
        (
            "selectors".to_string(),
            json!([{ "name": component_id, "type": 2 }]),
        ),
        ("style".to_string(), Value::Object(declarations)),
    ]);
    synchronize_rule_identity(&mut object, component_id, scope);
    Value::Object(object)
}

fn synchronize_rule_identity(
    object: &mut Map<String, Value>,
    component_id: &str,
    scope: &StyleRuleScope,
) {
    object.insert(
        FLY_COMPONENT_RULE_FIELD.to_string(),
        Value::String(component_id.to_string()),
    );
    object.insert(
        FLY_RULE_ID_FIELD.to_string(),
        Value::String(stable_rule_id(
            Some(component_id),
            &[component_id.to_string()],
            scope,
        )),
    );
    match scope {
        StyleRuleScope::Base => {
            object.remove("atRuleType");
            object.remove("mediaText");
        }
        StyleRuleScope::Media { query } => {
            object.insert("atRuleType".to_string(), Value::String("media".to_string()));
            object.insert(
                "mediaText".to_string(),
                Value::String(normalize_query(query)),
            );
        }
    }
}

fn find_component_rule_index(
    document: &ProjectDocument,
    component_id: &str,
    scope: &StyleRuleScope,
) -> Option<usize> {
    document.project.styles.iter().position(|raw| {
        StyleRuleDescriptor::from_value(raw.clone()).is_some_and(|rule| {
            rule.component_id.as_deref() == Some(component_id) && rule.scope == *scope
        })
    })
}

fn remove_empty_component_rules(document: &mut ProjectDocument) {
    document.project.styles.retain(|raw| {
        StyleRuleDescriptor::from_value(raw.clone())
            .is_none_or(|rule| rule.component_id.is_none() || !rule.declarations.is_empty())
    });
}

fn selector_names(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(selectors)) => selectors
            .iter()
            .filter_map(|selector| match selector {
                Value::String(name) => Some(name.clone()),
                Value::Object(object) => object
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                _ => None,
            })
            .collect(),
        Some(Value::String(selector)) => vec![selector.clone()],
        _ => Vec::new(),
    }
}

fn component_id_from_selectors(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Array(selectors)) => selectors.iter().find_map(|selector| {
            let object = selector.as_object()?;
            let name = object.get("name")?.as_str()?;
            let selector_type = object.get("type").and_then(Value::as_u64);
            (selector_type == Some(2) || name.starts_with('#'))
                .then(|| name.trim_start_matches('#').to_string())
        }),
        Some(Value::String(selector)) if selector.starts_with('#') => {
            Some(selector.trim_start_matches('#').to_string())
        }
        _ => None,
    }
}

fn stable_rule_id(
    component_id: Option<&str>,
    selectors: &[String],
    scope: &StyleRuleScope,
) -> String {
    let source = format!(
        "{}|{}|{}",
        component_id.unwrap_or_default(),
        selectors.join(","),
        scope.stable_key()
    );
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fly-rule-{hash:016x}")
}

fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "styles": [],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn component_media_rule_round_trips_through_grapesjs_shape() {
        let mut document = document();
        apply_style_rule_command(
            &mut document,
            &StyleRuleCommand::UpsertComponentRule {
                component_id: "hero".to_string(),
                scope: StyleRuleScope::Media {
                    query: "(max-width: 767px)".to_string(),
                },
                declarations: Map::from_iter([(
                    "padding".to_string(),
                    Value::String("24px".to_string()),
                )]),
                remove_properties: Vec::new(),
            },
        )
        .expect("rule");
        let rule = StyleRuleCatalog::from_document(&document)
            .component_rule(
                "hero",
                &StyleRuleScope::Media {
                    query: "(max-width: 767px)".to_string(),
                },
            )
            .expect("component rule")
            .clone();
        assert_eq!(rule.declarations["padding"], "24px");
        assert_eq!(rule.raw["selectors"][0]["type"], 2);
        assert_eq!(rule.raw["atRuleType"], "media");
    }

    #[test]
    fn upsert_preserves_unknown_rule_fields() {
        let mut document = document();
        document.project.styles.push(json!({
            "selectors": [{ "name": "hero", "type": 2 }],
            "style": { "color": "red" },
            "flyComponentId": "hero",
            "customPluginField": { "keep": true }
        }));
        apply_style_rule_command(
            &mut document,
            &StyleRuleCommand::UpsertComponentRule {
                component_id: "hero".to_string(),
                scope: StyleRuleScope::Base,
                declarations: Map::from_iter([(
                    "width".to_string(),
                    Value::String("100%".to_string()),
                )]),
                remove_properties: Vec::new(),
            },
        )
        .expect("rule");
        assert_eq!(
            document.project.styles[0]["customPluginField"]["keep"],
            true
        );
        assert_eq!(document.project.styles[0]["style"]["color"], "red");
        assert_eq!(document.project.styles[0]["style"]["width"], "100%");
    }
}

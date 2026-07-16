use crate::AdminCanvasController;
use fly::{
    blank_page, ComponentPatch, EditorCommand, PageCommand, PageLocator, PageMetadata, PagePatch,
    FLY_PAGE_META_FIELD,
};
use fly_ui::UiIntent;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SsrComponentPropertyKind {
    Field,
    Attribute,
    Style,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrComponentPropertyRequest {
    pub component_id: String,
    pub kind: SsrComponentPropertyKind,
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrPageMetadataRequest {
    pub page_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub canonical_url: String,
    #[serde(default)]
    pub og_title: String,
    #[serde(default)]
    pub og_description: String,
    #[serde(default)]
    pub og_image: String,
    #[serde(default)]
    pub no_index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrPageCreateRequest {
    pub page_id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrPageRenameRequest {
    pub page_id: String,
    pub new_page_id: String,
    #[serde(default)]
    pub name: String,
}

impl AdminCanvasController {
    pub fn ssr_component_property_intent(
        &self,
        request: SsrComponentPropertyRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let name = required(&request.name, "property name")?;
        if self.editor().document().component(component_id).is_none() {
            return Err(format!("component `{component_id}` does not exist"));
        }
        if name == "id" || name == "components" {
            return Err(format!("property `{name}` cannot be edited through the SSR inspector"));
        }

        let mut patch = ComponentPatch::default();
        match request.kind {
            SsrComponentPropertyKind::Field => {
                if request.remove {
                    patch.remove_fields.push(name.to_string());
                } else {
                    patch
                        .fields
                        .insert(name.to_string(), Value::String(request.value));
                }
            }
            SsrComponentPropertyKind::Attribute => {
                if request.remove {
                    patch.remove_attributes.push(name.to_string());
                } else {
                    patch
                        .attributes
                        .insert(name.to_string(), Value::String(request.value));
                }
            }
            SsrComponentPropertyKind::Style => {
                let style_name = normalize_css_property(name)?;
                let mut style = Map::new();
                style.insert(
                    style_name,
                    if request.remove {
                        Value::Null
                    } else {
                        Value::String(request.value)
                    },
                );
                patch.style = Some(Value::Object(style));
            }
        }
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        }))
    }

    pub fn ssr_page_metadata_intent(
        &self,
        request: SsrPageMetadataRequest,
    ) -> Result<UiIntent, String> {
        let page_id = required(&request.page_id, "page id")?;
        let locator = PageLocator::by_id(page_id);
        let page = self
            .editor()
            .document()
            .page(&locator)
            .map_err(|error| error.to_string())?;
        let mut metadata = PageMetadata::from_page(page);
        metadata.title = optional(request.title);
        metadata.slug = optional(request.slug);
        metadata.description = optional(request.description);
        metadata.canonical_url = optional(request.canonical_url);
        metadata.og_title = optional(request.og_title);
        metadata.og_description = optional(request.og_description);
        metadata.og_image = optional(request.og_image);
        metadata.no_index = request.no_index;
        metadata = metadata.normalized().map_err(|error| error.to_string())?;

        let mut extensions = page.extensions.clone();
        metadata
            .apply_to_extensions(&mut extensions)
            .map_err(|error| error.to_string())?;
        let mut patch = PagePatch::default();
        match extensions.get(FLY_PAGE_META_FIELD).cloned() {
            Some(value) => {
                patch
                    .extensions
                    .insert(FLY_PAGE_META_FIELD.to_string(), value);
            }
            None => patch
                .remove_extensions
                .push(FLY_PAGE_META_FIELD.to_string()),
        }
        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Patch { locator, patch },
        }))
    }

    pub fn ssr_create_page_intent(
        &self,
        request: SsrPageCreateRequest,
    ) -> Result<UiIntent, String> {
        let page_id = normalize_page_id(&request.page_id)?;
        let name = optional(request.name).unwrap_or_else(|| page_id.clone());
        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Add {
                index: self.editor().document().project.pages.len(),
                page: Box::new(blank_page(page_id, name)),
            },
        }))
    }

    pub fn ssr_rename_page_intent(
        &self,
        request: SsrPageRenameRequest,
    ) -> Result<UiIntent, String> {
        let page_id = required(&request.page_id, "page id")?;
        let new_page_id = normalize_page_id(&request.new_page_id)?;
        let mut patch = PagePatch {
            id: Some(Some(new_page_id)),
            ..PagePatch::default()
        };
        if let Some(name) = optional(request.name) {
            patch
                .fields
                .insert("name".to_string(), Value::String(name));
        }
        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Patch {
                locator: PageLocator::by_id(page_id),
                patch,
            },
        }))
    }

    pub fn ssr_remove_page_intent(&self, page_id: &str) -> Result<UiIntent, String> {
        let page_id = required(page_id, "page id")?;
        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Remove {
                locator: PageLocator::by_id(page_id),
            },
        }))
    }
}

fn required<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value)
    }
}

fn optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn normalize_page_id(value: &str) -> Result<String, String> {
    let value = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else if character.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '_'])
        .to_string();
    if value.is_empty() {
        Err("page id must contain at least one letter or number".to_string())
    } else {
        Ok(value)
    }
}

fn normalize_css_property(value: &str) -> Result<String, String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.starts_with("--")
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(format!("CSS property `{value}` is not supported"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn property_request_builds_normal_fly_patch() {
        let mut controller = controller();
        let intent = controller
            .ssr_component_property_intent(SsrComponentPropertyRequest {
                component_id: "hero".to_string(),
                kind: SsrComponentPropertyKind::Attribute,
                name: "aria-label".to_string(),
                value: "Hero".to_string(),
                remove: false,
            })
            .expect("intent");
        controller.dispatch(intent).expect("patch");
        assert_eq!(
            controller
                .editor()
                .document()
                .component("hero")
                .unwrap()
                .attributes["aria-label"],
            "Hero"
        );
    }

    #[test]
    fn metadata_request_preserves_unknown_metadata_extensions() {
        let mut controller = controller();
        controller
            .editor_mut()
            .document_mut()
            .project
            .pages[0]
            .extensions
            .insert(
                FLY_PAGE_META_FIELD.to_string(),
                serde_json::json!({ "future": 42 }),
            );
        let intent = controller
            .ssr_page_metadata_intent(SsrPageMetadataRequest {
                page_id: "home".to_string(),
                title: "Home".to_string(),
                slug: "home".to_string(),
                description: String::new(),
                canonical_url: String::new(),
                og_title: String::new(),
                og_description: String::new(),
                og_image: String::new(),
                no_index: false,
            })
            .expect("metadata intent");
        controller.dispatch(intent).expect("metadata patch");
        assert_eq!(
            controller.editor().document().project.pages[0].extensions[FLY_PAGE_META_FIELD]
                ["future"],
            42
        );
    }
}

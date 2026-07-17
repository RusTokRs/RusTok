use crate::AdminCanvasController;
use fly::{
    blank_page, BindingCatalog, BindingCommand, BindingTarget, BindingTransform, ComponentPatch,
    EditorCommand, PageCommand, PageLocator, PageMetadata, PagePatch, RuntimeBinding,
    TranslationCommand, TranslationEntry, FLY_PAGE_METADATA_FIELD,
};
use fly_ui::UiIntent;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_TRANSLATION_VALUES_BYTES: usize = 256 * 1024;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrTranslationUpsertRequest {
    pub translation_id: String,
    pub values_json: String,
    #[serde(default)]
    pub fallback_locale: String,
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub bind_kind: Option<SsrComponentPropertyKind>,
    #[serde(default)]
    pub bind_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrTranslationRemoveRequest {
    pub translation_id: String,
}

impl AdminCanvasController {
    pub fn ssr_form_intent(
        &self,
        intent: &str,
        payload: &Value,
    ) -> Result<Option<UiIntent>, String> {
        let intent = match intent {
            "patch_component_property" => self.ssr_component_property_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid component property form: {error}"))?,
            )?,
            "patch_page_metadata" => self.ssr_page_metadata_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid page metadata form: {error}"))?,
            )?,
            "create_page" => self.ssr_create_page_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid create page form: {error}"))?,
            )?,
            "rename_page" => self.ssr_rename_page_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid rename page form: {error}"))?,
            )?,
            "remove_page" => self.ssr_remove_page_intent(
                payload
                    .get("page_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "remove page form requires `page_id`".to_string())?,
            )?,
            "upsert_translation" => self.ssr_upsert_translation_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid translation form: {error}"))?,
            )?,
            "remove_translation" => self.ssr_remove_translation_intent(
                serde_json::from_value(payload.clone())
                    .map_err(|error| format!("invalid remove translation form: {error}"))?,
            )?,
            _ => return Ok(None),
        };
        Ok(Some(intent))
    }

    pub fn ssr_component_property_intent(
        &self,
        request: SsrComponentPropertyRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let name = required(&request.name, "property name")?;
        ensure_component_exists(self, component_id)?;
        if name == "id" || name == "components" {
            return Err(format!(
                "property `{name}` cannot be edited through the SSR inspector"
            ));
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
        metadata.open_graph_title = optional(request.og_title);
        metadata.open_graph_description = optional(request.og_description);
        metadata.open_graph_image = optional(request.og_image);
        metadata.no_index = request.no_index;
        let metadata = metadata.normalized();
        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Patch {
                locator,
                patch: PagePatch {
                    fields: Map::from_iter([(
                        FLY_PAGE_METADATA_FIELD.to_string(),
                        metadata.into_value(),
                    )]),
                    ..PagePatch::default()
                },
            },
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
        let mut patch = PagePatch::default();
        patch
            .fields
            .insert("id".to_string(), Value::String(new_page_id));
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

    pub fn ssr_upsert_translation_intent(
        &self,
        request: SsrTranslationUpsertRequest,
    ) -> Result<UiIntent, String> {
        let translation_id = normalize_translation_id(&request.translation_id)?;
        if request.values_json.len() > MAX_TRANSLATION_VALUES_BYTES {
            return Err(format!(
                "translation values exceed {MAX_TRANSLATION_VALUES_BYTES} bytes"
            ));
        }
        let values = serde_json::from_str::<Value>(&request.values_json)
            .map_err(|error| format!("translation values JSON is invalid: {error}"))?
            .as_object()
            .cloned()
            .ok_or_else(|| "translation values must be a JSON object keyed by locale".to_string())?;
        if values.is_empty() {
            return Err("translation values must not be empty".to_string());
        }
        let fallback_locale = optional(request.fallback_locale);
        let mut commands = vec![EditorCommand::Translation {
            command: TranslationCommand::Upsert {
                entry: Box::new(TranslationEntry {
                    id: translation_id.clone(),
                    values,
                    fallback_locale,
                    extensions: Map::new(),
                }),
            },
        }];

        let component_id = request.component_id.trim();
        let bind_name = request.bind_name.trim();
        match (component_id.is_empty(), bind_name.is_empty(), request.bind_kind) {
            (true, true, None) => {}
            (false, false, Some(bind_kind)) => {
                ensure_component_exists(self, component_id)?;
                let target = binding_target(bind_kind, bind_name)?;
                commands.push(EditorCommand::Binding {
                    command: BindingCommand::Upsert {
                        binding: Box::new(RuntimeBinding {
                            id: translation_binding_id(
                                &translation_id,
                                component_id,
                                bind_kind,
                                bind_name,
                            ),
                            component_id: component_id.to_string(),
                            path: format!("translations.{translation_id}"),
                            target,
                            fallback: None,
                            transform: BindingTransform::Identity,
                            extensions: Map::new(),
                        }),
                    },
                });
            }
            _ => {
                return Err(
                    "translation binding requires component_id, bind_kind, and bind_name together"
                        .to_string(),
                );
            }
        }
        Ok(UiIntent::execute(EditorCommand::batch(commands)))
    }

    pub fn ssr_remove_translation_intent(
        &self,
        request: SsrTranslationRemoveRequest,
    ) -> Result<UiIntent, String> {
        let translation_id = normalize_translation_id(&request.translation_id)?;
        let translation_path = format!("translations.{translation_id}");
        let mut commands = BindingCatalog::from_document(self.editor().document())
            .bindings
            .into_iter()
            .filter(|binding| binding.path == translation_path)
            .map(|binding| EditorCommand::Binding {
                command: BindingCommand::Remove {
                    binding_id: binding.id,
                },
            })
            .collect::<Vec<_>>();
        commands.push(EditorCommand::Translation {
            command: TranslationCommand::Remove { translation_id },
        });
        Ok(UiIntent::execute(EditorCommand::batch(commands)))
    }
}

fn ensure_component_exists(
    controller: &AdminCanvasController,
    component_id: &str,
) -> Result<(), String> {
    if controller
        .editor()
        .document()
        .component(component_id)
        .is_none()
    {
        Err(format!("component `{component_id}` does not exist"))
    } else {
        Ok(())
    }
}

fn binding_target(
    kind: SsrComponentPropertyKind,
    name: &str,
) -> Result<BindingTarget, String> {
    let name = required(name, "binding target name")?.to_string();
    Ok(match kind {
        SsrComponentPropertyKind::Field => BindingTarget::Field { name },
        SsrComponentPropertyKind::Attribute => BindingTarget::Attribute { name },
        SsrComponentPropertyKind::Style => BindingTarget::Style {
            name: normalize_css_property(&name)?,
        },
    })
}

fn translation_binding_id(
    translation_id: &str,
    component_id: &str,
    kind: SsrComponentPropertyKind,
    name: &str,
) -> String {
    let kind = match kind {
        SsrComponentPropertyKind::Field => "field",
        SsrComponentPropertyKind::Attribute => "attribute",
        SsrComponentPropertyKind::Style => "style",
    };
    format!(
        "translation-{}-{}-{kind}-{}",
        stable_identifier(translation_id),
        stable_identifier(component_id),
        stable_identifier(name)
    )
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
    let value = stable_identifier(value);
    if value.is_empty() {
        Err("page id must contain at least one letter or number".to_string())
    } else {
        Ok(value)
    }
}

fn normalize_translation_id(value: &str) -> Result<String, String> {
    let value = stable_identifier(value);
    if value.is_empty() {
        Err("translation id must contain at least one letter or number".to_string())
    } else {
        Ok(value)
    }
}

fn stable_identifier(value: &str) -> String {
    value
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
        .to_string()
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
    use fly::{BindingCatalog, TranslationCatalog};
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "flyPageMeta": { "future": 42 },
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
            controller.editor().document().project.pages[0].extensions[FLY_PAGE_METADATA_FIELD]
                ["future"],
            42
        );
        assert_eq!(
            controller.editor().document().project.pages[0].extensions[FLY_PAGE_METADATA_FIELD]
                ["title"],
            "Home"
        );
    }

    #[test]
    fn translation_upsert_and_binding_are_one_history_entry() {
        let mut controller = controller();
        let intent = controller
            .ssr_upsert_translation_intent(SsrTranslationUpsertRequest {
                translation_id: "Hero Title".to_string(),
                values_json: json!({
                    "en": "Welcome",
                    "ru": "Добро пожаловать"
                })
                .to_string(),
                fallback_locale: "en".to_string(),
                component_id: "hero".to_string(),
                bind_kind: Some(SsrComponentPropertyKind::Field),
                bind_name: "content".to_string(),
            })
            .expect("translation intent");
        controller.dispatch(intent).expect("translation transaction");
        assert_eq!(controller.editor().history().undo_len(), 1);
        assert_eq!(
            TranslationCatalog::from_document(controller.editor().document())
                .entries[0]
                .id,
            "hero-title"
        );
        assert_eq!(
            BindingCatalog::from_document(controller.editor().document()).bindings[0].path,
            "translations.hero-title"
        );
        controller
            .dispatch(UiIntent::Undo)
            .expect("undo translation transaction");
        assert!(TranslationCatalog::from_document(controller.editor().document())
            .entries
            .is_empty());
        assert!(BindingCatalog::from_document(controller.editor().document())
            .bindings
            .is_empty());
    }

    #[test]
    fn removing_translation_removes_its_bindings_in_one_history_entry() {
        let mut controller = controller();
        let upsert = controller
            .ssr_upsert_translation_intent(SsrTranslationUpsertRequest {
                translation_id: "hero".to_string(),
                values_json: json!({ "en": "Hero" }).to_string(),
                fallback_locale: "en".to_string(),
                component_id: "hero".to_string(),
                bind_kind: Some(SsrComponentPropertyKind::Field),
                bind_name: "content".to_string(),
            })
            .expect("upsert");
        controller.dispatch(upsert).expect("translation transaction");
        let remove = controller
            .ssr_remove_translation_intent(SsrTranslationRemoveRequest {
                translation_id: "hero".to_string(),
            })
            .expect("remove");
        controller.dispatch(remove).expect("remove transaction");
        assert_eq!(controller.editor().history().undo_len(), 2);
        assert!(TranslationCatalog::from_document(controller.editor().document())
            .entries
            .is_empty());
        assert!(BindingCatalog::from_document(controller.editor().document())
            .bindings
            .is_empty());
    }

    #[test]
    fn incomplete_translation_binding_is_rejected() {
        let controller = controller();
        assert!(controller
            .ssr_upsert_translation_intent(SsrTranslationUpsertRequest {
                translation_id: "hero".to_string(),
                values_json: json!({ "en": "Hero" }).to_string(),
                fallback_locale: String::new(),
                component_id: "hero".to_string(),
                bind_kind: None,
                bind_name: "content".to_string(),
            })
            .is_err());
    }

    #[test]
    fn form_dispatch_rejects_unknown_intents() {
        let controller = controller();
        assert!(controller
            .ssr_form_intent("unknown", &json!({}))
            .unwrap()
            .is_none());
    }
}

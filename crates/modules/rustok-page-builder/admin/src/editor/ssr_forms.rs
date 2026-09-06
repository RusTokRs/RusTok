use super::ssr_actions_forms::{
    SsrComponentActionRemoveRequest, SsrComponentActionRequest, SsrComponentFormRemoveRequest,
    SsrComponentFormRequest, SsrNativeFormFieldRequest,
};
use super::ssr_assets::{SsrAssetApplyRequest, SsrAssetRemoveRequest, SsrAssetUpsertRequest};
use crate::AdminCanvasController;
use fly::{
    BindingCatalog, BindingCommand, BindingTarget, BindingTransform, ComponentPatch, EditorCommand,
    FLY_PAGE_METADATA_FIELD, PageCommand, PageLocator, PageMetadata, PagePatch, RuntimeBinding,
    TranslationCommand, TranslationEntry, blank_page,
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
            "upsert_asset" => self.ssr_asset_upsert_intent(
                serde_json::from_value::<SsrAssetUpsertRequest>(payload.clone())
                    .map_err(|error| format!("invalid asset form: {error}"))?,
            )?,
            "remove_asset" => self.ssr_asset_remove_intent(
                serde_json::from_value::<SsrAssetRemoveRequest>(payload.clone())
                    .map_err(|error| format!("invalid remove asset form: {error}"))?,
            )?,
            "select_asset" => self.ssr_asset_apply_intent(
                serde_json::from_value::<SsrAssetApplyRequest>(payload.clone())
                    .map_err(|error| format!("invalid asset assignment form: {error}"))?,
            )?,
            "set_component_action" => self.ssr_component_action_intent(
                serde_json::from_value::<SsrComponentActionRequest>(payload.clone())
                    .map_err(|error| format!("invalid component action form: {error}"))?,
            )?,
            "remove_component_action" => self.ssr_remove_component_action_intent(
                serde_json::from_value::<SsrComponentActionRemoveRequest>(payload.clone())
                    .map_err(|error| format!("invalid remove component action form: {error}"))?,
            )?,
            "set_component_form" => self.ssr_component_form_intent(
                serde_json::from_value::<SsrComponentFormRequest>(payload.clone())
                    .map_err(|error| format!("invalid component form: {error}"))?,
            )?,
            "remove_component_form" => self.ssr_remove_component_form_intent(
                serde_json::from_value::<SsrComponentFormRemoveRequest>(payload.clone())
                    .map_err(|error| format!("invalid remove component form: {error}"))?,
            )?,
            "set_native_form_field" => self.ssr_native_form_field_intent(
                serde_json::from_value::<SsrNativeFormFieldRequest>(payload.clone())
                    .map_err(|error| format!("invalid native form field: {error}"))?,
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
            patch.fields.insert("name".to_string(), Value::String(name));
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
            .ok_or_else(|| {
                "translation values must be a JSON object keyed by locale".to_string()
            })?;
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
        match (
            component_id.is_empty(),
            bind_name.is_empty(),
            request.bind_kind,
        ) {
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
        return Err(format!("component `{component_id}` does not exist"));
    }
    Ok(())
}

fn normalize_page_id(value: &str) -> Result<String, String> {
    let value = required(value, "page id")?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(format!("page id `{value}` contains unsupported characters"));
    }
    Ok(value.to_string())
}

fn normalize_translation_id(value: &str) -> Result<String, String> {
    let value = required(value, "translation id")?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(format!(
            "translation id `{value}` contains unsupported characters"
        ));
    }
    Ok(value.to_string())
}

fn binding_target(kind: SsrComponentPropertyKind, name: &str) -> Result<BindingTarget, String> {
    let name = required(name, "binding property name")?.to_string();
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
        "fly.translation.{}.{}.{}.{}",
        stable_suffix(translation_id),
        stable_suffix(component_id),
        kind,
        stable_suffix(name)
    )
}

fn stable_suffix(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
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

fn normalize_css_property(value: &str) -> Result<String, String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("style property must contain only letters, digits, and hyphens".to_string());
    }
    Ok(value)
}

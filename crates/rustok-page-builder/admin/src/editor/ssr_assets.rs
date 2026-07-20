use crate::AdminCanvasController;
use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    AssetCatalog, AssetCommand, AssetDescriptor, AssetPolicy, EditorCommand, source_allowed,
    visit_project_components,
};
use fly_ui::{EditorCapability, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_ASSET_SOURCE_BYTES: usize = 64 * 1024;
const MAX_ASSET_TEXT_BYTES: usize = 4 * 1024;
const ALLOWED_ASSET_SOURCE_ATTRIBUTES: [&str; 3] = ["src", "srcset", "poster"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrAssetUpsertRequest {
    #[serde(default)]
    pub asset_id: String,
    pub source: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrAssetRemoveRequest {
    pub asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrAssetApplyRequest {
    pub component_id: String,
    pub asset_id: String,
    #[serde(default = "default_source_attribute")]
    pub source_attribute: String,
}

impl AdminCanvasController {
    pub fn ssr_asset_upsert_intent(
        &self,
        request: SsrAssetUpsertRequest,
    ) -> Result<UiIntent, String> {
        let source = required(&request.source, "asset source")?;
        validate_length(source, MAX_ASSET_SOURCE_BYTES, "asset source")?;
        let asset_id = optional(request.asset_id);
        if let Some(asset_id) = asset_id.as_deref() {
            validate_length(asset_id, MAX_ASSET_TEXT_BYTES, "asset id")?;
        }
        let name = optional(request.name);
        if let Some(name) = name.as_deref() {
            validate_length(name, MAX_ASSET_TEXT_BYTES, "asset name")?;
        }

        let catalog = AssetCatalog::from_document(self.editor().document());
        let mut raw = asset_id
            .as_deref()
            .and_then(|asset_id| catalog.get(asset_id))
            .and_then(|asset| asset.raw.as_object())
            .cloned()
            .unwrap_or_else(Map::new);

        raw.remove("source");
        raw.remove("url");
        raw.insert("src".to_string(), Value::String(source.to_string()));
        match asset_id {
            Some(asset_id) => {
                raw.remove("assetId");
                raw.insert("id".to_string(), Value::String(asset_id));
            }
            None => {
                raw.remove("id");
                raw.remove("assetId");
            }
        }
        for key in ["name", "filename", "title"] {
            raw.remove(key);
        }
        if let Some(name) = name {
            raw.insert("name".to_string(), Value::String(name));
        }

        let asset = Value::Object(raw);
        let descriptor = AssetDescriptor::from_value(asset.clone())
            .ok_or_else(|| "asset cannot be normalized".to_string())?;
        if !source_allowed(&descriptor.source, descriptor.kind, &AssetPolicy::default()) {
            return Err(format!(
                "asset source `{}` is rejected by the default asset policy",
                descriptor.source
            ));
        }

        Ok(UiIntent::execute(EditorCommand::Asset {
            command: AssetCommand::Upsert { asset },
        }))
    }

    pub fn ssr_asset_remove_intent(
        &self,
        request: SsrAssetRemoveRequest,
    ) -> Result<UiIntent, String> {
        let asset_id = required(&request.asset_id, "asset id")?;
        if AssetCatalog::from_document(self.editor().document())
            .get(asset_id)
            .is_none()
        {
            return Err(format!("asset `{asset_id}` does not exist"));
        }
        let references = asset_reference_component_ids(self, asset_id);
        if !references.is_empty() {
            return Err(format!(
                "asset `{asset_id}` is still referenced by component(s): {}",
                references.join(", ")
            ));
        }
        Ok(UiIntent::execute(EditorCommand::Asset {
            command: AssetCommand::Remove {
                asset_id: asset_id.to_string(),
            },
        }))
    }

    pub fn ssr_asset_apply_intent(
        &self,
        request: SsrAssetApplyRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let asset_id = required(&request.asset_id, "asset id")?;
        let source_attribute = normalize_asset_source_attribute(&request.source_attribute)?;
        if self.editor().document().component(component_id).is_none() {
            return Err(format!("component `{component_id}` does not exist"));
        }
        let catalog = AssetCatalog::from_document(self.editor().document());
        let asset = catalog
            .get(asset_id)
            .ok_or_else(|| format!("asset `{asset_id}` does not exist"))?;
        let patch = asset
            .component_patch(source_attribute)
            .map_err(|error| error.to_string())?;
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        }))
    }
}

#[component]
pub fn SsrAssetPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.ssrAssets.title",
            "SSR assets",
        );
        let description = t(
            locale.as_deref(),
            "page_builder.ssrAssets.description",
            "Create, remove, and assign project assets through the revision-protected SSR intent endpoint.",
        );
        let empty = t(
            locale.as_deref(),
            "page_builder.ssrAssets.empty",
            "No project assets are defined.",
        );
        let id_label = t(locale.as_deref(), "page_builder.field.assetId", "Asset id");
        let source_label = t(
            locale.as_deref(),
            "page_builder.field.assetUrl",
            "Asset URL",
        );
        let name_label = t(
            locale.as_deref(),
            "page_builder.ssrAssets.name",
            "Asset name",
        );
        let source_attribute_label = t(
            locale.as_deref(),
            "page_builder.ssrAssets.sourceAttribute",
            "Component source attribute",
        );
        let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
        let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
        let apply_label = t(locale.as_deref(), "page_builder.action.select", "Use");
        let catalog = runtime
            .controller
            .with(|controller| AssetCatalog::from_document(controller.editor().document()));
        let selected_component_id = runtime
            .controller
            .with(|controller| controller.ui().state.selection.component_id.clone())
            .unwrap_or_default();
        let assets_enabled = runtime.capability_enabled(EditorCapability::Assets);
        let apply_enabled =
            assets_enabled && runtime.capability_enabled(EditorCapability::Properties);
        let has_assets = !catalog.assets.is_empty();

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-assets="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{description}</p>
                </div>
                <form class="grid gap-2" data-fly-intent-form="upsert_asset">
                    <fieldset class="grid gap-2" disabled=!assets_enabled>
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{id_label}</span>
                            <input
                                name="asset_id"
                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                autocomplete="off"
                                spellcheck="false"
                            />
                        </label>
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{source_label}</span>
                            <input
                                name="source"
                                required=true
                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                placeholder="/media/hero.webp"
                                autocomplete="url"
                                spellcheck="false"
                            />
                        </label>
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{name_label}</span>
                            <input
                                name="name"
                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                autocomplete="off"
                            />
                        </label>
                        <button
                            type="submit"
                            class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                        >{add_label}</button>
                    </fieldset>
                </form>
                {(!has_assets).then(|| view! {
                    <p class="text-xs text-muted-foreground">{empty}</p>
                })}
                <div class="space-y-2">
                    {catalog.assets.into_iter().map(|asset| {
                        let apply_asset_id = asset.id.clone();
                        let remove_asset_id = asset.id.clone();
                        let component_id = selected_component_id.clone();
                        let source_attribute_label = source_attribute_label.clone();
                        let apply_label = apply_label.clone();
                        let remove_label = remove_label.clone();
                        view! {
                            <article class="space-y-2 rounded border border-border p-2 text-xs">
                                <div class="font-medium">
                                    {asset.name.clone().unwrap_or_else(|| asset.id.clone())}
                                </div>
                                <div class="break-all text-muted-foreground">{asset.source}</div>
                                <div class="grid gap-2 sm:grid-cols-2">
                                    <form class="grid gap-1" data-fly-intent-form="select_asset">
                                        <input
                                            type="hidden"
                                            name="component_id"
                                            value=component_id
                                            data-fly-selected-component-input="true"
                                        />
                                        <input type="hidden" name="asset_id" value=apply_asset_id />
                                        <label class="grid gap-1">
                                            <span>{source_attribute_label}</span>
                                            <select
                                                name="source_attribute"
                                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                            >
                                                <option value="src">"src"</option>
                                                <option value="srcset">"srcset"</option>
                                                <option value="poster">"poster"</option>
                                            </select>
                                        </label>
                                        <button
                                            type="submit"
                                            disabled=!apply_enabled
                                            class="w-fit rounded border border-primary/40 px-2 py-1 text-primary"
                                        >{apply_label}</button>
                                    </form>
                                    <form data-fly-intent-form="remove_asset">
                                        <input type="hidden" name="asset_id" value=remove_asset_id />
                                        <button
                                            type="submit"
                                            disabled=!assets_enabled
                                            class="w-fit rounded border border-destructive/40 px-2 py-1 text-destructive"
                                        >{remove_label}</button>
                                    </form>
                                </div>
                            </article>
                        }
                    }).collect_view()}
                </div>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-assets="disabled"></span> }.into_any()
    }
}

fn default_source_attribute() -> String {
    "src".to_string()
}

fn normalize_asset_source_attribute(value: &str) -> Result<&'static str, String> {
    let value = required(value, "source attribute")?.to_ascii_lowercase();
    ALLOWED_ASSET_SOURCE_ATTRIBUTES
        .into_iter()
        .find(|allowed| *allowed == value.as_str())
        .ok_or_else(|| {
            format!(
                "source attribute `{value}` is not supported; expected one of {}",
                ALLOWED_ASSET_SOURCE_ATTRIBUTES.join(", ")
            )
        })
}

fn asset_reference_component_ids(
    controller: &AdminCanvasController,
    asset_id: &str,
) -> Vec<String> {
    let mut references = Vec::new();
    visit_project_components(
        &controller.editor().document().project,
        |component, visit| {
            if component
                .attributes
                .get("data-fly-asset-id")
                .and_then(Value::as_str)
                == Some(asset_id)
            {
                references.push(
                    component
                        .id
                        .clone()
                        .unwrap_or_else(|| visit.path().to_string()),
                );
            }
        },
    );
    references
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

fn validate_length(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.len() > maximum {
        Err(format!("{label} exceeds {maximum} bytes"))
    } else {
        Ok(())
    }
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
                "assets": [{
                    "id": "hero",
                    "src": "/old.webp",
                    "name": "Old hero",
                    "providerFuture": { "preserve": true }
                }],
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "image", "type": "image" }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn asset_upsert_preserves_unknown_fields_and_history() {
        let mut controller = controller();
        let intent = controller
            .ssr_asset_upsert_intent(SsrAssetUpsertRequest {
                asset_id: "hero".to_string(),
                source: "/new.webp".to_string(),
                name: "Hero".to_string(),
            })
            .expect("upsert intent");
        controller.dispatch(intent).expect("upsert asset");
        let asset = AssetCatalog::from_document(controller.editor().document())
            .get("hero")
            .cloned()
            .expect("hero asset");
        assert_eq!(asset.source, "/new.webp");
        assert_eq!(asset.name.as_deref(), Some("Hero"));
        assert_eq!(asset.raw["providerFuture"]["preserve"], true);
        controller.dispatch(UiIntent::Undo).expect("undo asset");
        assert_eq!(
            AssetCatalog::from_document(controller.editor().document())
                .get("hero")
                .expect("restored asset")
                .source,
            "/old.webp"
        );
    }

    #[test]
    fn asset_apply_uses_explicit_component_and_provider_reference_patch() {
        let mut controller = controller();
        let intent = controller
            .ssr_asset_apply_intent(SsrAssetApplyRequest {
                component_id: "image".to_string(),
                asset_id: "hero".to_string(),
                source_attribute: "src".to_string(),
            })
            .expect("apply intent");
        controller.dispatch(intent).expect("apply asset");
        let component = controller.editor().document().component("image").unwrap();
        assert_eq!(component.attributes["src"], "/old.webp");
        assert_eq!(component.attributes["data-fly-asset-id"], "hero");
    }

    #[test]
    fn asset_apply_rejects_arbitrary_attribute_names() {
        let controller = controller();
        assert!(
            controller
                .ssr_asset_apply_intent(SsrAssetApplyRequest {
                    component_id: "image".to_string(),
                    asset_id: "hero".to_string(),
                    source_attribute: "onerror".to_string(),
                })
                .is_err()
        );
    }

    #[test]
    fn referenced_asset_cannot_be_removed() {
        let mut controller = controller();
        let apply = controller
            .ssr_asset_apply_intent(SsrAssetApplyRequest {
                component_id: "image".to_string(),
                asset_id: "hero".to_string(),
                source_attribute: "src".to_string(),
            })
            .expect("apply intent");
        controller.dispatch(apply).expect("apply asset");
        let error = controller
            .ssr_asset_remove_intent(SsrAssetRemoveRequest {
                asset_id: "hero".to_string(),
            })
            .expect_err("referenced asset removal");
        assert!(error.contains("image"));
    }

    #[test]
    fn asset_remove_uses_normal_history() {
        let mut controller = controller();
        let intent = controller
            .ssr_asset_remove_intent(SsrAssetRemoveRequest {
                asset_id: "hero".to_string(),
            })
            .expect("remove intent");
        controller.dispatch(intent).expect("remove asset");
        assert!(
            AssetCatalog::from_document(controller.editor().document())
                .get("hero")
                .is_none()
        );
        controller.dispatch(UiIntent::Undo).expect("undo remove");
        assert!(
            AssetCatalog::from_document(controller.editor().document())
                .get("hero")
                .is_some()
        );
    }

    #[test]
    fn unsafe_asset_source_is_rejected_before_dispatch() {
        let controller = controller();
        assert!(
            controller
                .ssr_asset_upsert_intent(SsrAssetUpsertRequest {
                    asset_id: "unsafe".to_string(),
                    source: "javascript:alert(1)".to_string(),
                    name: String::new(),
                })
                .is_err()
        );
    }
}

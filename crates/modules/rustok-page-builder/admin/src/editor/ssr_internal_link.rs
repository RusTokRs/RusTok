use crate::AdminCanvasController;
use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
use fly::{ComponentPatch, EditorCommand, FLY_PAGE_LINK_FIELD, InternalPageLink};
use fly_ui::UiIntent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};
use serde_json::Map;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrInternalPageLinkRequest {
    pub component_id: String,
    pub page_id: String,
    #[serde(default)]
    pub base_path: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub fragment: String,
    #[serde(default)]
    pub fallback_href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrInternalPageLinkRemoveRequest {
    pub component_id: String,
}

impl AdminCanvasController {
    pub fn ssr_internal_page_link_intent(
        &self,
        request: SsrInternalPageLinkRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let page_id = required(&request.page_id, "target page id")?;
        let component = self
            .editor()
            .document()
            .component(component_id)
            .ok_or_else(|| format!("component `{component_id}` does not exist"))?;
        if !self
            .editor()
            .document()
            .project
            .pages
            .iter()
            .any(|page| page.id.as_deref() == Some(page_id))
        {
            return Err(format!("target page `{page_id}` does not exist"));
        }
        let extensions = component
            .extensions
            .get(FLY_PAGE_LINK_FIELD)
            .cloned()
            .and_then(|value| serde_json::from_value::<InternalPageLink>(value).ok())
            .map(|link| link.extensions)
            .unwrap_or_else(Map::new);
        let link = InternalPageLink {
            page_id: page_id.to_string(),
            base_path: optional(request.base_path),
            query: optional(request.query),
            fragment: optional(request.fragment),
            fallback_href: optional(request.fallback_href),
            extensions,
        }
        .normalized()?;
        let value = serde_json::to_value(link)
            .map_err(|error| format!("internal page link cannot be encoded: {error}"))?;
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch: ComponentPatch {
                fields: Map::from_iter([(FLY_PAGE_LINK_FIELD.to_string(), value)]),
                ..ComponentPatch::default()
            },
        }))
    }

    pub fn ssr_remove_internal_page_link_intent(
        &self,
        request: SsrInternalPageLinkRemoveRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let component = self
            .editor()
            .document()
            .component(component_id)
            .ok_or_else(|| format!("component `{component_id}` does not exist"))?;
        if !component.extensions.contains_key(FLY_PAGE_LINK_FIELD) {
            return Err(format!(
                "component `{component_id}` does not define an internal page link"
            ));
        }
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch: ComponentPatch {
                remove_fields: vec![FLY_PAGE_LINK_FIELD.to_string()],
                ..ComponentPatch::default()
            },
        }))
    }
}

#[component]
pub fn SsrInternalPageLinkPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.internalLink.title",
            "Internal page link",
        );
        let description = t(
            locale.as_deref(),
            "page_builder.internalLink.description",
            "Reference a stable page id and let Fly generate the locale-specific href at runtime.",
        );
        let empty = t(
            locale.as_deref(),
            "page_builder.internalLink.empty",
            "Select a component to configure an internal page link.",
        );
        let target_label = t(
            locale.as_deref(),
            "page_builder.internalLink.targetLabel",
            "Target page",
        );
        let base_path_label = t(
            locale.as_deref(),
            "page_builder.internalLink.basePathLabel",
            "Base path",
        );
        let query_label = t(
            locale.as_deref(),
            "page_builder.internalLink.queryLabel",
            "Query",
        );
        let fragment_label = t(
            locale.as_deref(),
            "page_builder.internalLink.fragmentLabel",
            "Fragment",
        );
        let fallback_label = t(
            locale.as_deref(),
            "page_builder.internalLink.fallbackLabel",
            "Fallback href",
        );
        let save = t(
            locale.as_deref(),
            "page_builder.internalLink.save",
            "Save internal link",
        );
        let remove = t(
            locale.as_deref(),
            "page_builder.internalLink.remove",
            "Remove internal link",
        );
        let selected_component_id = runtime
            .controller
            .with(|controller| controller.ui().state.selection.component_id.clone());
        let page_options = runtime
            .controller
            .with(|controller| controller.page_summaries());
        let current = runtime.controller.with(|controller| {
            selected_component_id
                .as_deref()
                .and_then(|component_id| controller.editor().document().component(component_id))
                .and_then(|component| component.extensions.get(FLY_PAGE_LINK_FIELD))
                .cloned()
                .and_then(|value| serde_json::from_value::<InternalPageLink>(value).ok())
        });

        let Some(component_id) = selected_component_id else {
            return view! {
                <section
                    class="space-y-2 rounded-xl border border-border bg-card p-3"
                    data-fly-ssr-internal-link="true"
                >
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{empty}</p>
                </section>
            }
            .into_any();
        };
        let target_page_id = current
            .as_ref()
            .map(|link| link.page_id.clone())
            .or_else(|| page_options.first().and_then(|page| page.id.clone()))
            .unwrap_or_default();
        let base_path = current
            .as_ref()
            .and_then(|link| link.base_path.clone())
            .unwrap_or_default();
        let query = current
            .as_ref()
            .and_then(|link| link.query.clone())
            .unwrap_or_default();
        let fragment = current
            .as_ref()
            .and_then(|link| link.fragment.clone())
            .unwrap_or_default();
        let fallback_href = current
            .as_ref()
            .and_then(|link| link.fallback_href.clone())
            .unwrap_or_default();
        let has_link = current.is_some();
        let remove_component_id = component_id.clone();

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-internal-link="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{description}</p>
                </div>
                <form class="grid gap-2" data-fly-intent-form="set_internal_page_link">
                    <input
                        type="hidden"
                        name="component_id"
                        value=component_id
                        data-fly-selected-component-input="true"
                    />
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{target_label}</span>
                        <select
                            name="page_id"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                        >
                            {page_options.into_iter().filter_map(|page| {
                                let page_id = page.id?;
                                let selected = page_id == target_page_id;
                                Some(view! {
                                    <option value=page_id.clone() selected=selected>
                                        {format!("{} ({page_id})", page.name)}
                                    </option>
                                })
                            }).collect_view()}
                        </select>
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{base_path_label}</span>
                        <input
                            name="base_path"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="/"
                            value=base_path
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{query_label}</span>
                        <input
                            name="query"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="source=hero"
                            value=query
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{fragment_label}</span>
                        <input
                            name="fragment"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="team"
                            value=fragment
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{fallback_label}</span>
                        <input
                            name="fallback_href"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="/fallback"
                            value=fallback_href
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <button
                        type="submit"
                        class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                    >{save}</button>
                </form>
                {has_link.then(|| view! {
                    <form data-fly-intent-form="remove_internal_page_link">
                        <input
                            type="hidden"
                            name="component_id"
                            value=remove_component_id
                            data-fly-selected-component-input="true"
                        />
                        <button
                            type="submit"
                            class="w-fit rounded border border-destructive/40 px-2 py-1 text-xs text-destructive"
                        >{remove}</button>
                    </form>
                })}
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-internal-link="disabled"></span> }.into_any()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdminCanvasController;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "home-root",
                        "type": "wrapper",
                        "components": [{ "id": "link", "type": "link" }]
                    }
                }, {
                    "id": "about",
                    "flyPageMeta": { "slug": "about" },
                    "component": { "id": "about-root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn internal_link_form_uses_patch_history_and_preserves_extensions() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "link".to_string(),
                patch: ComponentPatch {
                    fields: Map::from_iter([(
                        FLY_PAGE_LINK_FIELD.to_string(),
                        json!({ "page_id": "about", "providerFuture": true }),
                    )]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("seed link extension through the controller");
        let intent = controller
            .ssr_internal_page_link_intent(SsrInternalPageLinkRequest {
                component_id: "link".to_string(),
                page_id: "about".to_string(),
                base_path: "/site".to_string(),
                query: String::new(),
                fragment: String::new(),
                fallback_href: String::new(),
            })
            .expect("internal link intent");
        controller.dispatch(intent).expect("internal link patch");
        let value = &controller
            .editor()
            .document()
            .component("link")
            .unwrap()
            .extensions[FLY_PAGE_LINK_FIELD];
        assert_eq!(value["page_id"], "about");
        assert_eq!(value["base_path"], "/site");
        assert_eq!(value["providerFuture"], true);
        controller
            .dispatch(UiIntent::Undo)
            .expect("undo link patch");
        assert!(
            controller
                .editor()
                .document()
                .component("link")
                .unwrap()
                .extensions
                .contains_key(FLY_PAGE_LINK_FIELD)
        );
    }

    #[test]
    fn missing_target_is_rejected_before_dispatch() {
        let controller = controller();
        assert!(
            controller
                .ssr_internal_page_link_intent(SsrInternalPageLinkRequest {
                    component_id: "link".to_string(),
                    page_id: "missing".to_string(),
                    base_path: String::new(),
                    query: String::new(),
                    fragment: String::new(),
                    fallback_href: String::new(),
                })
                .is_err()
        );
    }
}

use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
#[cfg(not(target_arch = "wasm32"))]
use fly::PageMetadata;
#[cfg(not(target_arch = "wasm32"))]
use fly::ProjectPage;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct SsrComponentOption {
    id: String,
    label: String,
    depth: usize,
}

#[component]
pub fn SsrInspectorPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let inspector_title = t(
            locale.as_deref(),
            "page_builder.ssrInspector.title",
            "Classic SSR inspector",
        );
        let inspector_description = t(
            locale.as_deref(),
            "page_builder.ssrInspector.description",
            "These forms submit directly to the consumer-owned Fly intent endpoint. No hydration or WASM is required.",
        );
        let runtime_context_title = t(
            locale.as_deref(),
            "page_builder.ssrInspector.runtimeContext",
            "Runtime preview context",
        );
        let runtime_context_aria = t(
            locale.as_deref(),
            "page_builder.ssrInspector.runtimeContextAria",
            "Runtime preview context JSON",
        );
        let runtime_context_help = t(
            locale.as_deref(),
            "page_builder.ssrInspector.runtimeContextHelp",
            "The context is stored in the server draft only. It is not written into the canonical page project.",
        );
        let apply_runtime_context = t(
            locale.as_deref(),
            "page_builder.ssrInspector.applyRuntimeContext",
            "Apply preview context",
        );
        let canvas_component = t(
            locale.as_deref(),
            "page_builder.ssrInspector.canvasComponent",
            "Canvas component",
        );
        let component_property = t(
            locale.as_deref(),
            "page_builder.ssrInspector.componentProperty",
            "Component property",
        );
        let field_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.fieldKind",
            "Field",
        );
        let attribute_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.attributeKind",
            "Attribute",
        );
        let inline_style_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.inlineStyleKind",
            "Inline style",
        );
        let property_name_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.propertyNamePlaceholder",
            "content, href, aria-label, color...",
        );
        let value_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.valuePlaceholder",
            "Value",
        );
        let remove_property = t(
            locale.as_deref(),
            "page_builder.ssrInspector.removeProperty",
            "Remove this property instead of setting it",
        );
        let apply_component_patch = t(
            locale.as_deref(),
            "page_builder.ssrInspector.applyComponentPatch",
            "Apply component patch",
        );
        let page_metadata = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageMetadata",
            "Page metadata and SEO",
        );
        let save_page_metadata = t(
            locale.as_deref(),
            "page_builder.ssrInspector.savePageMetadata",
            "Save page metadata",
        );
        let page_lifecycle = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageLifecycle",
            "Page lifecycle",
        );
        let add_page = t(
            locale.as_deref(),
            "page_builder.ssrInspector.addPage",
            "Add page",
        );
        let rename_page = t(
            locale.as_deref(),
            "page_builder.ssrInspector.renamePage",
            "Rename page",
        );
        let remove_page = t(
            locale.as_deref(),
            "page_builder.ssrInspector.removePage",
            "Remove page",
        );
        let page_id_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageIdPlaceholder",
            "page-id",
        );
        let new_page_id_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.newPageIdPlaceholder",
            "new-page-id",
        );
        let page_name_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageNamePlaceholder",
            "Page name",
        );
        let new_page_name_placeholder = t(
            locale.as_deref(),
            "page_builder.ssrInspector.newPageNamePlaceholder",
            "New page name",
        );
        let page_fallback = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageFallback",
            "Page",
        );
        let seo_title = t(
            locale.as_deref(),
            "page_builder.field.seoTitle",
            "SEO title",
        );
        let slug = t(locale.as_deref(), "page_builder.field.slug", "Slug");
        let seo_description = t(
            locale.as_deref(),
            "page_builder.field.seoDescription",
            "SEO description",
        );
        let canonical_url = t(
            locale.as_deref(),
            "page_builder.field.canonicalUrl",
            "Canonical URL",
        );
        let open_graph_title = t(
            locale.as_deref(),
            "page_builder.field.openGraphTitle",
            "Open Graph title",
        );
        let open_graph_description = t(
            locale.as_deref(),
            "page_builder.field.openGraphDescription",
            "Open Graph description",
        );
        let open_graph_image = t(
            locale.as_deref(),
            "page_builder.field.openGraphImage",
            "Open Graph image",
        );
        let no_index = t(
            locale.as_deref(),
            "page_builder.field.noIndex",
            "Prevent search indexing",
        );

        let (components, pages) = runtime.controller.with(|controller| {
            let components = controller
                .layer_items()
                .iter()
                .flat_map(flatten_layer)
                .collect::<Vec<_>>();
            let pages = controller.editor().document().project.pages.clone();
            (components, pages)
        });
        let default_component = components.first().map(|component| component.id.clone());
        let runtime_context_json =
            serde_json::to_string_pretty(&runtime.runtime_context.get_untracked())
                .unwrap_or_else(|_| "{}".to_string());
        let metadata_page_fallback = page_fallback.clone();

        view! {
            <section
                class="space-y-4 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-inspector="true"
            >
                <div>
                    <h2 class="font-semibold">{inspector_title}</h2>
                    <p class="text-xs text-muted-foreground">
                        {inspector_description}
                    </p>
                </div>

                <details class="rounded border border-border p-2" open>
                    <summary class="cursor-pointer text-xs font-semibold">{runtime_context_title}</summary>
                    <form
                        class="mt-3 grid gap-2"
                        data-fly-intent-form="set_runtime_context"
                    >
                        <textarea
                            name="context_json"
                            required
                            spellcheck="false"
                            class="min-h-52 rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                            aria-label=runtime_context_aria
                        >{runtime_context_json}</textarea>
                        <p class="text-xs text-muted-foreground">
                            {runtime_context_help}
                        </p>
                        <button
                            type="submit"
                            class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                        >
                            {apply_runtime_context}
                        </button>
                    </form>
                </details>

                <label class="grid gap-1 text-xs">
                    <span class="font-medium">{canvas_component}</span>
                    <select
                        class="rounded border border-input bg-background px-2 py-1"
                        data-fly-component-picker="true"
                    >
                        {components.iter().map(|component| {
                            let value = component.id.clone();
                            let label = format!("{}{} ({})", "  ".repeat(component.depth), component.label, component.id);
                            view! { <option value=value>{label}</option> }
                        }).collect_view()}
                    </select>
                </label>

                <form
                    class="grid gap-2 rounded border border-border p-2"
                    data-fly-intent-form="patch_component_property"
                >
                    <strong class="text-xs">{component_property}</strong>
                    <input
                        type="hidden"
                        name="component_id"
                        data-fly-selected-component-input="true"
                        value=default_component.clone().unwrap_or_default()
                    />
                    <div class="grid grid-cols-2 gap-2">
                        <select name="kind" class="rounded border border-input bg-background px-2 py-1 text-xs">
                            <option value="field">{field_kind}</option>
                            <option value="attribute">{attribute_kind}</option>
                            <option value="style">{inline_style_kind}</option>
                        </select>
                        <input
                            name="name"
                            required
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder=property_name_placeholder
                        />
                    </div>
                    <textarea
                        name="value"
                        class="min-h-16 rounded border border-input bg-background px-2 py-1 text-xs"
                        placeholder=value_placeholder
                    ></textarea>
                    <label class="flex items-center gap-2 text-xs">
                        <input type="checkbox" name="remove" value="true"/>
                        <span>{remove_property}</span>
                    </label>
                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">
                        {apply_component_patch}
                    </button>
                </form>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">{page_metadata}</summary>
                    <div class="mt-3 space-y-3">
                        {pages.iter().enumerate().map(|(index, page)| {
                            let page_id = page.id.clone().unwrap_or_else(|| format!("page-{index}"));
                            let metadata = PageMetadata::from_page(page);
                            view! {
                                <form
                                    class="grid gap-2 rounded bg-muted/30 p-2"
                                    data-fly-intent-form="patch_page_metadata"
                                >
                                    <strong class="text-xs">{page_label(page, index, &metadata_page_fallback)}</strong>
                                    <input type="hidden" name="page_id" value=page_id/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="title" placeholder=seo_title.clone() value=metadata.title.unwrap_or_default()/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="slug" placeholder=slug.clone() value=metadata.slug.unwrap_or_default()/>
                                    <textarea class="min-h-16 rounded border border-input bg-background px-2 py-1 text-xs" name="description" placeholder=seo_description.clone()>{metadata.description.unwrap_or_default()}</textarea>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="canonical_url" placeholder=canonical_url.clone() value=metadata.canonical_url.unwrap_or_default()/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="og_title" placeholder=open_graph_title.clone() value=metadata.open_graph_title.unwrap_or_default()/>
                                    <textarea class="min-h-14 rounded border border-input bg-background px-2 py-1 text-xs" name="og_description" placeholder=open_graph_description.clone()>{metadata.open_graph_description.unwrap_or_default()}</textarea>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="og_image" placeholder=open_graph_image.clone() value=metadata.open_graph_image.unwrap_or_default()/>
                                    <label class="flex items-center gap-2 text-xs">
                                        <input type="checkbox" name="no_index" value="true" checked=metadata.no_index/>
                                        <span>{no_index.clone()}</span>
                                    </label>
                                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">{save_page_metadata.clone()}</button>
                                </form>
                            }
                        }).collect_view()}
                    </div>
                </details>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">{page_lifecycle}</summary>
                    <div class="mt-3 grid gap-3">
                        <form class="grid gap-2" data-fly-intent-form="create_page">
                            <strong class="text-xs">{add_page.clone()}</strong>
                            <input required name="page_id" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder=page_id_placeholder/>
                            <input name="name" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder=page_name_placeholder/>
                            <button type="submit" class="w-fit rounded border border-border px-2 py-1 text-xs">{add_page}</button>
                        </form>
                        <form class="grid gap-2 border-t border-border pt-3" data-fly-intent-form="rename_page">
                            <strong class="text-xs">{rename_page.clone()}</strong>
                            <PageSelect pages=pages.clone() name="page_id" page_fallback=page_fallback.clone()/>
                            <input required name="new_page_id" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder=new_page_id_placeholder/>
                            <input name="name" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder=new_page_name_placeholder/>
                            <button type="submit" class="w-fit rounded border border-border px-2 py-1 text-xs">{rename_page}</button>
                        </form>
                        <form class="grid gap-2 border-t border-border pt-3" data-fly-intent-form="remove_page">
                            <strong class="text-xs text-destructive">{remove_page.clone()}</strong>
                            <PageSelect pages=pages name="page_id" page_fallback=page_fallback/>
                            <button type="submit" class="w-fit rounded border border-destructive/40 px-2 py-1 text-xs text-destructive">{remove_page}</button>
                        </form>
                    </div>
                </details>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-inspector="disabled"></span> }.into_any()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn PageSelect(pages: Vec<ProjectPage>, name: &'static str, page_fallback: String) -> impl IntoView {
    view! {
        <select name=name class="rounded border border-input bg-background px-2 py-1 text-xs">
            {pages.into_iter().enumerate().map(move |(index, page)| {
                let value = page.id.clone().unwrap_or_else(|| format!("page-{index}"));
                view! { <option value=value>{page_label(&page, index, &page_fallback)}</option> }
            }).collect_view()}
        </select>
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn flatten_layer(layer: &crate::editor::LayerItemView) -> Vec<SsrComponentOption> {
    vec![SsrComponentOption {
        id: layer.id.clone(),
        label: layer.component_type.clone(),
        depth: layer.depth,
    }]
}

#[cfg(not(target_arch = "wasm32"))]
fn page_label(page: &ProjectPage, index: usize, page_fallback: &str) -> String {
    page.extensions
        .get("name")
        .and_then(serde_json::Value::as_str)
        .or_else(|| page.id.as_deref())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{page_fallback} {}", index + 1))
}

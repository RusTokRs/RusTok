use crate::editor::AdminEditorRuntime;
use fly::{PageMetadata, ProjectPage};
use leptos::prelude::*;

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

        view! {
            <section
                class="space-y-4 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-inspector="true"
            >
                <div>
                    <h2 class="font-semibold">"Classic SSR inspector"</h2>
                    <p class="text-xs text-muted-foreground">
                        "These forms submit directly to the consumer-owned Fly intent endpoint. No hydration or WASM is required."
                    </p>
                </div>

                <label class="grid gap-1 text-xs">
                    <span class="font-medium">"Canvas component"</span>
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
                    <strong class="text-xs">"Component property"</strong>
                    <input
                        type="hidden"
                        name="component_id"
                        data-fly-selected-component-input="true"
                        value=default_component.clone().unwrap_or_default()
                    />
                    <div class="grid grid-cols-2 gap-2">
                        <select name="kind" class="rounded border border-input bg-background px-2 py-1 text-xs">
                            <option value="field">"Field"</option>
                            <option value="attribute">"Attribute"</option>
                            <option value="style">"Inline style"</option>
                        </select>
                        <input
                            name="name"
                            required
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="content, href, aria-label, color..."
                        />
                    </div>
                    <textarea
                        name="value"
                        class="min-h-16 rounded border border-input bg-background px-2 py-1 text-xs"
                        placeholder="Value"
                    ></textarea>
                    <label class="flex items-center gap-2 text-xs">
                        <input type="checkbox" name="remove" value="true"/>
                        <span>"Remove this property instead of setting it"</span>
                    </label>
                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">
                        "Apply component patch"
                    </button>
                </form>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">"Page metadata and SEO"</summary>
                    <div class="mt-3 space-y-3">
                        {pages.iter().enumerate().map(|(index, page)| {
                            let page_id = page.id.clone().unwrap_or_else(|| format!("page-{index}"));
                            let metadata = PageMetadata::from_page(page);
                            view! {
                                <form
                                    class="grid gap-2 rounded bg-muted/30 p-2"
                                    data-fly-intent-form="patch_page_metadata"
                                >
                                    <strong class="text-xs">{page_label(page, index)}</strong>
                                    <input type="hidden" name="page_id" value=page_id/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="title" placeholder="SEO title" value=metadata.title.unwrap_or_default()/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="slug" placeholder="slug" value=metadata.slug.unwrap_or_default()/>
                                    <textarea class="min-h-16 rounded border border-input bg-background px-2 py-1 text-xs" name="description" placeholder="SEO description">{metadata.description.unwrap_or_default()}</textarea>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="canonical_url" placeholder="Canonical URL" value=metadata.canonical_url.unwrap_or_default()/>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="og_title" placeholder="Open Graph title" value=metadata.og_title.unwrap_or_default()/>
                                    <textarea class="min-h-14 rounded border border-input bg-background px-2 py-1 text-xs" name="og_description" placeholder="Open Graph description">{metadata.og_description.unwrap_or_default()}</textarea>
                                    <input class="rounded border border-input bg-background px-2 py-1 text-xs" name="og_image" placeholder="Open Graph image" value=metadata.og_image.unwrap_or_default()/>
                                    <label class="flex items-center gap-2 text-xs">
                                        <input type="checkbox" name="no_index" value="true" checked=metadata.no_index/>
                                        <span>"No index"</span>
                                    </label>
                                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">"Save page metadata"</button>
                                </form>
                            }
                        }).collect_view()}
                    </div>
                </details>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">"Page lifecycle"</summary>
                    <div class="mt-3 grid gap-3">
                        <form class="grid gap-2" data-fly-intent-form="create_page">
                            <strong class="text-xs">"Add page"</strong>
                            <input required name="page_id" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder="page-id"/>
                            <input name="name" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder="Page name"/>
                            <button type="submit" class="w-fit rounded border border-border px-2 py-1 text-xs">"Add page"</button>
                        </form>
                        <form class="grid gap-2 border-t border-border pt-3" data-fly-intent-form="rename_page">
                            <strong class="text-xs">"Rename page"</strong>
                            <PageSelect pages=pages.clone() name="page_id"/>
                            <input required name="new_page_id" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder="new-page-id"/>
                            <input name="name" class="rounded border border-input bg-background px-2 py-1 text-xs" placeholder="New page name"/>
                            <button type="submit" class="w-fit rounded border border-border px-2 py-1 text-xs">"Rename page"</button>
                        </form>
                        <form class="grid gap-2 border-t border-border pt-3" data-fly-intent-form="remove_page">
                            <strong class="text-xs text-destructive">"Remove page"</strong>
                            <PageSelect pages=pages name="page_id"/>
                            <button type="submit" class="w-fit rounded border border-destructive/40 px-2 py-1 text-xs text-destructive">"Remove page"</button>
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

#[component]
fn PageSelect(pages: Vec<ProjectPage>, name: &'static str) -> impl IntoView {
    view! {
        <select name=name class="rounded border border-input bg-background px-2 py-1 text-xs">
            {pages.into_iter().enumerate().map(|(index, page)| {
                let value = page.id.clone().unwrap_or_else(|| format!("page-{index}"));
                view! { <option value=value>{page_label(&page, index)}</option> }
            }).collect_view()}
        </select>
    }
}

fn flatten_layer(layer: &crate::editor::LayerItemView) -> Vec<SsrComponentOption> {
    fn visit(
        layer: &crate::editor::LayerItemView,
        depth: usize,
        output: &mut Vec<SsrComponentOption>,
    ) {
        output.push(SsrComponentOption {
            id: layer.id.clone(),
            label: layer.label.clone(),
            depth,
        });
        for child in &layer.children {
            visit(child, depth.saturating_add(1), output);
        }
    }
    let mut output = Vec::new();
    visit(layer, 0, &mut output);
    output
}

fn page_label(page: &ProjectPage, index: usize) -> String {
    page.extensions
        .get("name")
        .and_then(serde_json::Value::as_str)
        .or_else(|| page.id.as_deref())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("Page {}", index + 1))
}

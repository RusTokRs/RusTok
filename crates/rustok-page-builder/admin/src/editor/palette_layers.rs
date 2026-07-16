use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::collections::BTreeMap;

#[component]
pub fn PaletteLayersPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let palette_label = t(locale.as_deref(), "page_builder.panel.palette", "Blocks");
    let layers_label = t(locale.as_deref(), "page_builder.panel.layers", "Layers");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let drag_label = t(locale.as_deref(), "page_builder.action.drag", "Drag");
    let palette_runtime = runtime.clone();
    let layers_runtime = runtime;

    view! {
        <aside class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
            <section class="space-y-2">
                <h2 class="font-semibold">{palette_label}</h2>
                {move || {
                    let mut groups = BTreeMap::<String, Vec<_>>::new();
                    for block in palette_runtime.controller.with(|controller| controller.palette_blocks()) {
                        groups.entry(block.category.clone()).or_default().push(block);
                    }
                    groups.into_iter().map(|(category, blocks)| {
                        let category_open = category == "landing";
                        view! {
                            <details open=category_open class="rounded-lg border border-border">
                                <summary class="cursor-pointer px-3 py-2 text-sm font-medium">{category}</summary>
                                <div class="grid gap-2 border-t border-border p-2">
                                    {blocks.into_iter().map(|block| {
                                        let insert_id = block.id.clone();
                                        let drag_id = block.id.clone();
                                        let html_drag_id = block.id.clone();
                                        let browser_block_id = block.id.clone();
                                        let insert_runtime = palette_runtime.clone();
                                        let drag_runtime = palette_runtime.clone();
                                        let html_drag_runtime = palette_runtime.clone();
                                        let add_label = add_label.clone();
                                        let drag_label = drag_label.clone();
                                        view! {
                                            <article
                                                class="rounded-lg border border-border bg-background p-2"
                                                draggable="true"
                                                data-fly-block-id=browser_block_id
                                                on:dragstart=move |_| {
                                                    let intent = html_drag_runtime.controller.with(|controller| {
                                                        controller.begin_palette_drag_intent(&html_drag_id)
                                                    });
                                                    html_drag_runtime.dispatch_result(intent);
                                                }
                                            >
                                                <div class="text-sm font-medium">{block.label}</div>
                                                <div class="mt-2 flex gap-2">
                                                    <button
                                                        type="button"
                                                        class="rounded border border-border px-2 py-1 text-xs"
                                                        data-fly-action="insert-block"
                                                        on:click=move |_| {
                                                            let intent = insert_runtime.controller.with(|controller| {
                                                                controller.insert_palette_block_intent(&insert_id)
                                                            });
                                                            insert_runtime.dispatch_result(intent);
                                                        }
                                                    >{add_label}</button>
                                                    <button
                                                        type="button"
                                                        class="rounded border border-border px-2 py-1 text-xs"
                                                        data-fly-action="begin-block-drag"
                                                        on:click=move |_| {
                                                            let intent = drag_runtime.controller.with(|controller| {
                                                                controller.begin_palette_drag_intent(&drag_id)
                                                            });
                                                            drag_runtime.dispatch_result(intent);
                                                        }
                                                    >{drag_label}</button>
                                                </div>
                                            </article>
                                        }
                                    }).collect_view()}
                                </div>
                            </details>
                        }
                    }).collect_view()
                }}
            </section>

            <section class="space-y-2 border-t border-border pt-3">
                <h2 class="font-semibold">{layers_label}</h2>
                <div class="space-y-1">
                    {move || {
                        let selected = layers_runtime.controller.with(|controller| {
                            controller.ui().state.selection.component_id.clone()
                        });
                        layers_runtime.controller.with(|controller| controller.layer_items()).into_iter().map(|layer| {
                            let component_id = layer.id.clone();
                            let browser_component_id = layer.id.clone();
                            let active = selected.as_deref() == Some(layer.id.as_str());
                            let select_runtime = layers_runtime.clone();
                            view! {
                                <button
                                    type="button"
                                    data-fly-component-id=browser_component_id
                                    data-fly-action="select-component"
                                    class=if active {
                                        "block w-full rounded bg-primary/10 px-2 py-1 text-left text-sm text-primary"
                                    } else {
                                        "block w-full rounded px-2 py-1 text-left text-sm hover:bg-muted"
                                    }
                                    style=format!("padding-left:{}px", 8 + layer.depth * 14)
                                    on:click=move |_| select_runtime.dispatch(
                                        UiIntent::Select(Some(component_id.clone()))
                                    )
                                >
                                    <span class="font-medium">{layer.component_type}</span>
                                    <span class="ml-1 text-xs text-muted-foreground">{layer.id}</span>
                                    <span class="ml-1 text-xs text-muted-foreground">{format!("({})", layer.child_count)}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>
            </section>
        </aside>
    }
}

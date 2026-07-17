use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly_ui::{
    ContributionAssemblyResult, ContributionAssemblySeverity, PaletteBlockAccess, UiIntent,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::collections::BTreeMap;
use std::sync::Arc;

fn layer_indent_class(depth: usize) -> &'static str {
    match depth {
        0 => "pl-2",
        1 => "pl-[22px]",
        2 => "pl-9",
        3 => "pl-[50px]",
        4 => "pl-16",
        5 => "pl-[78px]",
        6 => "pl-[92px]",
        7 => "pl-[106px]",
        _ => "pl-[120px]",
    }
}

#[component]
pub fn PaletteLayersPanel(
    runtime: AdminEditorRuntime,
    contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let palette_label = t(locale.as_deref(), "page_builder.panel.palette", "Blocks");
    let layers_label = t(locale.as_deref(), "page_builder.panel.layers", "Layers");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let drag_label = t(locale.as_deref(), "page_builder.action.drag", "Drag");
    let palette_runtime = runtime.clone();
    let layers_runtime = runtime;
    let palette_access = Arc::new(PaletteBlockAccess::from_optional_assembly(
        contribution_assembly.as_deref(),
    ));
    let contribution_summary = contribution_assembly.as_ref().map(|assembly| {
        (
            assembly.registered_contributions,
            assembly.skipped_contributions,
            assembly.diagnostics.clone(),
        )
    });

    view! {
        <aside class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
            <section class="space-y-2">
                <h2 class="font-semibold">{palette_label}</h2>
                {contribution_summary.map(|(registered, skipped, diagnostics)| view! {
                    <div
                        class="space-y-2 rounded-lg border border-border bg-muted/30 p-2 text-xs"
                        data-fly-contribution-registry="true"
                        data-fly-contributions-registered=registered
                        data-fly-contributions-skipped=skipped
                    >
                        <div class="text-muted-foreground">
                            {format!("Provider contributions: {registered} registered, {skipped} skipped")}
                        </div>
                        {diagnostics.into_iter().map(|diagnostic| {
                            let (class, role) = match diagnostic.severity {
                                ContributionAssemblySeverity::Error => (
                                    "rounded border border-destructive/30 bg-destructive/10 px-2 py-1 text-destructive",
                                    "alert",
                                ),
                                ContributionAssemblySeverity::Warning => (
                                    "rounded border border-amber-300/50 bg-amber-50 px-2 py-1 text-amber-900",
                                    "status",
                                ),
                                ContributionAssemblySeverity::Info => (
                                    "rounded border border-border bg-background px-2 py-1 text-muted-foreground",
                                    "status",
                                ),
                            };
                            view! {
                                <p
                                    class=class
                                    role=role
                                    data-fly-contribution-diagnostic=diagnostic.code.clone()
                                >
                                    <code>{diagnostic.code.clone()}</code>
                                    {format!(": {}", diagnostic.message)}
                                </p>
                            }
                        }).collect_view()}
                    </div>
                })}
                {move || {
                    let access = Arc::clone(&palette_access);
                    let capabilities = palette_runtime.controller.with(|controller| {
                        controller.ui().state.capabilities
                    });
                    let can_insert = capabilities.edit;
                    let can_drag = capabilities.drag_drop;
                    let mut groups = BTreeMap::<String, Vec<_>>::new();
                    for block in palette_runtime.controller.with(|controller| {
                        controller.palette_blocks_with_access(&access)
                    }) {
                        groups.entry(block.category.clone()).or_default().push(block);
                    }
                    groups.into_iter().map(|(category, blocks)| {
                        let category_open = category == "landing";
                        let access = Arc::clone(&access);
                        view! {
                            <details open=category_open class="rounded-lg border border-border">
                                <summary class="cursor-pointer px-3 py-2 text-sm font-medium">{category}</summary>
                                <div class="grid gap-2 border-t border-border p-2">
                                    {blocks.into_iter().map(|block| {
                                        let insert_id = block.id.clone();
                                        let drag_id = block.id.clone();
                                        let html_drag_id = block.id.clone();
                                        let browser_block_id = block.id.clone();
                                        let contribution_ids = access
                                            .contribution_ids(&block.id)
                                            .map(ToString::to_string)
                                            .collect::<Vec<_>>();
                                        let contribution_attr = contribution_ids.join(",");
                                        let contribution_badge = (!contribution_ids.is_empty()).then(|| {
                                            let label = contribution_ids.join(", ");
                                            view! {
                                                <span
                                                    class="rounded bg-primary/10 px-1.5 py-0.5 text-[10px] text-primary"
                                                    title=format!("Provided by {label}")
                                                >"provider"</span>
                                            }
                                        });
                                        let insert_runtime = palette_runtime.clone();
                                        let drag_runtime = palette_runtime.clone();
                                        let html_drag_runtime = palette_runtime.clone();
                                        let insert_access = Arc::clone(&access);
                                        let drag_access = Arc::clone(&access);
                                        let html_drag_access = Arc::clone(&access);
                                        let add_label = add_label.clone();
                                        let drag_label = drag_label.clone();
                                        view! {
                                            <article
                                                class="rounded-lg border border-border bg-background p-2"
                                                draggable=can_drag
                                                aria-disabled=(!can_insert).then_some("true")
                                                data-fly-block-id=browser_block_id
                                                data-fly-contribution-ids=contribution_attr
                                                data-fly-can-insert=can_insert
                                                data-fly-can-drag=can_drag
                                                on:dragstart=move |_| {
                                                    let intent = html_drag_runtime.controller.with(|controller| {
                                                        controller.begin_palette_drag_intent_with_access(
                                                            &html_drag_id,
                                                            &html_drag_access,
                                                        )
                                                    });
                                                    html_drag_runtime.dispatch_result(intent);
                                                }
                                            >
                                                <div class="flex items-center justify-between gap-2 text-sm font-medium">
                                                    <span>{block.label}</span>
                                                    {contribution_badge}
                                                </div>
                                                <div class="mt-2 flex gap-2">
                                                    <button
                                                        type="button"
                                                        class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                                                        data-fly-action="insert-block"
                                                        disabled=!can_insert
                                                        on:click=move |_| {
                                                            let intent = insert_runtime.controller.with(|controller| {
                                                                controller.insert_palette_block_intent_with_access(
                                                                    &insert_id,
                                                                    &insert_access,
                                                                )
                                                            });
                                                            insert_runtime.dispatch_result(intent);
                                                        }
                                                    >{add_label}</button>
                                                    <button
                                                        type="button"
                                                        class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                                                        data-fly-action="begin-block-drag"
                                                        disabled=!can_drag
                                                        on:click=move |_| {
                                                            let intent = drag_runtime.controller.with(|controller| {
                                                                controller.begin_palette_drag_intent_with_access(
                                                                    &drag_id,
                                                                    &drag_access,
                                                                )
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
                            let state_class = if active {
                                "block w-full rounded bg-primary/10 px-2 py-1 text-left text-sm text-primary"
                            } else {
                                "block w-full rounded px-2 py-1 text-left text-sm hover:bg-muted"
                            };
                            let class = format!("{state_class} {}", layer_indent_class(layer.depth));
                            view! {
                                <button
                                    type="button"
                                    data-fly-component-id=browser_component_id
                                    data-fly-action="select-component"
                                    class=class
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

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{ContributionDescriptor, ContributionRegistry};
    use serde_json::Map;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn layer_indent_uses_a_bounded_class_scale() {
        assert_eq!(layer_indent_class(0), "pl-2");
        assert_eq!(layer_indent_class(3), "pl-[50px]");
        assert_eq!(layer_indent_class(8), "pl-[120px]");
        assert_eq!(layer_indent_class(usize::MAX), "pl-[120px]");
    }

    #[test]
    fn contribution_access_filters_templates_without_copying_block_definitions() {
        let mut registry = ContributionRegistry::default();
        registry
            .register(ContributionDescriptor {
                id: "pages.blocks".to_string(),
                provider: "fly.builtin".to_string(),
                required_capabilities: BTreeSet::new(),
                blocks: vec!["fly.hero".to_string()],
                renderers: Vec::new(),
                property_editors: Vec::new(),
                messages: BTreeMap::new(),
                metadata: Map::new(),
            })
            .expect("registry");
        let assembly = ContributionAssemblyResult {
            registry,
            registered_contributions: 1,
            ..ContributionAssemblyResult::default()
        };
        let access = PaletteBlockAccess::from_assembly(&assembly);
        assert!(access.allows("text"));
        assert!(access.allows("fly.hero"));
        assert!(!access.allows("fly.cta"));
        assert_eq!(
            access.contribution_ids("fly.hero").collect::<Vec<_>>(),
            vec!["pages.blocks"]
        );
    }
}

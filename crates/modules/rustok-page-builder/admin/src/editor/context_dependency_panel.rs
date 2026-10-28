use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    RuntimeContextConsumer, RuntimeContextPathSource, ValidationSeverity,
    analyze_runtime_context_dependencies,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn ContextDependencyPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let title = t(
        route_context.locale.as_deref(),
        "page_builder.panel.contextDependencies",
        "Context dependencies",
    );
    let filter = RwSignal::new(String::new());

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <input
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                placeholder="Filter runtime paths"
                prop:value=move || filter.get()
                on:input=move |event| filter.set(event_target_value(&event))
            />
            {move || {
                let graph = runtime.controller.with(|controller| {
                    analyze_runtime_context_dependencies(controller.editor().document())
                });
                let query = filter.get().trim().to_ascii_lowercase();
                let nodes = graph.nodes.into_iter().filter(|node| {
                    query.is_empty()
                        || node.path.to_ascii_lowercase().contains(&query)
                        || node.consumers.iter().any(|consumer| {
                            consumer_label(consumer).to_ascii_lowercase().contains(&query)
                        })
                }).collect::<Vec<_>>();
                let blocking = graph.diagnostics.iter()
                    .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
                    .count();
                let warnings = graph.diagnostics.iter()
                    .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Warning)
                    .count();
                let status_class = if blocking > 0 {
                    "rounded bg-destructive/10 px-2 py-1 text-destructive"
                } else {
                    "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700"
                };
                view! {
                    <div class="space-y-2 text-xs">
                        <p class="text-muted-foreground">{format!(
                            "{} fields · {} computed · {} external · {} consumers",
                            graph.declared_field_count,
                            graph.computed_count,
                            graph.external_path_count,
                            graph.consumer_count,
                        )}</p>
                        <p class=status_class>{format!("{blocking} blocking · {warnings} warnings")}</p>
                        {(!graph.computed_evaluation_order.is_empty()).then(|| view! {
                            <details>
                                <summary class="cursor-pointer font-medium">"Computed evaluation order"</summary>
                                <ol class="mt-1 list-inside list-decimal space-y-1">
                                    {graph.computed_evaluation_order.into_iter().map(|path| view! {
                                        <li class="break-all rounded bg-muted/50 px-2 py-1">{path}</li>
                                    }).collect_view()}
                                </ol>
                            </details>
                        })}
                        <div class="space-y-1">
                            {nodes.into_iter().map(|node| {
                                let external = node
                                    .sources
                                    .contains(&RuntimeContextPathSource::External);
                                let unused = node.consumers.is_empty();
                                let sources = node.sources.iter().map(|source| source_label(*source))
                                    .collect::<Vec<_>>().join(", ");
                                view! {
                                    <details class="rounded bg-muted/50 px-2 py-1">
                                        <summary class="cursor-pointer">
                                            <span class="break-all font-medium">{node.path.clone()}</span>
                                            <span class="ml-1 text-muted-foreground">
                                                {format!("· {sources} · {} consumer(s)", node.consumers.len())}
                                            </span>
                                            {external.then(|| view! {
                                                <span class="ml-1 text-amber-700">"host input"</span>
                                            })}
                                            {unused.then(|| view! {
                                                <span class="ml-1 text-muted-foreground">"unused"</span>
                                            })}
                                        </summary>
                                        <div class="mt-1 space-y-1 pl-2">
                                            {node.required.then(|| view! { <p>"Required input"</p> })}
                                            {node.has_default.then(|| view! { <p>"Has default"</p> })}
                                            {node.consumers.into_iter().map(|consumer| view! {
                                                <p class="break-all rounded bg-background/70 px-2 py-1">
                                                    {consumer_label(&consumer)}
                                                </p>
                                            }).collect_view()}
                                        </div>
                                    </details>
                                }
                            }).collect_view()}
                        </div>
                        {graph.diagnostics.into_iter().take(12).map(|diagnostic| {
                            let class = match diagnostic.severity {
                                ValidationSeverity::Error => "rounded bg-destructive/10 px-2 py-1 text-destructive",
                                ValidationSeverity::Warning => "rounded bg-amber-500/10 px-2 py-1 text-amber-700",
                                ValidationSeverity::Info => "rounded bg-muted px-2 py-1 text-muted-foreground",
                            };
                            view! {
                                <p class=class>
                                    <strong>{diagnostic.code}</strong>
                                    <span class="ml-1">{diagnostic.message}</span>
                                </p>
                            }
                        }).collect_view()}
                    </div>
                }
            }}
        </section>
    }
}

fn source_label(source: RuntimeContextPathSource) -> &'static str {
    match source {
        RuntimeContextPathSource::DeclaredField => "declared",
        RuntimeContextPathSource::Computed => "computed",
        RuntimeContextPathSource::External => "external",
    }
}

fn consumer_label(consumer: &RuntimeContextConsumer) -> String {
    match consumer {
        RuntimeContextConsumer::Computed { id, target_path } => {
            format!("computed `{id}` → `{target_path}`")
        }
        RuntimeContextConsumer::Binding {
            id,
            component_id,
            target,
        } => format!("binding `{id}` → component `{component_id}` {target}"),
        RuntimeContextConsumer::Condition { id, component_id } => {
            format!("condition `{id}` → component `{component_id}`")
        }
        RuntimeContextConsumer::Repeater { id, component_id } => {
            format!("repeater `{id}` → component `{component_id}`")
        }
    }
}

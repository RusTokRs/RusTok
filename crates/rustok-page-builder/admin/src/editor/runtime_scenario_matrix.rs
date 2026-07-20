use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    PageSelection, RenderPolicy, RuntimeScenarioRenderMatrix, ValidationSeverity,
    render_runtime_scenario_matrix,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

#[component]
pub fn RuntimeScenarioMatrixPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.runtimeScenarioMatrix",
        "Scenario render matrix",
    );
    let scenarios = Arc::clone(&runtime.runtime_scenarios);
    let has_scenarios = !scenarios.is_empty();
    let report_runtime = runtime;

    view! {
        <Show when=move || has_scenarios>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <h2 class="font-semibold">{title.clone()}</h2>
                {{
                    let report_scenarios = Arc::clone(&scenarios);
                    move || {
                    let matrix = report_runtime.controller.with(|controller| {
                        render_runtime_scenario_matrix(
                            controller.editor().document(),
                            &PageSelection::Index(controller.active_page_index()),
                            &RenderPolicy::default(),
                            report_scenarios.as_slice(),
                        )
                    });
                    let blocking = matrix
                        .cases
                        .iter()
                        .flat_map(|case| case.diagnostics.iter())
                        .chain(matrix.diagnostics.iter())
                        .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
                        .count();
                    let renderable = matrix.is_renderable();
                    let status_class = if renderable {
                        "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700"
                    } else {
                        "rounded bg-destructive/10 px-2 py-1 text-destructive"
                    };
                    let RuntimeScenarioRenderMatrix {
                        cases,
                        rendered_count,
                        failed_count,
                        unique_html_outputs,
                        duplicate_html_groups,
                        diagnostics,
                    } = matrix;
                    let duplicate_group_count = duplicate_html_groups.len();
                    view! {
                        <div class="space-y-2 text-xs">
                            <p class=status_class>{format!(
                                "{} rendered · {} failed · {} blocking",
                                rendered_count,
                                failed_count,
                                blocking,
                            )}</p>
                            <p class="text-muted-foreground">{format!(
                                "{} unique HTML output(s) · {} duplicate group(s)",
                                unique_html_outputs,
                                duplicate_group_count,
                            )}</p>
                            <div class="space-y-1">
                                {cases.into_iter().map(|case| {
                                    let error_count = case
                                        .diagnostics
                                        .iter()
                                        .filter(|diagnostic| {
                                            diagnostic.severity == ValidationSeverity::Error
                                        })
                                        .count();
                                    let summary = if case.rendered {
                                        format!("· rendered · {error_count} blocking")
                                    } else {
                                        "· render failed".to_string()
                                    };
                                    view! {
                                        <details class="rounded bg-muted/50 px-2 py-1">
                                            <summary class="cursor-pointer">
                                                <span class="font-medium">{case.scenario_label}</span>
                                                <span class="ml-1 text-muted-foreground">{summary}</span>
                                            </summary>
                                            <div class="mt-1 space-y-1 break-all text-muted-foreground">
                                                {case.html_hash.map(|hash| view! {
                                                    <p>{format!("HTML: {hash}")}</p>
                                                })}
                                                {case.css_hash.map(|hash| view! {
                                                    <p>{format!("CSS: {hash}")}</p>
                                                })}
                                                {case.document_hash.map(|hash| view! {
                                                    <p>{format!("Document: {hash}")}</p>
                                                })}
                                                <p>{format!(
                                                    "{} defaults · {} computed · {} bindings · {} hidden · {} repeated",
                                                    case.defaults_applied,
                                                    case.computed_applied,
                                                    case.applied_bindings,
                                                    case.hidden_components,
                                                    case.repeated_nodes,
                                                )}</p>
                                                {case.error.map(|error| view! {
                                                    <p class="text-destructive">{error}</p>
                                                })}
                                            </div>
                                        </details>
                                    }
                                }).collect_view()}
                            </div>
                            {(!duplicate_html_groups.is_empty()).then(|| view! {
                                <details>
                                    <summary class="cursor-pointer font-medium">"Duplicate output groups"</summary>
                                    <div class="mt-1 space-y-1">
                                        {duplicate_html_groups.into_iter().map(|group| view! {
                                            <p class="rounded bg-muted px-2 py-1">{group.join(", ")}</p>
                                        }).collect_view()}
                                    </div>
                                </details>
                            })}
                            {diagnostics.into_iter().map(|diagnostic| view! {
                                <p class="rounded bg-destructive/10 px-2 py-1 text-destructive">
                                    <strong>{diagnostic.code}</strong>
                                    <span class="ml-1">{diagnostic.message}</span>
                                </p>
                            }).collect_view()}
                        </div>
                    }
                    }
                }}
            </section>
        </Show>
    }
}

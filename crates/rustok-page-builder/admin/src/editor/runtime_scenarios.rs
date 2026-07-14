use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{preflight_runtime_context_scenarios, RuntimeContextPreflightPolicy};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn RuntimeScenarioPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.runtimeScenarios",
        "Preview scenarios",
    );
    let apply_label = t(
        locale.as_deref(),
        "page_builder.action.applyScenario",
        "Apply scenario",
    );
    let selected = RwSignal::new(
        runtime
            .active_runtime_scenario
            .get_untracked()
            .or_else(|| runtime.runtime_scenarios.first().map(|scenario| scenario.id.clone()))
            .unwrap_or_default(),
    );
    let apply_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <Show when=move || !report_runtime.runtime_scenarios.is_empty()>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <h2 class="font-semibold">{title}</h2>
                <div class="flex gap-2">
                    <select
                        class="min-w-0 flex-1 rounded border border-input bg-background px-2 py-1 text-sm"
                        prop:value=move || selected.get()
                        on:change=move |event| selected.set(event_target_value(&event))
                    >
                        {report_runtime.runtime_scenarios.iter().map(|scenario| view! {
                            <option value=scenario.id.clone()>{scenario.label.clone()}</option>
                        }).collect_view()}
                    </select>
                    <button
                        type="button"
                        class="shrink-0 rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let scenario_id = selected.get_untracked();
                            if !scenario_id.is_empty() {
                                apply_runtime.apply_runtime_scenario(&scenario_id);
                            }
                        }
                    >{apply_label}</button>
                </div>

                {move || {
                    let suite = report_runtime.controller.with(|controller| {
                        preflight_runtime_context_scenarios(
                            controller.editor().document(),
                            report_runtime.runtime_scenarios.as_slice(),
                            RuntimeContextPreflightPolicy::default(),
                        )
                    });
                    view! {
                        <div class="space-y-1 text-xs">
                            <p class="text-muted-foreground">{format!(
                                "{} accepted · {} rejected",
                                suite.accepted_count,
                                suite.rejected_count,
                            )}</p>
                            {suite.results.into_iter().map(|result| {
                                let accepted = result.preflight.accepted;
                                let issue_count = result.preflight.diagnostics.iter().filter(|diagnostic| {
                                    diagnostic.severity == fly::ValidationSeverity::Error
                                }).count();
                                view! {
                                    <div class="rounded bg-muted/50 px-2 py-1">
                                        <div class="flex items-center justify-between gap-2">
                                            <span class="truncate font-medium">{result.scenario_label}</span>
                                            <span class=if accepted {
                                                "text-emerald-700"
                                            } else {
                                                "text-destructive"
                                            }>
                                                {if accepted {
                                                    "Accepted".to_string()
                                                } else {
                                                    format!("{issue_count} blocking")
                                                }}
                                            </span>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                            {suite.diagnostics.into_iter().take(4).map(|diagnostic| view! {
                                <p class="rounded bg-destructive/10 px-2 py-1 text-destructive">
                                    {diagnostic.message}
                                </p>
                            }).collect_view()}
                        </div>
                    }
                }}
            </section>
        </Show>
    }
}

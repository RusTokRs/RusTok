use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    diff_runtime_scenario_render_snapshots, PageSelection, RenderPolicy,
    RuntimeScenarioRegressionStatus, RuntimeScenarioRenderChange,
    RuntimeScenarioRenderChangeImpact, RuntimeScenarioRenderSnapshot,
    FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

#[component]
pub fn RuntimeScenarioRegressionPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.runtimeScenarioRegression",
        "Scenario regression baseline",
    );
    let capture_label = t(
        locale.as_deref(),
        "page_builder.action.captureScenarioBaseline",
        "Capture current baseline",
    );
    let import_label = t(
        locale.as_deref(),
        "page_builder.action.importScenarioBaseline",
        "Use pasted baseline",
    );
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let scenarios = Arc::clone(&runtime.runtime_scenarios);
    let has_scenarios = !scenarios.is_empty();
    let baseline = RwSignal::new(None::<RuntimeScenarioRenderSnapshot>);
    let baseline_json = RwSignal::new(String::new());
    let capture_runtime = runtime.clone();
    let import_runtime = runtime.clone();
    let report_runtime = runtime;
    let capture_scenarios = Arc::clone(&scenarios);
    let report_scenarios = Arc::clone(&scenarios);

    view! {
        <Show when=move || has_scenarios>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <h2 class="font-semibold">{title}</h2>
                <div class="flex flex-wrap gap-2">
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let snapshot = capture_runtime.controller.with(|controller| {
                                RuntimeScenarioRenderSnapshot::capture(
                                    controller.editor().document(),
                                    &PageSelection::Index(controller.active_page_index()),
                                    &RenderPolicy::default(),
                                    capture_scenarios.as_slice(),
                                )
                            });
                            baseline_json.set(
                                serde_json::to_string_pretty(&snapshot).unwrap_or_default()
                            );
                            baseline.set(Some(snapshot));
                            capture_runtime.last_error.set(None);
                            capture_runtime.announce("Scenario render baseline captured");
                        }
                    >{capture_label}</button>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            match serde_json::from_str::<RuntimeScenarioRenderSnapshot>(
                                &baseline_json.get_untracked()
                            ) {
                                Ok(snapshot)
                                    if snapshot.format
                                        == FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1 =>
                                {
                                    baseline.set(Some(snapshot));
                                    import_runtime.last_error.set(None);
                                    import_runtime.announce(
                                        "Scenario render baseline imported"
                                    );
                                }
                                Ok(snapshot) => import_runtime.fail(format!(
                                    "Unsupported scenario snapshot format `{}`",
                                    snapshot.format
                                )),
                                Err(error) => import_runtime.fail(format!(
                                    "Invalid scenario baseline JSON: {error}"
                                )),
                            }
                        }
                    >{import_label}</button>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            baseline.set(None);
                            baseline_json.set(String::new());
                        }
                    >{clear_label}</button>
                </div>

                <textarea
                    class="min-h-36 w-full rounded border border-input bg-background px-2 py-1 font-mono text-[11px]"
                    placeholder="Paste fly_runtime_scenario_render_snapshot_v1 JSON"
                    prop:value=move || baseline_json.get()
                    on:input=move |event| baseline_json.set(event_target_value(&event))
                ></textarea>

                {move || {
                    let Some(previous) = baseline.get() else {
                        return view! {
                            <p class="text-xs text-muted-foreground">
                                "Capture or paste a baseline to detect scenario output drift."
                            </p>
                        }
                        .into_any();
                    };
                    let next = report_runtime.controller.with(|controller| {
                        RuntimeScenarioRenderSnapshot::capture(
                            controller.editor().document(),
                            &PageSelection::Index(controller.active_page_index()),
                            &RenderPolicy::default(),
                            report_scenarios.as_slice(),
                        )
                    });
                    let diff = diff_runtime_scenario_render_snapshots(&previous, &next);
                    let (status_class, status_label) = match diff.status {
                        RuntimeScenarioRegressionStatus::Stable => (
                            "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700",
                            "Stable",
                        ),
                        RuntimeScenarioRegressionStatus::RequiresReview => (
                            "rounded bg-amber-500/10 px-2 py-1 text-amber-700",
                            "Requires review",
                        ),
                        RuntimeScenarioRegressionStatus::Broken => (
                            "rounded bg-destructive/10 px-2 py-1 text-destructive",
                            "Broken",
                        ),
                    };
                    let visual_changes = diff
                        .changes
                        .iter()
                        .filter(|change| {
                            change.impact() == RuntimeScenarioRenderChangeImpact::Visual
                        })
                        .count();
                    let breaking_changes = diff
                        .changes
                        .iter()
                        .filter(|change| {
                            change.impact() == RuntimeScenarioRenderChangeImpact::Breaking
                        })
                        .count();
                    view! {
                        <div class="space-y-2 text-xs">
                            <div class=status_class>
                                <strong>{status_label}</strong>
                                <span class="ml-1">{format!(
                                    "· {} visual · {} breaking",
                                    visual_changes,
                                    breaking_changes,
                                )}</span>
                            </div>
                            <p class="break-all text-muted-foreground">{format!(
                                "{} → {}",
                                diff.previous_hash,
                                diff.next_hash,
                            )}</p>
                            {if diff.changes.is_empty() {
                                view! {
                                    <p class="rounded bg-muted/50 px-2 py-1">
                                        "No scenario output changes"
                                    </p>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="space-y-1">
                                        {diff.changes.into_iter().map(|change| {
                                            let impact = change.impact();
                                            let impact_class = match impact {
                                                RuntimeScenarioRenderChangeImpact::Informational => {
                                                    "text-muted-foreground"
                                                }
                                                RuntimeScenarioRenderChangeImpact::Visual => {
                                                    "text-amber-700"
                                                }
                                                RuntimeScenarioRenderChangeImpact::Breaking => {
                                                    "text-destructive"
                                                }
                                            };
                                            view! {
                                                <p class="rounded bg-muted px-2 py-1">
                                                    <span class=impact_class>{impact_label(impact)}</span>
                                                    <span class="ml-1">{change_summary(&change)}</span>
                                                </p>
                                            }
                                        }).collect_view()}
                                    </div>
                                }
                                .into_any()
                            }}
                        </div>
                    }
                    .into_any()
                }}
            </section>
        </Show>
    }
}

fn impact_label(impact: RuntimeScenarioRenderChangeImpact) -> &'static str {
    match impact {
        RuntimeScenarioRenderChangeImpact::Informational => "Info",
        RuntimeScenarioRenderChangeImpact::Visual => "Visual",
        RuntimeScenarioRenderChangeImpact::Breaking => "Breaking",
    }
}

fn change_summary(change: &RuntimeScenarioRenderChange) -> String {
    match change {
        RuntimeScenarioRenderChange::SnapshotFormatChanged { previous, next } => {
            format!("snapshot format changed from `{previous}` to `{next}`")
        }
        RuntimeScenarioRenderChange::SelectionChanged { .. } => {
            "selected page changed".to_string()
        }
        RuntimeScenarioRenderChange::PolicyChanged => "render policy changed".to_string(),
        RuntimeScenarioRenderChange::ScenarioAdded { scenario_id } => {
            format!("scenario `{scenario_id}` added")
        }
        RuntimeScenarioRenderChange::ScenarioRemoved { scenario_id } => {
            format!("scenario `{scenario_id}` removed")
        }
        RuntimeScenarioRenderChange::RenderStateChanged {
            scenario_id,
            previous,
            next,
        } => format!(
            "scenario `{scenario_id}` rendered changed from {previous} to {next}"
        ),
        RuntimeScenarioRenderChange::PageChanged { scenario_id, .. } => {
            format!("scenario `{scenario_id}` resolved to another page")
        }
        RuntimeScenarioRenderChange::HtmlChanged { scenario_id } => {
            format!("scenario `{scenario_id}` HTML changed")
        }
        RuntimeScenarioRenderChange::CssChanged { scenario_id } => {
            format!("scenario `{scenario_id}` CSS changed")
        }
        RuntimeScenarioRenderChange::DocumentChanged { scenario_id } => {
            format!("scenario `{scenario_id}` document changed")
        }
        RuntimeScenarioRenderChange::BlockingDiagnosticsChanged {
            scenario_id,
            previous,
            next,
        } => format!(
            "scenario `{scenario_id}` blocking diagnostics changed from {previous} to {next}"
        ),
        RuntimeScenarioRenderChange::RenderErrorChanged { scenario_id, .. } => {
            format!("scenario `{scenario_id}` render error changed")
        }
    }
}

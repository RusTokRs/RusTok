use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    FLY_RUNTIME_SCENARIO_RELEASE_BASELINE, PageSelection, RenderPolicy,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleasePolicy, RuntimeScenarioReleaseStatus,
    RuntimeScenarioRenderChange, RuntimeScenarioRenderChangeImpact,
    evaluate_runtime_scenario_release,
};
use leptos::prelude::*;
use rustok_page_builder::runtime_scenario_release::PageBuilderScenarioBaselineChange;
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

#[component]
pub fn RuntimeScenarioRegressionPanel(
    runtime: AdminEditorRuntime,
    initial_baseline: Option<RuntimeScenarioReleaseBaseline>,
    on_baseline_change: Option<Callback<PageBuilderScenarioBaselineChange>>,
) -> impl IntoView {
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
    let promotion_note_label = t(
        locale.as_deref(),
        "page_builder.field.scenarioBaselinePromotionNote",
        "Review note for replacing the persisted baseline",
    );
    let scenarios = Arc::clone(&runtime.runtime_scenarios);
    let has_scenarios = !scenarios.is_empty();
    let initial_json = initial_baseline
        .as_ref()
        .and_then(|baseline| serde_json::to_string_pretty(baseline).ok())
        .unwrap_or_default();
    let baseline = RwSignal::new(initial_baseline);
    let baseline_json = RwSignal::new(initial_json);
    let promotion_note = RwSignal::new(String::new());
    let callback = StoredValue::new(on_baseline_change);
    let capture_runtime = runtime.clone();
    let import_runtime = runtime.clone();
    let report_runtime = runtime;
    let capture_scenarios = Arc::clone(&scenarios);

    view! {
        <Show when=move || has_scenarios>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <h2 class="font-semibold">{title.clone()}</h2>
                <input
                    class="w-full rounded border border-input bg-background px-2 py-1 text-xs"
                    placeholder=promotion_note_label.clone()
                    prop:value=move || promotion_note.get()
                    on:input=move |event| promotion_note.set(event_target_value(&event))
                />
                <p class="text-xs text-muted-foreground">
                    "A review note is required when replacing an existing persisted baseline."
                </p>
                <div class="flex flex-wrap gap-2">
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click={
                            let capture_runtime = capture_runtime.clone();
                            let capture_scenarios = Arc::clone(&capture_scenarios);
                            move |_| {
                            let replacing = baseline.get_untracked().is_some();
                            let note = normalized_note(&promotion_note.get_untracked());
                            if replacing && note.is_none() {
                                capture_runtime.fail(
                                    "A review note is required before replacing the scenario baseline",
                                );
                                return;
                            }
                            let release_baseline = capture_runtime.controller.with(|controller| {
                                let baseline_id = format!(
                                    "{}:{}",
                                    controller.page_id(),
                                    controller.editor().revision().project_hash.hex(),
                                );
                                RuntimeScenarioReleaseBaseline::capture(
                                    baseline_id,
                                    controller.editor().document(),
                                    &PageSelection::Index(controller.active_page_index()),
                                    &RenderPolicy::default(),
                                    capture_scenarios.as_slice(),
                                )
                            });
                            if !release_baseline.is_valid() {
                                capture_runtime.fail(
                                    "Current scenario matrix cannot be used as a release baseline",
                                );
                                return;
                            }
                            baseline_json.set(
                                serde_json::to_string_pretty(&release_baseline)
                                    .unwrap_or_default(),
                            );
                            baseline.set(Some(release_baseline.clone()));
                            if let Some(callback) = callback.get_value() {
                                callback.run(PageBuilderScenarioBaselineChange::save(
                                    release_baseline,
                                    note,
                                ));
                            }
                            promotion_note.set(String::new());
                            capture_runtime.last_error.set(None);
                            capture_runtime.announce("Scenario release baseline captured");
                            }
                        }
                    >{capture_label.clone()}</button>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click={
                            let import_runtime = import_runtime.clone();
                            move |_| {
                            let replacing = baseline.get_untracked().is_some();
                            let note = normalized_note(&promotion_note.get_untracked());
                            if replacing && note.is_none() {
                                import_runtime.fail(
                                    "A review note is required before replacing the scenario baseline",
                                );
                                return;
                            }
                            match serde_json::from_str::<RuntimeScenarioReleaseBaseline>(
                                &baseline_json.get_untracked(),
                            ) {
                                Ok(release_baseline)
                                    if release_baseline.format
                                        == FLY_RUNTIME_SCENARIO_RELEASE_BASELINE
                                        && release_baseline.is_valid() =>
                                {
                                    baseline.set(Some(release_baseline.clone()));
                                    if let Some(callback) = callback.get_value() {
                                        callback.run(PageBuilderScenarioBaselineChange::save(
                                            release_baseline,
                                            note,
                                        ));
                                    }
                                    promotion_note.set(String::new());
                                    import_runtime.last_error.set(None);
                                    import_runtime.announce(
                                        "Scenario release baseline imported",
                                    );
                                }
                                Ok(release_baseline) => {
                                    let details = release_baseline
                                        .validate()
                                        .into_iter()
                                        .take(4)
                                        .map(|diagnostic| diagnostic.message)
                                        .collect::<Vec<_>>()
                                        .join("; ");
                                    import_runtime.fail(if details.is_empty() {
                                        format!(
                                            "Unsupported scenario release baseline format `{}`",
                                            release_baseline.format,
                                        )
                                    } else {
                                        format!("Invalid scenario release baseline: {details}")
                                    });
                                }
                                Err(error) => import_runtime.fail(format!(
                                    "Invalid scenario release baseline JSON: {error}",
                                )),
                            }
                            }
                        }
                    >{import_label.clone()}</button>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let note = normalized_note(&promotion_note.get_untracked());
                            baseline.set(None);
                            baseline_json.set(String::new());
                            if let Some(callback) = callback.get_value() {
                                callback.run(PageBuilderScenarioBaselineChange::clear(note));
                            }
                            promotion_note.set(String::new());
                        }
                    >{clear_label.clone()}</button>
                </div>

                <textarea
                    class="min-h-36 w-full rounded border border-input bg-background px-2 py-1 font-mono text-[11px]"
                    placeholder="Paste fly_runtime_scenario_release_baseline JSON"
                    prop:value=move || baseline_json.get()
                    on:input=move |event| baseline_json.set(event_target_value(&event))
                ></textarea>

                {move || {
                    let Some(previous) = baseline.get() else {
                        return view! {
                            <p class="text-xs text-muted-foreground">
                                "Capture or paste a persisted baseline to gate scenario output regressions."
                            </p>
                        }
                        .into_any();
                    };
                    let evaluation = report_runtime.controller.with(|controller| {
                        evaluate_runtime_scenario_release(
                            controller.editor().document(),
                            Some(&previous),
                            RuntimeScenarioReleasePolicy::block_broken(),
                        )
                    });
                    let (status_class, status_label) = match evaluation.status {
                        RuntimeScenarioReleaseStatus::Stable => (
                            "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700",
                            "Stable",
                        ),
                        RuntimeScenarioReleaseStatus::RequiresReview => (
                            "rounded bg-amber-500/10 px-2 py-1 text-amber-700",
                            "Requires review",
                        ),
                        RuntimeScenarioReleaseStatus::Broken => (
                            "rounded bg-destructive/10 px-2 py-1 text-destructive",
                            "Broken",
                        ),
                        RuntimeScenarioReleaseStatus::BaselineInvalid => (
                            "rounded bg-destructive/10 px-2 py-1 text-destructive",
                            "Invalid baseline",
                        ),
                        RuntimeScenarioReleaseStatus::BaselineMissing => (
                            "rounded bg-destructive/10 px-2 py-1 text-destructive",
                            "Baseline missing",
                        ),
                        RuntimeScenarioReleaseStatus::Disabled => (
                            "rounded bg-muted px-2 py-1 text-muted-foreground",
                            "Disabled",
                        ),
                    };
                    let changes = evaluation
                        .diff
                        .as_ref()
                        .map(|diff| diff.changes.clone())
                        .unwrap_or_default();
                    let visual_changes = changes
                        .iter()
                        .filter(|change| {
                            change.impact() == RuntimeScenarioRenderChangeImpact::Visual
                        })
                        .count();
                    let breaking_changes = changes
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
                                "baseline {} · source {}",
                                previous.baseline_hash,
                                previous.source_project_hash,
                            )}</p>
                            {if changes.is_empty() {
                                view! {
                                    <p class="rounded bg-muted/50 px-2 py-1">
                                        "No scenario output changes"
                                    </p>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="space-y-1">
                                        {changes.into_iter().map(|change| {
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
                            {evaluation.diagnostics.into_iter().map(|diagnostic| view! {
                                <p class="rounded bg-destructive/10 px-2 py-1 text-destructive">
                                    <strong>{diagnostic.code}</strong>
                                    <span class="ml-1">{diagnostic.message}</span>
                                </p>
                            }).collect_view()}
                        </div>
                    }
                    .into_any()
                }}
            </section>
        </Show>
    }
}

fn normalized_note(value: &str) -> Option<String> {
    let note = value.trim();
    (!note.is_empty()).then(|| note.to_string())
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
        RuntimeScenarioRenderChange::SelectionChanged { .. } => "selected page changed".to_string(),
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
        } => format!("scenario `{scenario_id}` rendered changed from {previous} to {next}"),
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

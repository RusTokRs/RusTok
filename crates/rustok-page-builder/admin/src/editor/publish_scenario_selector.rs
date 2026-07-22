use crate::editor::AdminEditorRuntime;
use crate::{
    load_publish_scenario_selection, resolve_publish_scenario, save_publish_scenario_selection,
};
use fly::RuntimeScenarioReleaseBaseline;
use leptos::ev::Event;
use leptos::prelude::*;

#[component]
pub fn PublishScenarioSelectorPanel(
    runtime: AdminEditorRuntime,
    baseline: RwSignal<Option<RuntimeScenarioReleaseBaseline>>,
) -> impl IntoView {
    let page_id = runtime
        .controller
        .with(|controller| controller.page_id().to_string());
    let selected_scenario = RwSignal::new(None::<String>);
    let sync_runtime = runtime.clone();
    let sync_page_id = page_id.clone();

    Effect::new(move |_| {
        let Some(baseline) = baseline.get() else {
            selected_scenario.set(None);
            return;
        };
        let stored = match load_publish_scenario_selection(&sync_page_id, &baseline.baseline_hash) {
            Ok(stored) => stored,
            Err(error) => {
                sync_runtime.fail(error.to_string());
                None
            }
        };
        let selected = match baseline.scenarios.as_slice() {
            [scenario] => {
                if let Err(error) = save_publish_scenario_selection(
                    &sync_page_id,
                    &baseline.baseline_hash,
                    Some(&scenario.id),
                ) {
                    sync_runtime.fail(error.to_string());
                }
                Some(scenario.id.clone())
            }
            scenarios if scenarios.len() > 1 => stored.filter(|scenario_id| {
                scenarios
                    .iter()
                    .any(|scenario| scenario.id == *scenario_id)
            }),
            _ => None,
        };
        selected_scenario.set(selected);
    });

    let select_runtime = runtime.clone();
    let select_page_id = page_id.clone();
    let select_baseline = baseline;
    let on_change = Callback::new(move |event: Event| {
        let scenario_id = event_target_value(&event);
        let scenario_id = scenario_id.trim();
        let Some(baseline) = select_baseline.get_untracked() else {
            select_runtime.fail("Promote a runtime scenario baseline before publish");
            selected_scenario.set(None);
            return;
        };
        if scenario_id.is_empty() {
            match save_publish_scenario_selection(
                &select_page_id,
                &baseline.baseline_hash,
                None,
            ) {
                Ok(()) => {
                    selected_scenario.set(None);
                    select_runtime.announce("Publish runtime scenario selection cleared");
                }
                Err(error) => select_runtime.fail(error.to_string()),
            }
            return;
        }
        match resolve_publish_scenario(&baseline, Some(scenario_id)) {
            Ok(scenario) => match save_publish_scenario_selection(
                &select_page_id,
                &baseline.baseline_hash,
                Some(&scenario.id),
            ) {
                Ok(()) => {
                    selected_scenario.set(Some(scenario.id.clone()));
                    select_runtime.announce(format!(
                        "Publish runtime scenario selected: {}",
                        scenario.label
                    ));
                    select_runtime.last_error.set(None);
                }
                Err(error) => select_runtime.fail(error.to_string()),
            },
            Err(error) => select_runtime.fail(error.to_string()),
        }
    });

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <div>
                <h2 class="font-semibold">"Publish runtime scenario"</h2>
                <p class="mt-1 text-xs text-muted-foreground">
                    "Choose the promoted scenario that will be bound into the reviewed publish receipt. The selection is ephemeral and contains no runtime context."
                </p>
            </div>
            {move || match baseline.get() {
                None => view! {
                    <p class="rounded bg-destructive/10 px-2 py-2 text-xs text-destructive" role="status">
                        "Promote a scenario regression baseline before publishing this page."
                    </p>
                }.into_any(),
                Some(baseline) if baseline.scenarios.is_empty() => view! {
                    <p class="rounded bg-destructive/10 px-2 py-2 text-xs text-destructive" role="status">
                        "The promoted baseline has no runtime scenarios and cannot be published."
                    </p>
                }.into_any(),
                Some(baseline) => {
                    let scenario_count = baseline.scenarios.len();
                    let baseline_hash = baseline.baseline_hash.clone();
                    let options = baseline.scenarios.into_iter().map(|scenario| {
                        let value = scenario.id.clone();
                        let label = format!("{} · {}", scenario.label, scenario.id);
                        view! { <option value=value>{label}</option> }
                    }).collect_view();
                    let change = on_change;
                    view! {
                        <label class="block text-xs font-medium text-card-foreground">
                            "Reviewed scenario"
                            <select
                                class="mt-1 w-full rounded border border-input bg-background px-2 py-2 text-xs"
                                prop:value=move || selected_scenario.get().unwrap_or_default()
                                on:change=move |event| change.run(event)
                                disabled=scenario_count == 1
                            >
                                {if scenario_count > 1 {
                                    view! { <option value="">"Select a promoted scenario"</option> }.into_any()
                                } else {
                                    ().into_any()
                                }}
                                {options}
                            </select>
                        </label>
                        <p class="break-all text-[11px] text-muted-foreground">
                            {format!("Selection scope: page {page_id} · baseline {baseline_hash}")}
                        </p>
                        {move || if scenario_count > 1 && selected_scenario.get().is_none() {
                            view! {
                                <p class="rounded bg-amber-500/10 px-2 py-2 text-xs text-amber-800" role="status">
                                    "Publish remains blocked until one promoted scenario is selected explicitly."
                                </p>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    }.into_any()
                }
            }}
        </section>
    }
}

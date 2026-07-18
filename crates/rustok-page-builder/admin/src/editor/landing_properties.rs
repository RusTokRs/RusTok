use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use crate::AdminCanvasController;
use fly::{
    LandingPropertyIssue, LandingPropertyIssueKind, LandingPropertySnapshot,
    LandingPropertyValidationReport, LandingSectionPropertySnapshot, TraitValueKind,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
struct SelectedLandingSection {
    snapshot: LandingSectionPropertySnapshot,
    issues: Vec<LandingPropertyIssue>,
}

#[component]
pub(crate) fn LandingPropertiesPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.landingProperties",
        "Landing properties",
    );
    let empty = t(
        locale.as_deref(),
        "page_builder.landingProperties.empty",
        "Select a landing section or one of its nested components.",
    );
    let valid_label = t(
        locale.as_deref(),
        "page_builder.landingProperties.valid",
        "Contract valid",
    );
    let invalid_label = t(
        locale.as_deref(),
        "page_builder.landingProperties.invalid",
        "Contract issues",
    );
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let panel_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            {move || {
                let selected = panel_runtime.controller.with(selected_landing_section);
                match selected {
                    Some(selected) => {
                        let status_label = if selected.issues.is_empty() {
                            valid_label.clone()
                        } else {
                            format!("{}: {}", invalid_label, selected.issues.len())
                        };
                        let status_class = if selected.issues.is_empty() {
                            "rounded bg-emerald-500/10 px-2 py-1 text-xs text-emerald-700"
                        } else {
                            "rounded bg-destructive/10 px-2 py-1 text-xs text-destructive"
                        };
                        let section_kind = humanize(selected.snapshot.section_kind.as_str());
                        let section_id = selected
                            .snapshot
                            .section_component_id
                            .clone()
                            .unwrap_or_else(|| "unassigned".to_string());
                        let groups = group_properties(selected.snapshot.properties);
                        let issues = selected.issues;
                        view! {
                            <div class="space-y-3">
                                <div class="flex flex-wrap items-center justify-between gap-2 text-sm">
                                    <div>
                                        <div class="font-medium">{section_kind}</div>
                                        <code class="text-xs text-muted-foreground">{section_id}</code>
                                    </div>
                                    <span class=status_class>{status_label}</span>
                                </div>

                                {(!issues.is_empty()).then(|| view! {
                                    <div class="space-y-1 rounded border border-destructive/30 bg-destructive/5 p-2">
                                        {issues.into_iter().map(|issue| view! {
                                            <p class="text-xs text-destructive">{issue_message(&issue)}</p>
                                        }).collect_view()}
                                    </div>
                                })}

                                {groups.into_iter().map(|(group, properties)| {
                                    let group_label = humanize(&group);
                                    let runtime = panel_runtime.clone();
                                    let apply_label = apply_label.clone();
                                    view! {
                                        <div class="space-y-2 border-t border-border pt-3 first:border-t-0 first:pt-0">
                                            <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                {group_label}
                                            </h3>
                                            {properties.into_iter().map(|snapshot| view! {
                                                <LandingPropertyEditorRow
                                                    runtime=runtime.clone()
                                                    snapshot
                                                    apply_label=apply_label.clone()
                                                />
                                            }).collect_view()}
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }
                        .into_any()
                    }
                    None => view! {
                        <p class="text-sm text-muted-foreground">{empty.clone()}</p>
                    }
                    .into_any(),
                }
            }}
        </section>
    }
}

#[component]
fn LandingPropertyEditorRow(
    runtime: AdminEditorRuntime,
    snapshot: LandingPropertySnapshot,
    apply_label: String,
) -> impl IntoView {
    let schema = snapshot.schema.clone();
    let available = snapshot
        .target
        .as_ref()
        .and_then(|target| target.component_id.as_ref())
        .is_some();
    let initial = value_text(
        snapshot
            .target
            .as_ref()
            .and_then(|target| target.value.as_ref()),
    );
    let value = RwSignal::new(initial);
    let input_snapshot = snapshot.clone();
    let apply_snapshot = snapshot.clone();
    let input_runtime = runtime.clone();
    let apply_runtime = runtime;
    let options = schema.options.clone();
    let target_path = snapshot.target.as_ref().map(|target| target.path.clone());

    view! {
        <div class="space-y-2 rounded border border-border/70 p-2">
            <div class="flex items-start justify-between gap-2">
                <label class="text-sm font-medium">{schema.label.clone()}</label>
                <code class="text-[11px] text-muted-foreground">{schema.id.clone()}</code>
            </div>

            {match schema.value_type {
                TraitValueKind::Boolean => {
                    let checked = snapshot
                        .target
                        .as_ref()
                        .and_then(|target| target.value.as_ref())
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    view! {
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                disabled=!available
                                prop:checked=checked
                                on:change=move |event| apply_property_value(
                                    &input_runtime,
                                    &input_snapshot,
                                    if event_target_checked(&event) { "true" } else { "false" },
                                )
                            />
                            <span>{if checked { "true" } else { "false" }}</span>
                        </label>
                    }
                    .into_any()
                }
                TraitValueKind::Select => view! {
                    <select
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        disabled=!available
                        prop:value=move || value.get()
                        on:change=move |event| {
                            let selected = event_target_value(&event);
                            value.set(selected.clone());
                            apply_property_value(&input_runtime, &input_snapshot, &selected);
                        }
                    >
                        <option value="">"—"</option>
                        {options.into_iter().map(|option| view! {
                            <option value=option.value>{option.label}</option>
                        }).collect_view()}
                    </select>
                }
                .into_any(),
                TraitValueKind::Multiline => view! {
                    <textarea
                        class="min-h-20 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        disabled=!available
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    ></textarea>
                }
                .into_any(),
                TraitValueKind::Number => view! {
                    <input
                        type="number"
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        disabled=!available
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    />
                }
                .into_any(),
                TraitValueKind::Text | TraitValueKind::Url => view! {
                    <input
                        type=if matches!(schema.value_type, TraitValueKind::Url) { "url" } else { "text" }
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        disabled=!available
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    />
                }
                .into_any(),
            }}

            <div
                class="flex items-center justify-between gap-2"
                class:hidden=matches!(schema.value_type, TraitValueKind::Boolean | TraitValueKind::Select)
            >
                <span class="min-w-0 truncate text-[11px] text-muted-foreground">
                    {target_path.unwrap_or_else(|| "Property target is unavailable".to_string())}
                </span>
                <button
                    type="button"
                    disabled=!available
                    class="rounded border border-border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-50"
                    on:click=move |_| apply_property_value(
                        &apply_runtime,
                        &apply_snapshot,
                        &value.get_untracked(),
                    )
                >{apply_label}</button>
            </div>
        </div>
    }
}

fn selected_landing_section(controller: &AdminCanvasController) -> Option<SelectedLandingSection> {
    let selected_id = controller.ui().state.selection.component_id.clone()?;
    let active_page_index = controller.active_page_index();
    let document = controller.editor().document();
    let report = LandingPropertyValidationReport::for_document(document);
    let mut current_id = Some(selected_id);

    while let Some(component_id) = current_id {
        if let Some(snapshot) = report.sections.iter().find(|section| {
            section.page_index == active_page_index
                && section.section_component_id.as_deref() == Some(component_id.as_str())
        }) {
            let issues = report
                .issues
                .iter()
                .filter(|issue| {
                    issue.page_index == snapshot.page_index
                        && issue.section_path == snapshot.section_path
                })
                .cloned()
                .collect();
            return Some(SelectedLandingSection {
                snapshot: snapshot.clone(),
                issues,
            });
        }
        current_id = document.component_parent_id(&component_id);
    }

    None
}

fn group_properties(
    properties: Vec<LandingPropertySnapshot>,
) -> Vec<(String, Vec<LandingPropertySnapshot>)> {
    let mut groups = Vec::<(String, Vec<LandingPropertySnapshot>)>::new();
    for property in properties {
        if let Some((_, values)) = groups
            .iter_mut()
            .find(|(group, _)| group == &property.schema.group)
        {
            values.push(property);
        } else {
            groups.push((property.schema.group.clone(), vec![property]));
        }
    }
    groups
}

fn apply_property_value(
    runtime: &AdminEditorRuntime,
    snapshot: &LandingPropertySnapshot,
    value: &str,
) {
    match snapshot.command_from_text(value) {
        Ok(command) => runtime.dispatch(UiIntent::execute(command)),
        Err(error) => runtime.fail(error.to_string()),
    }
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn humanize(value: &str) -> String {
    let mut value = value.replace('_', " ").replace('.', " ");
    if let Some(first) = value.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    value
}

fn issue_message(issue: &LandingPropertyIssue) -> String {
    match issue.kind {
        LandingPropertyIssueKind::InvalidRoleMarker => {
            "A component has an invalid landing property marker".to_string()
        }
        LandingPropertyIssueKind::MissingRole => format!("Missing target for `{}`", issue.role),
        LandingPropertyIssueKind::UnexpectedRoleOccurrence => {
            format!("Unexpected extra target for `{}`", issue.role)
        }
        LandingPropertyIssueKind::UnknownRole => {
            format!("Unknown landing property role `{}`", issue.role)
        }
        LandingPropertyIssueKind::ComponentTypeMismatch => format!(
            "Role `{}` expects `{}` but found `{}`",
            issue.role,
            issue
                .expected_component_type
                .as_deref()
                .unwrap_or("unknown"),
            issue.actual_component_type.as_deref().unwrap_or("unknown"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "hero",
                            "type": "section",
                            "flyLandingSection": "hero",
                            "components": [{
                                "id": "container",
                                "type": "container",
                                "components": [{
                                    "id": "headline",
                                    "type": "heading",
                                    "flyLandingProperty": "headline",
                                    "content": "Hello"
                                }, {
                                    "id": "body",
                                    "type": "text",
                                    "flyLandingProperty": "body",
                                    "content": "World"
                                }, {
                                    "id": "action",
                                    "type": "button",
                                    "flyLandingProperty": "primary_action",
                                    "attributes": { "href": "#start" },
                                    "content": "Start"
                                }]
                            }]
                        }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn nested_selection_resolves_nearest_landing_section() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("headline".to_string())))
            .expect("select headline");
        let selected = selected_landing_section(&controller).expect("landing section");
        assert_eq!(
            selected.snapshot.section_component_id.as_deref(),
            Some("hero")
        );
        assert_eq!(selected.snapshot.section_kind.as_str(), "hero");
        assert!(selected.issues.is_empty());
    }

    #[test]
    fn selection_outside_landing_section_has_no_property_panel() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("root".to_string())))
            .expect("select root");
        assert!(selected_landing_section(&controller).is_none());
    }
}

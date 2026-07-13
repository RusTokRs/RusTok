use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{ProjectDiffSummary, ProjectSnapshot, SnapshotCatalog};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};

#[component]
pub fn SnapshotPanel(
    runtime: AdminEditorRuntime,
    #[prop(optional)] on_restore_project: Option<Callback<Value>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.snapshots",
        "Session snapshots",
    );
    let capture_label = t(
        locale.as_deref(),
        "page_builder.action.captureSnapshot",
        "Capture snapshot",
    );
    let compare_label = t(
        locale.as_deref(),
        "page_builder.action.compareSnapshot",
        "Compare",
    );
    let restore_label = t(
        locale.as_deref(),
        "page_builder.action.restoreSnapshot",
        "Restore",
    );
    let remove_label = t(
        locale.as_deref(),
        "page_builder.action.remove",
        "Remove",
    );
    let empty_label = t(
        locale.as_deref(),
        "page_builder.snapshots.empty",
        "No session snapshots yet.",
    );
    let label = RwSignal::new(String::new());
    let catalog = RwSignal::new(SnapshotCatalog::default());
    let comparison = RwSignal::new(None::<(String, ProjectDiffSummary)>);
    let capture_runtime = runtime.clone();
    let list_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <div class="flex gap-2">
                <input
                    class="min-w-0 flex-1 rounded border border-input bg-background px-2 py-1 text-sm"
                    placeholder="Snapshot name"
                    prop:value=move || label.get()
                    on:input=move |event| label.set(event_target_value(&event))
                />
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        let snapshot_label = label.get_untracked();
                        let result = capture_runtime.controller.with(|controller| {
                            let mut catalog_value = catalog.get_untracked();
                            let result = catalog_value
                                .capture(
                                    snapshot_label,
                                    controller.editor().document(),
                                    Map::from_iter([
                                        (
                                            "documentId".to_string(),
                                            Value::String(controller.page_id().to_string()),
                                        ),
                                        (
                                            "revisionId".to_string(),
                                            Value::String(controller.revision_id().to_string()),
                                        ),
                                    ]),
                                )
                                .map(|snapshot| snapshot.id.clone());
                            catalog.set(catalog_value);
                            result
                        });
                        match result {
                            Ok(_) => {
                                label.set(String::new());
                                comparison.set(None);
                                capture_runtime.announce("Snapshot captured");
                            }
                            Err(error) => capture_runtime.fail(error.to_string()),
                        }
                    }
                >{capture_label}</button>
            </div>

            {move || {
                let snapshots = catalog
                    .get()
                    .iter()
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>();
                if snapshots.is_empty() {
                    return view! {
                        <p class="text-sm text-muted-foreground">{empty_label.clone()}</p>
                    }
                    .into_any();
                }
                view! {
                    <div class="space-y-2">
                        {snapshots.into_iter().map(|snapshot| {
                            let compare_runtime = list_runtime.clone();
                            let snapshot_id = snapshot.id.clone();
                            let remove_id = snapshot.id.clone();
                            let restore_snapshot = snapshot.clone();
                            let compare_label = compare_label.clone();
                            let restore_label = restore_label.clone();
                            let remove_label = remove_label.clone();
                            let restore_view = on_restore_project.map(|callback| {
                                view! {
                                    <button
                                        type="button"
                                        class="rounded border border-primary/40 px-2 py-1 text-primary"
                                        on:click=move |_| callback.run(
                                            restore_snapshot.project_data.clone()
                                        )
                                    >{restore_label}</button>
                                }
                            });
                            view! {
                                <article class="rounded border border-border p-2 text-xs">
                                    <div class="flex items-start justify-between gap-2">
                                        <div>
                                            <strong>{snapshot.label}</strong>
                                            <code class="mt-1 block break-all text-muted-foreground">{snapshot.project_hash}</code>
                                        </div>
                                        <span class="text-muted-foreground">{snapshot.id.clone()}</span>
                                    </div>
                                    <div class="mt-2 flex flex-wrap gap-2">
                                        <button
                                            type="button"
                                            class="rounded border border-border px-2 py-1"
                                            on:click=move |_| {
                                                let result = compare_runtime.controller.with(|controller| {
                                                    catalog
                                                        .get_untracked()
                                                        .compare_with_current(
                                                            &snapshot_id,
                                                            controller.editor().document(),
                                                        )
                                                });
                                                match result {
                                                    Ok(diff) => comparison.set(Some((snapshot_id.clone(), diff))),
                                                    Err(error) => compare_runtime.fail(error.to_string()),
                                                }
                                            }
                                        >{compare_label}</button>
                                        {restore_view}
                                        <button
                                            type="button"
                                            class="rounded border border-destructive/40 px-2 py-1 text-destructive"
                                            on:click=move |_| {
                                                catalog.update(|catalog| {
                                                    catalog.remove(&remove_id);
                                                });
                                                comparison.update(|comparison| {
                                                    if comparison
                                                        .as_ref()
                                                        .is_some_and(|(id, _)| id == &remove_id)
                                                    {
                                                        *comparison = None;
                                                    }
                                                });
                                            }
                                        >{remove_label}</button>
                                    </div>
                                </article>
                            }
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }}

            {move || comparison.get().map(|(snapshot_id, diff)| view! {
                <SnapshotDiffView snapshot_id diff />
            })}
        </section>
    }
}

#[component]
fn SnapshotDiffView(snapshot_id: String, diff: ProjectDiffSummary) -> impl IntoView {
    let sections = [
        ("pages added", diff.added_pages),
        ("pages removed", diff.removed_pages),
        ("pages changed", diff.changed_pages),
        ("components added", diff.added_components),
        ("components removed", diff.removed_components),
        ("components changed", diff.changed_components),
        ("assets added", diff.added_assets),
        ("assets removed", diff.removed_assets),
        ("assets changed", diff.changed_assets),
        ("style rules added", diff.added_style_rules),
        ("style rules removed", diff.removed_style_rules),
        ("style rules changed", diff.changed_style_rules),
    ];
    view! {
        <div class="space-y-2 rounded border border-border bg-muted/40 p-2 text-xs">
            <div class="flex items-center justify-between gap-2">
                <strong>{format!("Diff from {snapshot_id}")}</strong>
                <span>{format!("{} changes", diff.change_count())}</span>
            </div>
            <code class="block break-all">{format!(
                "{} → {}",
                diff.before_hash,
                diff.after_hash,
            )}</code>
            {sections.into_iter().filter(|(_, values)| !values.is_empty()).map(|(label, values)| view! {
                <div>
                    <strong>{format!("{label}: ")}</strong>
                    <span>{values.join(", ")}</span>
                </div>
            }).collect_view()}
            {diff.project_extensions_changed.then(|| view! {
                <div><strong>"project extensions changed"</strong></div>
            })}
        </div>
    }
}

#[allow(dead_code)]
fn _snapshot_type_anchor(snapshot: &ProjectSnapshot) -> &str {
    &snapshot.id
}

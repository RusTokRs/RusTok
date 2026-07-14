use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    diff_runtime_context_contracts, migrate_runtime_context, ContractChangeImpact,
    RuntimeContextContractChange, RuntimeContextContractSnapshot, RuntimeContextMigrationPolicy,
    RuntimeContextMigrationResult, RuntimeContractCompatibility,
    FLY_RUNTIME_CONTEXT_CONTRACT_V1,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn ContextCompatibilityPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.contextCompatibility",
        "Context contract compatibility",
    );
    let capture_label = t(
        locale.as_deref(),
        "page_builder.action.captureContractBaseline",
        "Capture current baseline",
    );
    let import_label = t(
        locale.as_deref(),
        "page_builder.action.importContractBaseline",
        "Use pasted baseline",
    );
    let migrate_label = t(
        locale.as_deref(),
        "page_builder.action.previewContextMigration",
        "Preview migration",
    );
    let apply_migration_label = t(
        locale.as_deref(),
        "page_builder.action.applyContextMigration",
        "Use migrated preview",
    );
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let baseline = RwSignal::new(None::<RuntimeContextContractSnapshot>);
    let baseline_json = RwSignal::new(String::new());
    let coerce_scalars = RwSignal::new(false);
    let synthesize_required = RwSignal::new(false);
    let migration_result = RwSignal::new(None::<RuntimeContextMigrationResult>);
    let capture_runtime = runtime.clone();
    let import_runtime = runtime.clone();
    let migrate_runtime = runtime.clone();
    let apply_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <div class="flex flex-wrap gap-2">
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        let snapshot = capture_runtime.controller.with(|controller| {
                            RuntimeContextContractSnapshot::from_document(
                                controller.editor().document()
                            )
                        });
                        baseline_json.set(
                            serde_json::to_string_pretty(&snapshot).unwrap_or_default()
                        );
                        baseline.set(Some(snapshot));
                        migration_result.set(None);
                        capture_runtime.last_error.set(None);
                        capture_runtime.announce("Runtime contract baseline captured");
                    }
                >{capture_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        match serde_json::from_str::<RuntimeContextContractSnapshot>(
                            &baseline_json.get_untracked()
                        ) {
                            Ok(snapshot) if snapshot.format == FLY_RUNTIME_CONTEXT_CONTRACT_V1 => {
                                baseline.set(Some(snapshot));
                                migration_result.set(None);
                                import_runtime.last_error.set(None);
                                import_runtime.announce("Runtime contract baseline imported");
                            }
                            Ok(snapshot) => import_runtime.fail(format!(
                                "Unsupported runtime contract snapshot format `{}`",
                                snapshot.format
                            )),
                            Err(error) => import_runtime.fail(format!(
                                "Invalid runtime contract baseline JSON: {error}"
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
                        migration_result.set(None);
                    }
                >{clear_label}</button>
            </div>

            <textarea
                class="min-h-32 w-full rounded border border-input bg-background px-2 py-1 font-mono text-[11px]"
                placeholder="Paste fly_runtime_context_contract_v1 snapshot JSON"
                prop:value=move || baseline_json.get()
                on:input=move |event| {
                    baseline_json.set(event_target_value(&event));
                    migration_result.set(None);
                }
            ></textarea>

            <div class="space-y-2 rounded bg-muted/40 p-2 text-xs">
                <label class="flex items-center gap-2">
                    <input
                        type="checkbox"
                        prop:checked=move || coerce_scalars.get()
                        on:change=move |event| {
                            coerce_scalars.set(event_target_checked(&event));
                            migration_result.set(None);
                        }
                    />
                    <span>"Coerce compatible scalar values"</span>
                </label>
                <label class="flex items-center gap-2">
                    <input
                        type="checkbox"
                        prop:checked=move || synthesize_required.get()
                        on:change=move |event| {
                            synthesize_required.set(event_target_checked(&event));
                            migration_result.set(None);
                        }
                    />
                    <span>"Synthesize placeholders for required values"</span>
                </label>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    disabled=move || baseline.get().is_none()
                    on:click=move |_| {
                        let Some(previous) = baseline.get_untracked() else {
                            migrate_runtime.fail("Runtime contract baseline is not selected");
                            return;
                        };
                        let input_context = migrate_runtime.runtime_context.get_untracked();
                        let result = migrate_runtime.controller.with(|controller| {
                            migrate_runtime_context(
                                &previous,
                                controller.editor().document(),
                                &input_context,
                                RuntimeContextMigrationPolicy {
                                    coerce_scalars: coerce_scalars.get_untracked(),
                                    synthesize_required_values: synthesize_required.get_untracked(),
                                    ..RuntimeContextMigrationPolicy::default()
                                },
                            )
                        });
                        let accepted = result.accepted;
                        migration_result.set(Some(result));
                        if accepted {
                            migrate_runtime.last_error.set(None);
                            migrate_runtime.announce("Runtime context migration preview passed");
                        } else {
                            migrate_runtime.fail("Runtime context migration still has blocking issues");
                        }
                    }
                >{migrate_label}</button>
            </div>

            {move || {
                let Some(previous) = baseline.get() else {
                    return view! {
                        <p class="text-xs text-muted-foreground">
                            "Capture or paste a baseline to classify contract changes."
                        </p>
                    }
                    .into_any();
                };
                let next = report_runtime.controller.with(|controller| {
                    RuntimeContextContractSnapshot::from_document(
                        controller.editor().document()
                    )
                });
                let diff = diff_runtime_context_contracts(&previous, &next);
                let compatibility_class = match diff.compatibility {
                    RuntimeContractCompatibility::Compatible => "text-emerald-700",
                    RuntimeContractCompatibility::RequiresReview => "text-amber-700",
                    RuntimeContractCompatibility::Breaking => "text-destructive",
                };
                let compatibility_label = match diff.compatibility {
                    RuntimeContractCompatibility::Compatible => "Compatible",
                    RuntimeContractCompatibility::RequiresReview => "Requires review",
                    RuntimeContractCompatibility::Breaking => "Breaking",
                };
                view! {
                    <div class="space-y-2 text-xs">
                        <div class="rounded bg-muted/50 px-2 py-1">
                            <strong class=compatibility_class>{compatibility_label}</strong>
                            <p class="mt-1 break-all text-muted-foreground">
                                {format!("{} → {}", diff.previous_hash, diff.next_hash)}
                            </p>
                        </div>
                        {if diff.changes.is_empty() {
                            view! {
                                <p class="rounded bg-emerald-500/10 px-2 py-1 text-emerald-700">
                                    "No runtime contract changes"
                                </p>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="space-y-1">
                                    {diff.changes.into_iter().map(|change| {
                                        let impact = change.impact();
                                        let impact_class = match impact {
                                            ContractChangeImpact::NonBreaking => "text-emerald-700",
                                            ContractChangeImpact::Behavioral => "text-amber-700",
                                            ContractChangeImpact::Breaking => "text-destructive",
                                        };
                                        view! {
                                            <div class="rounded bg-muted px-2 py-1">
                                                <span class=impact_class>{impact_label(impact)}</span>
                                                <span class="ml-1">{change_summary(&change)}</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }
                            .into_any()
                        }}
                        {(!diff.migration_hints.is_empty()).then(|| view! {
                            <details>
                                <summary class="cursor-pointer font-medium">"Migration hints"</summary>
                                <div class="mt-1 space-y-1">
                                    {diff.migration_hints.into_iter().map(|hint| view! {
                                        <p class="rounded bg-muted/50 px-2 py-1">
                                            <span>{hint.message}</span>
                                            {hint.automatic.then(|| view! {
                                                <span class="ml-1 text-emerald-700">"automatic"</span>
                                            })}
                                        </p>
                                    }).collect_view()}
                                </div>
                            </details>
                        })}
                    </div>
                }
                .into_any()
            }}

            {move || migration_result.get().map(|result| {
                let accepted = result.accepted;
                let status_class = if accepted {
                    "rounded bg-emerald-500/10 px-2 py-1 text-xs text-emerald-700"
                } else {
                    "rounded bg-destructive/10 px-2 py-1 text-xs text-destructive"
                };
                let migrated_context = result.migrated_context.clone();
                view! {
                    <div class="space-y-2 border-t border-border pt-2 text-xs">
                        <p class=status_class>{if accepted {
                            "Migration preview accepted"
                        } else {
                            "Migration preview rejected"
                        }}</p>
                        <div class="space-y-1">
                            {result.operations.into_iter().map(|operation| view! {
                                <p class="rounded bg-muted px-2 py-1">
                                    <strong>{format!("{:?}", operation.kind)}</strong>
                                    <span class="ml-1">{operation.message}</span>
                                </p>
                            }).collect_view()}
                        </div>
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            disabled=!accepted
                            on:click={
                                let apply_runtime = apply_runtime.clone();
                                move |_| {
                                    if accepted {
                                        apply_runtime.set_runtime_context(migrated_context.clone());
                                        apply_runtime.announce("Migrated runtime context applied to preview");
                                    }
                                }
                            }
                        >{apply_migration_label.clone()}</button>
                    </div>
                }
            })}
        </section>
    }
}

fn impact_label(impact: ContractChangeImpact) -> &'static str {
    match impact {
        ContractChangeImpact::NonBreaking => "Non-breaking",
        ContractChangeImpact::Behavioral => "Behavioral",
        ContractChangeImpact::Breaking => "Breaking",
    }
}

fn change_summary(change: &RuntimeContextContractChange) -> String {
    match change {
        RuntimeContextContractChange::FieldAdded {
            path,
            required,
            has_default,
            ..
        } => format!(
            "field `{path}` added{}{}",
            if *required { " as required" } else { "" },
            if *has_default { " with default" } else { "" },
        ),
        RuntimeContextContractChange::FieldRemoved { path, .. } => {
            format!("field `{path}` removed")
        }
        RuntimeContextContractChange::FieldTypeChanged {
            path,
            previous,
            next,
            ..
        } => format!(
            "field `{path}` type changed from {} to {}",
            previous.as_str(),
            next.as_str(),
        ),
        RuntimeContextContractChange::FieldItemTypeChanged {
            path,
            previous,
            next,
            ..
        } => format!(
            "field `{path}` item type changed from {} to {}",
            previous.map(|kind| kind.as_str()).unwrap_or("any"),
            next.map(|kind| kind.as_str()).unwrap_or("any"),
        ),
        RuntimeContextContractChange::FieldRequiredChanged {
            path,
            previous,
            next,
            ..
        } => format!("field `{path}` required changed from {previous} to {next}"),
        RuntimeContextContractChange::FieldDefaultChanged { path, .. } => {
            format!("field `{path}` default changed")
        }
        RuntimeContextContractChange::ComputedAdded { path, .. } => {
            format!("computed value `{path}` added")
        }
        RuntimeContextContractChange::ComputedRemoved { path, .. } => {
            format!("computed value `{path}` removed")
        }
        RuntimeContextContractChange::ComputedExpressionChanged { path, .. } => {
            format!("computed value `{path}` expression changed")
        }
        RuntimeContextContractChange::ComputedDependenciesChanged { path, .. } => {
            format!("computed value `{path}` dependencies changed")
        }
        RuntimeContextContractChange::ComputedFallbackChanged { path, .. } => {
            format!("computed value `{path}` fallback changed")
        }
    }
}

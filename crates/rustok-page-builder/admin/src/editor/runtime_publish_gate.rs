use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn RuntimePublishGatePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.runtimePublishGate",
        "Runtime publish gate",
    );
    let check_label = t(
        locale.as_deref(),
        "page_builder.action.checkRuntimeGate",
        "Check gate",
    );
    let has_policy = runtime.runtime_publish_gate_policy.is_some();
    let check_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <Show when=move || has_policy>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <div class="flex items-center justify-between gap-2">
                    <h2 class="font-semibold">{title}</h2>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            if let Some(evaluation) = check_runtime.evaluate_runtime_publish_gate() {
                                let allowed = evaluation.allowed;
                                check_runtime.runtime_publish_gate_evaluation.set(Some(evaluation));
                                if allowed {
                                    check_runtime.announce("Runtime publish gate passed");
                                    check_runtime.last_error.set(None);
                                } else {
                                    check_runtime.fail("Runtime publish gate has blocking issues");
                                }
                            }
                        }
                    >{check_label}</button>
                </div>

                {move || {
                    let evaluation = report_runtime
                        .runtime_publish_gate_evaluation
                        .get()
                        .or_else(|| report_runtime.evaluate_runtime_publish_gate());
                    let Some(evaluation) = evaluation else {
                        return view! {
                            <p class="text-xs text-muted-foreground">"Runtime publish policy is not configured."</p>
                        }
                        .into_any();
                    };
                    let error_count = evaluation.diagnostics.iter().filter(|diagnostic| {
                        diagnostic.severity == fly::ValidationSeverity::Error
                    }).count();
                    let warning_count = evaluation.diagnostics.iter().filter(|diagnostic| {
                        diagnostic.severity == fly::ValidationSeverity::Warning
                    }).count();
                    let scenario_summary = evaluation.scenarios.as_ref().map(|suite| {
                        format!(
                            "{} scenario(s) accepted · {} rejected",
                            suite.accepted_count,
                            suite.rejected_count,
                        )
                    });
                    let status_class = if evaluation.allowed {
                        "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700"
                    } else {
                        "rounded bg-destructive/10 px-2 py-1 text-destructive"
                    };
                    let status_text = if evaluation.allowed {
                        "Publish runtime gate passed".to_string()
                    } else {
                        format!("Publish blocked · {error_count} error(s)")
                    };
                    view! {
                        <div class="space-y-2 text-xs">
                            <p class=status_class>{status_text}</p>
                            <p class="text-muted-foreground">{format!(
                                "{error_count} errors · {warning_count} warnings"
                            )}</p>
                            {scenario_summary.map(|summary| view! {
                                <p class="text-muted-foreground">{summary}</p>
                            })}
                            {evaluation.diagnostics.into_iter().filter(|diagnostic| {
                                diagnostic.severity != fly::ValidationSeverity::Info
                            }).take(8).map(|diagnostic| view! {
                                <p class="rounded bg-muted px-2 py-1">
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

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
    let gate_passed = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.passed",
        "Publish runtime gate passed",
    );
    let gate_blocked = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.blocked",
        "Publish blocked",
    );
    let gate_blocking_issues = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.blockingIssues",
        "Runtime publish gate has blocking issues",
    );
    let policy_missing = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.policyMissing",
        "Runtime publish policy is not configured.",
    );
    let readiness_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.readiness",
        "Landing readiness",
    );
    let ready_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.ready",
        "ready",
    );
    let readiness_blocked_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.readinessBlocked",
        "blocked",
    );
    let errors_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.errors",
        "errors",
    );
    let warnings_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.warnings",
        "warnings",
    );
    let scenarios_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.scenarios",
        "Scenarios",
    );
    let accepted_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.accepted",
        "accepted",
    );
    let rejected_label = t(
        locale.as_deref(),
        "page_builder.runtimePublishGate.rejected",
        "rejected",
    );
    let seo_label = t(
        locale.as_deref(),
        "page_builder.readinessCategory.seo",
        "SEO",
    );
    let content_label = t(
        locale.as_deref(),
        "page_builder.readinessCategory.content",
        "Content",
    );
    let routes_label = t(
        locale.as_deref(),
        "page_builder.readinessCategory.routes",
        "Routes",
    );
    let locales_label = t(
        locale.as_deref(),
        "page_builder.readinessCategory.locales",
        "Locales",
    );
    let runtime_contracts_label = t(
        locale.as_deref(),
        "page_builder.readinessCategory.runtimeContracts",
        "Runtime contracts",
    );
    let has_policy = runtime.runtime_publish_gate_policy.is_some();
    let check_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <Show when=move || has_policy>
            <section class="space-y-3 rounded-xl border border-border bg-card p-3">
                <div class="flex items-center justify-between gap-2">
                    <h2 class="font-semibold">{title.clone()}</h2>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click={
                            let check_runtime = check_runtime.clone();
                            let gate_passed = gate_passed.clone();
                            let gate_blocking_issues = gate_blocking_issues.clone();
                            move |_| {
                                if let Some(evaluation) = check_runtime.evaluate_runtime_publish_gate() {
                                    let allowed = evaluation.allowed;
                                    check_runtime.runtime_publish_gate_evaluation.set(Some(evaluation));
                                    if allowed {
                                        check_runtime.announce(gate_passed.clone());
                                        check_runtime.last_error.set(None);
                                    } else {
                                        check_runtime.fail(gate_blocking_issues.clone());
                                    }
                                }
                            }
                        }
                    >{check_label.clone()}</button>
                </div>

                {{
                    let report_runtime = report_runtime.clone();
                    let policy_missing = policy_missing.clone();
                    let gate_passed = gate_passed.clone();
                    let gate_blocked = gate_blocked.clone();
                    let readiness_label = readiness_label.clone();
                    let ready_label = ready_label.clone();
                    let readiness_blocked_label = readiness_blocked_label.clone();
                    let errors_label = errors_label.clone();
                    let warnings_label = warnings_label.clone();
                    let scenarios_label = scenarios_label.clone();
                    let accepted_label = accepted_label.clone();
                    let rejected_label = rejected_label.clone();
                    let seo_label = seo_label.clone();
                    let content_label = content_label.clone();
                    let routes_label = routes_label.clone();
                    let locales_label = locales_label.clone();
                    let runtime_contracts_label = runtime_contracts_label.clone();
                    move || {
                        let _tracked_context = report_runtime.runtime_context.get();
                        let evaluation = report_runtime.evaluate_runtime_publish_gate();
                        let Some(evaluation) = evaluation else {
                            return view! {
                                <p class="text-xs text-muted-foreground">{policy_missing.clone()}</p>
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
                                "{scenarios_label}: {} {accepted_label} · {} {rejected_label}",
                                suite.accepted_count,
                                suite.rejected_count,
                            )
                        });
                        let readiness_summary = evaluation.readiness.as_ref().map(|report| {
                            let categories = report
                                .categories
                                .iter()
                                .map(|summary| {
                                    let category = match summary.category {
                                        fly::LandingReadinessCategory::Seo => seo_label.as_str(),
                                        fly::LandingReadinessCategory::Content => content_label.as_str(),
                                        fly::LandingReadinessCategory::Routes => routes_label.as_str(),
                                        fly::LandingReadinessCategory::Locales => locales_label.as_str(),
                                        fly::LandingReadinessCategory::RuntimeContracts => {
                                            runtime_contracts_label.as_str()
                                        }
                                    };
                                    format!(
                                        "{category}: {} {errors_label}, {} {warnings_label}",
                                        summary.error_count,
                                        summary.warning_count,
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(" · ");
                            format!(
                                "{readiness_label}: {} · {categories}",
                                if report.ready {
                                    ready_label.as_str()
                                } else {
                                    readiness_blocked_label.as_str()
                                },
                            )
                        });
                        let status_class = if evaluation.allowed {
                            "rounded bg-emerald-500/10 px-2 py-1 text-emerald-700"
                        } else {
                            "rounded bg-destructive/10 px-2 py-1 text-destructive"
                        };
                        let status_text = if evaluation.allowed {
                            gate_passed.clone()
                        } else {
                            format!("{gate_blocked} · {error_count} {errors_label}")
                        };
                        view! {
                            <div class="space-y-2 text-xs">
                                <p class=status_class>{status_text}</p>
                                <p class="text-muted-foreground">{format!(
                                    "{error_count} {errors_label} · {warning_count} {warnings_label}"
                                )}</p>
                                {readiness_summary.map(|summary| view! {
                                    <p class="text-muted-foreground">{summary}</p>
                                })}
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
                    }
                }}
            </section>
        </Show>
    }
}

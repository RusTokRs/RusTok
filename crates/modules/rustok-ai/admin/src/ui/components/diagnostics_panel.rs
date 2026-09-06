use crate::core::average_latency_ms;
use crate::i18n::t;
use crate::model::AiAdminBootstrap;
use crate::ui::leptos::{
    Card, InfoItem, average_run_latency_summary, bucket_summary, recent_run_summary,
    stream_event_kind_label,
};
use leptos::prelude::*;

#[component]
pub fn AiDiagnosticsPanel(ui_locale: Option<String>, bootstrap: AiAdminBootstrap) -> impl IntoView {
    let ui_locale_diagnostics = ui_locale.clone();

    view! {
        <Card title=t(ui_locale_diagnostics.as_deref(), "ai.card.diagnostics", "Diagnostics")>
                                    <div class="grid gap-3 sm:grid-cols-2">
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.routerResolutions", "Router resolutions")
                                            value=bootstrap.metrics.router_resolutions_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.overrides", "Overrides")
                                            value=bootstrap.metrics.router_overrides_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.completedRuns", "Completed runs")
                                            value=bootstrap.metrics.completed_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.failedRuns", "Failed runs")
                                            value=bootstrap.metrics.failed_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.waitingApproval", "Waiting approval")
                                            value=bootstrap.metrics.waiting_approval_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.localeFallbacks", "Locale fallbacks")
                                            value=bootstrap.metrics.locale_fallback_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.directSelected", "Direct selected")
                                            value=bootstrap.metrics.selected_direct_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.mcpSelected", "MCP selected")
                                            value=bootstrap.metrics.selected_mcp_total.to_string()
                                        />
                                    </div>
                                    <div class="mt-4 space-y-3 text-sm text-muted-foreground">
                                        <div>
                                            {average_run_latency_summary(
                                                ui_locale_diagnostics.as_deref(),
                                                average_latency_ms(
                                                    bootstrap.metrics.run_latency_ms_total,
                                                    bootstrap.metrics.run_latency_samples,
                                                )
                                            )}
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.providerBuckets", "Provider buckets")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.provider_slug_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.executionTargets", "Execution targets")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.execution_target_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.taskProfiles", "Task profiles")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.task_profile_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.resolvedLocales", "Resolved locales")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.resolved_locale_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.recentRuns", "Recent runs")}</div>
                                            <div>{recent_run_summary(ui_locale_diagnostics.as_deref(), &bootstrap.recent_runs)}</div>
                                        </div>
                                        <div class="space-y-2">
                                            {bootstrap
                                                .recent_runs
                                                .iter()
                                                .take(8)
                                                .map(|run| {
                                                    let error_message = run.error_message.clone().unwrap_or_default();
                                                    let has_error = !error_message.trim().is_empty();
                                                    view! {
                                                        <div class="rounded-lg border border-border px-3 py-3">
                                                            <div class="font-medium text-foreground">
                                                                {format!(
                                                                    "{} В· {} В· {} ms",
                                                                    run.session_title,
                                                                    run.status,
                                                                    run.duration_ms,
                                                                )}
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">
                                                                {format!(
                                                                    "{} В· {} В· {} -> {}",
                                                                    run.provider_display_name,
                                                                    run
                                                                        .execution_target
                                                                        .clone()
                                                                        .unwrap_or_else(|| run.execution_path.clone()),
                                                                    run.requested_locale.clone().unwrap_or_else(|| "auto".to_string()),
                                                                    run.resolved_locale,
                                                                )}
                                                            </div>
                                                            <div class="mt-1 text-xs text-muted-foreground">
                                                                {format!(
                                                                    "{}{}",
                                                                    run.started_at,
                                                                    run.task_profile_slug
                                                                        .as_ref()
                                                                        .map(|slug| format!(" В· task {slug}"))
                                                                        .unwrap_or_default(),
                                                                )}
                                                            </div>
                                                            <Show when=move || has_error>
                                                                <div class="mt-1 text-sm text-destructive">
                                                                    {error_message.clone()}
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.recentStreamEvents", "Recent stream events")}</div>
                                            <div>
                                                {if bootstrap.recent_stream_events.is_empty() {
                                                    t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.noRecentEvents", "No recent events yet.")
                                                } else {
                                                    t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.cachedEventsCount", "{count} cached event(s)")
                                                        .replace("{count}", bootstrap.recent_stream_events.len().to_string().as_str())
                                                }}
                                            </div>
                                        </div>
                                        <div class="space-y-2">
                                            {bootstrap
                                                .recent_stream_events
                                                .iter()
                                                .take(6)
                                                .map(|event| {
                                                    let status = stream_event_kind_label(
                                                        ui_locale_diagnostics.as_deref(),
                                                        &event.event_kind,
                                                    );
                                                    let error_message = event.error_message.clone().unwrap_or_default();
                                                    let has_error = !error_message.trim().is_empty();
                                                    view! {
                                                        <div class="rounded-lg border border-border px-3 py-3">
                                                            <div class="font-medium text-foreground">
                                                                {format!("{status} · {}", event.run_id)}
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">{event.created_at.clone()}</div>
                                                            <div class="mt-1 whitespace-pre-wrap text-foreground">
                                                                {event
                                                                    .accumulated_content
                                                                    .clone()
                                                                    .or(event.content_delta.clone())
                                                                    .or(event.tool_call.clone().map(|tool_call| format!("{}({})", tool_call.name, tool_call.arguments)))
                                                                    .unwrap_or_else(|| t(ui_locale_diagnostics.as_deref(), "ai.common.noTextualDelta", "(no textual delta)"))}
                                                            </div>
                                                            <Show when=move || has_error>
                                                                <div class="mt-1 text-sm text-destructive">
                                                                    {error_message.clone()}
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                </Card>
    }
}

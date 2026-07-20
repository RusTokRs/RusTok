use crate::i18n::t;
use crate::model::{AiAdminBootstrap, AiChatSessionDetailPayload, AiLiveStreamStatePayload};
use crate::transport;
use crate::ui::leptos::{
    Card, locale_flow_summary, run_path_summary, session_list_summary, session_profile_summary,
    stream_event_kind_label, stream_status_summary, tool_trace_summary,
};
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::AdminQueryKey;

#[component]
pub fn AiChatSessionPanel(
    ui_locale: Option<String>,
    bootstrap: AiAdminBootstrap,
    session_detail: LocalResource<
        Result<Option<AiChatSessionDetailPayload>, crate::transport::ApiError>,
    >,
    live_stream: Signal<Option<AiLiveStreamStatePayload>>,
    reply_message: RwSignal<String>,
    on_send_message: Callback<SubmitEvent>,
    select_session_query_writer: leptos_ui_routing::RouteQueryWriter,
    set_refresh_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let ui_locale_sessions = ui_locale.clone();
    let ui_locale_operator = ui_locale.clone();

    view! {
        <Card title=t(ui_locale_sessions.as_deref(), "ai.card.sessions", "Sessions")>
                                    <div class="space-y-2">
                                            {bootstrap.sessions.into_iter().map(|session| {
                                                let session_id = session.id.clone();
                                                let item_query_writer = select_session_query_writer.clone();
                                                view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        item_query_writer.replace_value(
                                                            AdminQueryKey::SessionId.as_str(),
                                                            session_id.clone(),
                                                        );
                                                    }
                                                >
                                                    <div class="font-medium">{session.title}</div>
                                                    <div class="text-muted-foreground">
                                                        {session_list_summary(
                                                            ui_locale_sessions.as_deref(),
                                                            session.status.as_str(),
                                                            session.execution_mode.as_str(),
                                                            session.latest_run_status.as_deref(),
                                                            session.pending_approvals,
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>

                                <Card title=t(ui_locale_operator.as_deref(), "ai.card.operatorChat", "Operator Chat")>
                                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                                        {move || {
                                            let ui_locale = ui_locale_operator.clone();
                                            let on_send_message = on_send_message;
                                            session_detail.get().map(|result| match result {
                                            Ok(Some(detail)) => {
                                                let ui_locale_form = ui_locale.clone();
                                                let ui_locale_approvals = ui_locale.clone();
                                                let ui_locale_runs = ui_locale.clone();
                                                let pending_approvals = detail
                                                    .approvals
                                                    .clone()
                                                    .into_iter()
                                                    .filter(|item| item.status == "pending")
                                                    .collect::<Vec<_>>();
                                                view! {
                                                    <div class="space-y-5">
                                                        <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                            <div class="font-medium">{detail.session.title.clone()}</div>
                                                            <div class="text-muted-foreground">
                                                                {session_profile_summary(
                                                                    ui_locale.as_deref(),
                                                                    detail.provider_profile.display_name.as_str(),
                                                                    detail.provider_profile.model.as_str(),
                                                                    detail.session.execution_mode.as_str(),
                                                                )}
                                                            </div>
                                                            <div class="text-muted-foreground">
                                                                {locale_flow_summary(
                                                                    ui_locale.as_deref(),
                                                                    detail.session.requested_locale.as_deref(),
                                                                    detail.session.resolved_locale.as_str(),
                                                                )}
                                                            </div>
                                                        </div>

                                                        <div class="max-h-[380px] space-y-3 overflow-y-auto rounded-xl border border-border p-3">
                                                            {detail.messages.into_iter().map(|message| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                                        {message.role.clone()}
                                                                    </div>
                                                                    <div>{message.content.unwrap_or_else(|| t(ui_locale.as_deref(), "ai.common.noTextualContent", "(no textual content)"))}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>

                                                        {move || live_stream.get().map(|stream| {
                                                            let content = if stream.content.trim().is_empty() {
                                                                t(ui_locale.as_deref(), "ai.session.waitingForAssistant", "Waiting for assistant output...")
                                                            } else {
                                                                stream.content.clone()
                                                            };
                                                            let error_message = stream.error_message.clone().unwrap_or_default();
                                                            let has_error = !error_message.trim().is_empty();
                                                            view! {
                                                                <div class="rounded-lg border border-sky-300 bg-sky-50 px-4 py-3 text-sm text-sky-950">
                                                                    <div class="flex items-center justify-between gap-3">
                                                                        <div class="font-medium">{t(ui_locale.as_deref(), "ai.session.liveStream", "Live stream")}</div>
                                                                        <div class="text-xs uppercase tracking-wide text-sky-800">
                                                                            {stream_status_summary(
                                                                                ui_locale.as_deref(),
                                                                                stream.connected,
                                                                                stream.status.as_str(),
                                                                            )}
                                                                        </div>
                                                                    </div>
                                                                    <div class="mt-2 whitespace-pre-wrap text-sky-950">{content}</div>
                                                                    <Show when=move || has_error>
                                                                        <div class="mt-2 text-sm text-destructive">{error_message.clone()}</div>
                                                                    </Show>
                                                                </div>
                                                            }
                                                        })}

                                                        <form class="space-y-3" on:submit=move |ev| on_send_message.run(ev)>
                                                            <textarea
                                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                                prop:value=reply_message
                                                                on:input=move |ev| reply_message.set(event_target_value(&ev))
                                                            />
                                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_form.as_deref(), "ai.action.send", "Send")}</button>
                                                        </form>

                                                        {if pending_approvals.is_empty() {
                                                            ().into_any()
                                                        } else {
                                                            view! {
                                                                <div class="space-y-3">
                                                                    <div class="text-sm font-semibold">{t(ui_locale_approvals.as_deref(), "ai.session.pendingApprovals", "Pending approvals")}</div>
                                                                    {pending_approvals.into_iter().map(|approval| {
                                                                    let approve_id = approval.id.clone();
                                                                    let reject_id = approval.id.clone();
                                                                    let approval_reason = approval.reason.unwrap_or_else(|| t(ui_locale_approvals.as_deref(), "ai.session.operatorApprovalRequired", "Operator approval required"));
                                                                    let approve_label = t(ui_locale_approvals.as_deref(), "ai.action.approve", "Approve");
                                                                    let reject_label = t(ui_locale_approvals.as_deref(), "ai.action.reject", "Reject");
                                                                    let reject_reason = t(ui_locale_approvals.as_deref(), "ai.session.rejectedInAdminUi", "Rejected in admin UI");
                                                                    view! {
                                                                        <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                                                            <div class="font-medium">{approval.tool_name.clone()}</div>
                                                                            <div class="mt-1 text-amber-800">{approval_reason}</div>
                                                                            <div class="mt-3 flex gap-2">
                                                                                <button
                                                                                    class="rounded-md bg-amber-900 px-3 py-2 text-xs font-semibold text-white"
                                                                                    on:click=move |_| {
                                                                                        let approval_id = approve_id.clone();
                                                                                        spawn_local(async move {
                                                                                            let _ = transport::resume_approval(approval_id, true, None).await;
                                                                                            set_refresh_nonce.update(|value| *value += 1);
                                                                                        });
                                                                                    }
                                                                                >
                                                                                    {approve_label}
                                                                                </button>
                                                                                <button
                                                                                    class="rounded-md border border-amber-900 px-3 py-2 text-xs font-semibold text-amber-900"
                                                                                    on:click=move |_| {
                                                                                        let approval_id = reject_id.clone();
                                                                                        let reject_reason = reject_reason.clone();
                                                                                        spawn_local(async move {
                                                                                            let _ = transport::resume_approval(approval_id, false, Some(reject_reason)).await;
                                                                                            set_refresh_nonce.update(|value| *value += 1);
                                                                                        });
                                                                                    }
                                                                                >
                                                                                    {reject_label}
                                                                                </button>
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                    }).collect_view()}
                                                                </div>
                                                            }.into_any()
                                                        }}

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.session.runs", "Runs")}</div>
                                                            {detail.runs.into_iter().map(|run| {
                                                                let error_message = run.error_message.clone().unwrap_or_default();
                                                                let has_error = !error_message.is_empty();
                                                                view! {
                                                                    <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                        <div class="font-medium">{run.model.clone()}</div>
                                                                        <div class="text-muted-foreground">
                                                                            {run_path_summary(
                                                                                ui_locale_runs.as_deref(),
                                                                                run.status.as_str(),
                                                                                run.execution_mode.as_str(),
                                                                                run.execution_path.as_str(),
                                                                            )}
                                                                        </div>
                                                                        <div class="text-muted-foreground">
                                                                            {locale_flow_summary(
                                                                                ui_locale_runs.as_deref(),
                                                                                run.requested_locale.as_deref(),
                                                                                run.resolved_locale.as_str(),
                                                                            )}
                                                                        </div>
                                                                        <Show when=move || has_error>
                                                                            <div class="mt-2 text-destructive">{error_message.clone()}</div>
                                                                        </Show>
                                                                    </div>
                                                                }
                                                            }).collect_view()}
                                                        </div>

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.session.toolTrace", "Tool trace")}</div>
                                                            {detail.tool_traces.into_iter().map(|trace| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="font-medium">{trace.tool_name}</div>
                                                                    <div class="text-muted-foreground">{tool_trace_summary(ui_locale_runs.as_deref(), trace.status.as_str(), trace.duration_ms)}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.diagnostics.recentStreamEvents", "Recent stream events")}</div>
                                                            {if detail.recent_stream_events.is_empty() {
                                                                view! {
                                                                    <div class="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">
                                                                        {t(ui_locale_runs.as_deref(), "ai.session.noCachedStreamEvents", "No cached stream events for this session yet.")}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    {detail.recent_stream_events.into_iter().take(10).map(|event| {
                                                                        let status = stream_event_kind_label(
                                                                            ui_locale_runs.as_deref(),
                                                                            &event.event_kind,
                                                                        );
                                                                        let error_message = event.error_message.clone().unwrap_or_default();
                                                                        let has_error = !error_message.trim().is_empty();
                                                                        view! {
                                                                            <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                                <div class="font-medium">{format!("{status} · {}", event.run_id)}</div>
                                                                                <div class="text-xs text-muted-foreground">{event.created_at}</div>
                                                                                <div class="mt-1 whitespace-pre-wrap">
                                                                                    {event
                                                                                        .accumulated_content
                                                                                        .or(event.content_delta)
                                                                                        .or(event.tool_call.map(|tool_call| format!("{}({})", tool_call.name, tool_call.arguments)))
                                                                                        .unwrap_or_else(|| t(ui_locale_runs.as_deref(), "ai.common.noTextualDelta", "(no textual delta)"))}
                                                                                </div>
                                                                                <Show when=move || has_error>
                                                                                    <div class="mt-1 text-destructive">{error_message.clone()}</div>
                                                                                </Show>
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}
                                                                }.into_any()
                                                            }}
                                                        </div>
                                                    </div>
                                                }.into_any()
                                            }
                                            Ok(None) => view! {
                                                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-sm text-muted-foreground">
                                                    {t(ui_locale.as_deref(), "ai.session.selectPrompt", "Select a session to inspect chat history, traces, and approvals.")}
                                                </div>
                                            }.into_any(),
                                            Err(err) => view! {
                                                <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                                    {t(ui_locale.as_deref(), "ai.session.loadSession", "Failed to load session: {error}")
                                                        .replace("{error}", err.to_string().as_str())}
                                                </div>
                                            }.into_any(),
                                            })
                                        }}
                                    </Suspense>
                                </Card>
    }
}

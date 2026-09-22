use leptos::prelude::*;
use rustok_seo::{SeoIndexDeliveryStatusRecord, SeoIndexReplayMode};

use crate::core::SeoIndexReplayForm;
use crate::i18n::t;
use crate::transport::ApiError;

#[component]
pub fn SeoIndexPane(
    ui_locale: Option<String>,
    index_status: Resource<Result<SeoIndexDeliveryStatusRecord, ApiError>>,
    replay_form: RwSignal<SeoIndexReplayForm>,
    busy_key: RwSignal<Option<String>>,
    on_refresh: Callback<()>,
    on_run_repair_only: Callback<()>,
    on_run_repair_replay: Callback<()>,
) -> impl IntoView {
    let busy = Signal::derive(move || busy_key.get().is_some());
    let title = t(
        ui_locale.as_deref(),
        "seo.index.title",
        "Index delivery tracking",
    );
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.index.subtitle",
        "Operator-facing observability and remediation surface for SEO -> index delivery flow.",
    );

    view! {
        <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-2">
                <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
            </div>

            <div class="grid gap-4 rounded-xl border border-border/80 bg-background/60 p-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
                <label class="space-y-2 text-sm">
                    <span class="font-medium text-foreground">"Target type filter"</span>
                    <select
                        class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        prop:value=move || replay_form.get().target_type
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            replay_form.update(|draft| draft.target_type = value);
                        }
                    >
                        <option value="">"all"</option>
                        <option value="content">"content"</option>
                        <option value="product">"product"</option>
                    </select>
                </label>

                <label class="space-y-2 text-sm">
                    <span class="font-medium text-foreground">"Replay limit (1..500)"</span>
                    <input
                        type="number"
                        min="1"
                        max="500"
                        class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        prop:value=move || replay_form.get().limit.to_string()
                        on:input=move |ev| {
                            let value = event_target_value(&ev).parse::<i32>().unwrap_or(100);
                            replay_form.update(|draft| draft.limit = value);
                        }
                    />
                </label>

                <button
                    type="button"
                    class="self-end rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-60"
                    disabled=move || busy.get()
                    on:click=move |_| on_refresh.run(())
                >
                    "Refresh"
                </button>
            </div>

            <div class="grid gap-4 rounded-xl border border-border/80 bg-background/60 p-4 lg:grid-cols-2">
                <div class="space-y-3">
                    <label class="flex items-start gap-2 text-sm text-foreground">
                        <input
                            type="checkbox"
                            prop:checked=move || replay_form.get().confirm_repair_only
                            on:change=move |ev| {
                                let value = event_target_checked(&ev);
                                replay_form.update(|draft| draft.confirm_repair_only = value);
                            }
                        />
                        <span>
                            "I confirm running repair-only reindex for the selected tenant scope."
                        </span>
                    </label>
                    <button
                        type="button"
                        class="w-full rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-60"
                        disabled=move || busy.get() || !replay_form.get().confirm_repair_only
                        on:click=move |_| on_run_repair_only.run(())
                    >
                        "Run repair only"
                    </button>
                </div>
                <div class="space-y-3">
                    <label class="flex items-start gap-2 text-sm text-foreground">
                        <input
                            type="checkbox"
                            prop:checked=move || replay_form.get().confirm_replay_historical
                            on:change=move |ev| {
                                let value = event_target_checked(&ev);
                                replay_form.update(|draft| draft.confirm_replay_historical = value);
                            }
                        />
                        <span>
                            "I confirm historical replay (forward-only mode) and accept increased index workload."
                        </span>
                    </label>
                    <button
                        type="button"
                        class="w-full rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-60"
                        disabled=move || busy.get() || !replay_form.get().confirm_replay_historical
                        on:click=move |_| on_run_repair_replay.run(())
                    >
                        "Run repair + historical replay"
                    </button>
                </div>
            </div>

            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading index delivery status..."</p> }>
                {move || match index_status.get() {
                    Some(Ok(summary)) => {
                        let failure_sample_cards = if summary.failure_samples.is_empty() {
                            view! { <p class="text-sm text-muted-foreground">"No failed/dead-letter samples."</p> }
                                .into_any()
                        } else {
                            view! {
                                <ul class="space-y-2">
                                    {summary.failure_samples.into_iter().map(|sample| {
                                        let is_dead_letter = sample.status == "dead_letter";
                                        view! {
                                            <li class="rounded-lg border border-border/80 bg-background/70 px-3 py-2 text-sm">
                                                <div class="font-medium text-foreground">
                                                    {format!("{} / {}", sample.target_type, sample.status)}
                                                </div>
                                                <div class="mt-1 text-xs text-muted-foreground">
                                                    {format!("attempts: {} · updated: {}", sample.attempt_count, sample.updated_at.to_rfc3339())}
                                                </div>
                                                <div class="mt-1 break-words text-xs text-destructive">
                                                    {sample.last_error.unwrap_or_else(|| "n/a".to_string())}
                                                </div>
                                                <Show when=move || is_dead_letter>
                                                    <p class="mt-1 text-xs text-amber-700">
                                                        "Hint: run repair or historical replay after investigating index consumer health."
                                                    </p>
                                                </Show>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }
                            .into_any()
                        };
                        let cursor_cards = if summary.cursors.is_empty() {
                            view! { <p class="text-sm text-muted-foreground">"No cursor timeline yet."</p> }
                                .into_any()
                        } else {
                            view! {
                                <ul class="space-y-3">
                                    {summary.cursors.into_iter().map(|cursor| {
                                        let replay_badge = match cursor.replay_mode {
                                            SeoIndexReplayMode::NotStarted => "not_started",
                                            SeoIndexReplayMode::RepairOnly => "repair_only",
                                            SeoIndexReplayMode::ReplayRequested => "replay_requested",
                                            SeoIndexReplayMode::Replaying => "replaying",
                                            SeoIndexReplayMode::ReplayCompleted => "replay_completed",
                                        };
                                        view! {
                                            <li class="rounded-xl border border-border/80 bg-background/70 px-4 py-3 text-sm">
                                                <div class="flex flex-wrap items-center gap-2">
                                                    <span class="font-medium text-foreground">{cursor.target_type}</span>
                                                    <span class="rounded-full border border-border px-2 py-0.5 text-xs text-muted-foreground">
                                                        {replay_badge}
                                                    </span>
                                                    <span class="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                                                        "forward-only"
                                                    </span>
                                                </div>
                                                <div class="mt-2 grid gap-1 text-xs text-muted-foreground">
                                                    <div>{format!("initial: {}", cursor.initial_cursor_at.to_rfc3339())}</div>
                                                    <div>{format!("high-water: {}", cursor.high_water_mark_at.to_rfc3339())}</div>
                                                    <div>{format!(
                                                        "last repair: {}",
                                                        cursor
                                                            .last_repair_cursor_at
                                                            .map(|value| value.to_rfc3339())
                                                            .unwrap_or_else(|| "n/a".to_string())
                                                    )}</div>
                                                    <div>{format!(
                                                        "replay requested: {}",
                                                        cursor
                                                            .replay_requested_at
                                                            .map(|value| value.to_rfc3339())
                                                            .unwrap_or_else(|| "n/a".to_string())
                                                    )}</div>
                                                    <div>{format!(
                                                        "replay completed: {}",
                                                        cursor
                                                            .replay_completed_at
                                                            .map(|value| value.to_rfc3339())
                                                            .unwrap_or_else(|| "n/a".to_string())
                                                    )}</div>
                                                </div>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }
                            .into_any()
                        };

                        view! {
                            <div class="space-y-4">
                                <div class="grid gap-3 md:grid-cols-5">
                                    <MetricTile label="pending" value=summary.pending_count />
                                    <MetricTile label="sent" value=summary.sent_count />
                                    <MetricTile label="retry" value=summary.retry_count />
                                    <MetricTile label="failed" value=summary.failed_count />
                                    <MetricTile label="dead_letter" value=summary.dead_letter_count />
                                </div>

                                <div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                                    <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
                                        <h3 class="text-base font-semibold text-card-foreground">"Cursor timeline"</h3>
                                        {cursor_cards}
                                    </div>

                                    <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
                                        <h3 class="text-base font-semibold text-card-foreground">"Failure drilldown"</h3>
                                        {failure_sample_cards}
                                    </div>
                                </div>
                            </div>
                        }
                        .into_any()
                    }
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No index delivery status available."</p> }.into_any(),
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn MetricTile(label: &'static str, value: i32) -> impl IntoView {
    view! {
        <article class="rounded-xl border border-border/80 bg-background/70 px-3 py-2">
            <p class="text-xs uppercase tracking-wide text-muted-foreground">{label}</p>
            <p class="mt-1 text-lg font-semibold text-card-foreground">{value.to_string()}</p>
        </article>
    }
}

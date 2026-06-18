use super::*;

#[component]
pub(super) fn RuntimeContext(bootstrap: ChannelAdminBootstrap) -> impl IntoView {
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-1">
                <h2 class="text-lg font-semibold text-card-foreground">
                    {t(ui_locale.as_deref(), "channel.runtime.title", "Runtime Context")}
                </h2>
                <p class="text-sm text-muted-foreground">
                    {t(
                        ui_locale.as_deref(),
                        "channel.runtime.subtitle",
                        "Channel resolved by middleware for the current request.",
                    )}
                </p>
            </div>
            {match bootstrap.current_channel {
                Some(current) => view! {
                    <div class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.slug", "Slug") value=current.slug />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.name", "Name") value=current.name />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.source", "Source") value=resolution_source_label(&current.resolution_source, ui_locale.as_deref()) />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.target", "Target") value=current.target_value.unwrap_or_else(|| t(ui_locale.as_deref(), "channel.runtime.na", "n/a")) />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.type", "Type") value=current.target_type.unwrap_or_else(|| t(ui_locale.as_deref(), "channel.runtime.na", "n/a")) />
                    </div>
                    <div class="mt-4 rounded-xl border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-800">
                        {resolution_source_description(&current.resolution_source, ui_locale.as_deref())}
                    </div>
                    <div class="mt-4 space-y-2">
                        <div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                            {t(
                                ui_locale.as_deref(),
                                "channel.runtime.traceTitle",
                                "Resolution Trace",
                            )}
                        </div>
                        <div class="space-y-2">
                            {current
                                .resolution_trace
                                .into_iter()
                                .map(|step| {
                                    let badge_class = resolution_outcome_badge_class(&step.outcome);
                                    let stage = resolution_stage_label(&step.stage, ui_locale.as_deref());
                                    let outcome = resolution_outcome_label(&step.outcome, ui_locale.as_deref());
                                    view! {
                                        <div class="rounded-xl border border-border bg-background px-4 py-3">
                                            <div class="flex flex-wrap items-center gap-2 text-xs">
                                                <span class="inline-flex items-center rounded-full border border-border px-2 py-1 font-medium text-muted-foreground">
                                                    {stage}
                                                </span>
                                                <span class=badge_class>
                                                    {outcome}
                                                </span>
                                            </div>
                                            <div class="mt-2 text-sm text-card-foreground">{step.detail}</div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-4 rounded-xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
                        {t(
                            ui_locale.as_deref(),
                            "channel.runtime.empty",
                            "No channel was resolved for the current request yet.",
                        )}
                    </div>
                }.into_any(),
            }}
        </section>
    }
}

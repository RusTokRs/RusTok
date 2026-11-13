use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::core::{
    EventDeliverySummaryViewModel, EventsAdminLabels, KNOWN_PROFILES, ProfileSelectionDecision,
    evaluate_profile_selection, is_known_profile,
};
use crate::transport;

/// Builds a resource the response resolves.
///
/// The module admin mounts inside the layout route that declares `SsrMode::Async`, so a
/// `LocalResource` here is `pending()` on the server and the reader is served the `Suspense`
/// fallback. See `DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`.
fn loaded_resource<S, Fut, T>(
    source: impl Fn() -> S + Send + Sync + 'static,
    fetcher: impl Fn(S) -> Fut + Send + Sync + 'static,
) -> Resource<T>
where
    S: PartialEq + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    Resource::new(source, fetcher)
}

#[component]
pub fn EventsAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let configuration = loaded_resource(
        move || (token.get(), tenant.get()),
        move |(token, tenant)| async move { transport::fetch_configuration(token, tenant).await },
    );
    // The operator's choice is an edit laid over the resolved configuration, not a signal an
    // `Effect` copies it into: an `Effect` is not driven before the response, so the form would be
    // served showing a profile nobody selected. The profile shown is the edit when there is one and
    // the backend's desired profile otherwise.
    let (profile_edit, set_profile_edit) = signal(Option::<String>::None);
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<String, String>>::None);
    let (show_iggy_dialog, set_show_iggy_dialog) = signal(false);

    let labels = EventsAdminLabels::resolve(None);

    let selected_profile = Signal::derive(move || {
        if let Some(edited) = profile_edit.get() {
            return edited;
        }
        configuration
            .get()
            .and_then(Result::ok)
            .map(|configuration| configuration.desired_profile)
            .filter(|profile| is_known_profile(profile))
            .unwrap_or_else(|| String::from("outbox"))
    });

    let select_profile = move |profile: String| {
        let iggy_configured = configuration
            .get_untracked()
            .and_then(Result::ok)
            .map(|value| value.iggy_configured)
            .unwrap_or(false);
        match evaluate_profile_selection(&profile, iggy_configured) {
            ProfileSelectionDecision::RequiresIggyDialog => {
                set_show_iggy_dialog.set(true);
            }
            ProfileSelectionDecision::Select(p) => {
                set_profile_edit.set(Some(p));
                set_save_result.set(None);
            }
        }
    };

    let labels_save = labels.clone();
    let save = move |_| {
        let token = token.get();
        let tenant = tenant.get();
        let profile = selected_profile.get();
        let labels_save = labels_save.clone();
        set_saving.set(true);
        set_save_result.set(None);
        spawn_local(async move {
            let result = transport::update_profile(token, tenant, profile)
                .await
                .map(|outcome| {
                    labels_save
                        .format_save_outcome(outcome.restart_required)
                        .to_string()
                })
                .map_err(|error| error.to_string());
            set_save_result.set(Some(result));
            set_saving.set(false);
        });
    };

    let labels_view = labels.clone();
    let labels_dialog = labels.clone();

    view! {
        <section class="flex flex-1 flex-col gap-6 p-4 md:px-6">
            <header class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    {labels.header_eyebrow.clone()}
                </p>
                <h1 class="mt-2 text-2xl font-semibold text-card-foreground">{labels.title.clone()}</h1>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">
                    {labels.subtitle.clone()}
                </p>
            </header>

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-xl bg-muted"></div> }>
                {
                    let labels_view = labels_view.clone();
                    move || {
                        let labels_view = labels_view.clone();
                        configuration.get().map(move |result| match result {
                            Ok(current) => {
                                let summary = EventDeliverySummaryViewModel::from_configuration(&current, &labels_view);
                                view! {
                                    <div class="space-y-5 rounded-xl border border-border bg-card p-6 shadow-sm">
                                        <div class="grid gap-3 lg:grid-cols-2">
                                            {KNOWN_PROFILES
                                                .into_iter()
                                                .map(|value| {
                                                    let label = labels_view.profile_label(value).to_string();
                                                    let description = labels_view.profile_description(value).to_string();
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class=move || {
                                                                if selected_profile.get() == value {
                                                                    "rounded-xl border border-primary bg-primary/5 p-4 text-left"
                                                                } else {
                                                                    "rounded-xl border border-border bg-background p-4 text-left hover:border-primary/50"
                                                                }
                                                            }
                                                            on:click=move |_| select_profile(value.to_string())
                                                        >
                                                            <span class="block font-semibold text-card-foreground">{label}</span>
                                                            <span class="mt-1 block text-sm text-muted-foreground">{description}</span>
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                        <dl class="grid gap-2 text-sm sm:grid-cols-2 lg:max-w-2xl">
                                            <dt class="text-muted-foreground">{labels_view.active_profile.clone()}</dt>
                                            <dd class="font-mono">{summary.active_profile}</dd>
                                            <dt class="text-muted-foreground">{labels_view.desired_profile.clone()}</dt>
                                            <dd class="font-mono">{summary.desired_profile}</dd>
                                            <dt class="text-muted-foreground">{labels_view.iggy_connector.clone()}</dt>
                                            <dd>{summary.connector_status}</dd>
                                        </dl>
                                    </div>
                                }
                                .into_any()
                            }
                            Err(error) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                                    {error.to_string()}
                                </div>
                            }
                            .into_any(),
                        })
                    }
                }
            </Suspense>

            <div class="flex flex-wrap items-center gap-4">
                <button
                    type="button"
                    class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || saving.get()
                    on:click=save
                >
                    {
                        let labels_btn = labels.clone();
                        move || if saving.get() { labels_btn.saving.clone() } else { labels_btn.save_profile.clone() }
                    }
                </button>
                {move || {
                    save_result.get().map(|result| match result {
                        Ok(message) => view! { <span class="text-sm text-emerald-600">{message}</span> }.into_any(),
                        Err(error) => view! { <span class="text-sm text-destructive">{error}</span> }.into_any(),
                    })
                }}
            </div>

            <Show when=move || show_iggy_dialog.get()>
                <div
                    role="alertdialog"
                    aria-modal="true"
                    aria-labelledby="iggy-required-title"
                    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
                >
                    <div class="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-xl">
                        <h2 id="iggy-required-title" class="text-lg font-semibold text-card-foreground">
                            {labels_dialog.iggy_dialog_title.clone()}
                        </h2>
                        <p class="mt-2 text-sm text-muted-foreground">
                            {labels_dialog.iggy_dialog_body.clone()}
                        </p>
                        <div class="mt-5 flex flex-wrap gap-3">
                            <a
                                href="/modules/iggy-connector"
                                class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
                            >
                                {labels_dialog.iggy_dialog_open.clone()}
                            </a>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-4 py-2 text-sm font-medium"
                                on:click=move |_| set_show_iggy_dialog.set(false)
                            >
                                {labels_dialog.iggy_dialog_cancel.clone()}
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}

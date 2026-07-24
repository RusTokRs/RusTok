use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::core::{DELIVERY_PROFILES, is_known_profile};
use crate::transport;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn EventsAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let configuration = local_resource(
        move || (token.get(), tenant.get()),
        move |(token, tenant)| async move { transport::fetch_configuration(token, tenant).await },
    );
    let (selected_profile, set_selected_profile) = signal(String::from("memory"));
    let (loaded, set_loaded) = signal(false);
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<String, String>>::None);
    let (show_iggy_dialog, set_show_iggy_dialog) = signal(false);

    Effect::new(move |_| {
        if loaded.get_untracked() {
            return;
        }
        if let Some(Ok(configuration)) = configuration.get() {
            if is_known_profile(&configuration.desired_profile) {
                set_selected_profile.set(configuration.desired_profile);
            }
            set_loaded.set(true);
        }
    });

    let select_profile = move |profile: String| {
        let iggy_configured = configuration
            .get_untracked()
            .and_then(Result::ok)
            .map(|value| value.iggy_configured)
            .unwrap_or(false);
        if profile == "outbox_iggy" && !iggy_configured {
            set_show_iggy_dialog.set(true);
            return;
        }
        set_selected_profile.set(profile);
        set_save_result.set(None);
    };

    let save = move |_| {
        let token = token.get();
        let tenant = tenant.get();
        let profile = selected_profile.get();
        set_saving.set(true);
        set_save_result.set(None);
        spawn_local(async move {
            let result = transport::update_profile(token, tenant, profile)
                .await
                .map(|outcome| {
                    if outcome.restart_required {
                        "Saved. Restart the server to activate this profile.".to_string()
                    } else {
                        "Saved. This profile is already active.".to_string()
                    }
                })
                .map_err(|error| error.to_string());
            set_save_result.set(Some(result));
            set_saving.set(false);
        });
    };

    view! {
        <section class="flex flex-1 flex-col gap-6 p-4 md:px-6">
            <header class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    "Platform module"
                </p>
                <h1 class="mt-2 text-2xl font-semibold text-card-foreground">"Events"</h1>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">
                    "Choose one global delivery profile. Iggy connection settings remain owned by the Iggy Connector module."
                </p>
            </header>

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-xl bg-muted"></div> }>
                {move || {
                    configuration.get().map(|result| match result {
                        Ok(current) => view! {
                            <div class="space-y-5 rounded-xl border border-border bg-card p-6 shadow-sm">
                                <div class="grid gap-3 lg:grid-cols-3">
                                    {DELIVERY_PROFILES
                                        .into_iter()
                                        .map(|(value, label, description)| {
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
                                    <dt class="text-muted-foreground">"Active profile"</dt>
                                    <dd class="font-mono">{current.active_profile}</dd>
                                    <dt class="text-muted-foreground">"Desired profile"</dt>
                                    <dd class="font-mono">{current.desired_profile}</dd>
                                    <dt class="text-muted-foreground">"Iggy connector"</dt>
                                    <dd>
                                        {format!(
                                            "{} ({})",
                                            current.iggy_mode,
                                            if current.iggy_configured { "ready" } else { "configuration required" },
                                        )}
                                    </dd>
                                </dl>
                            </div>
                        }
                        .into_any(),
                        Err(error) => view! {
                            <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                                {error.to_string()}
                            </div>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>

            <div class="flex flex-wrap items-center gap-4">
                <button
                    type="button"
                    class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || saving.get()
                    on:click=save
                >
                    {move || if saving.get() { "Saving..." } else { "Save profile" }}
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
                            "Configure Iggy first"
                        </h2>
                        <p class="mt-2 text-sm text-muted-foreground">
                            "The outbox_iggy profile requires a ready bundled or external Iggy connection."
                        </p>
                        <div class="mt-5 flex flex-wrap gap-3">
                            <a
                                href="/modules/iggy-connector"
                                class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
                            >
                                "Open Iggy Connector"
                            </a>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-4 py-2 text-sm font-medium"
                                on:click=move |_| set_show_iggy_dialog.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}
